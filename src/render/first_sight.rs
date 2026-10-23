//! First sight: nothing the raid can bring out should load from disk
//! mid-raid. A model or texture asked for when its thing first appears
//! (a robot kind's first spawn, an item's first drop, a material's first
//! use) loads in the background, then lands: the mesh goes up to the GPU
//! and its pipelines build on the frame it's first drawn. In a headset that
//! frame is a judder, however steady the rest are.
//!
//! The guard here sets off what a raid can bring out, after the valley has
//! loaded and settled, and fails on any model or texture that loads then.
//! See docs/PERFORMANCE.md, "Hitches".

use bevy::prelude::*;

/// Every model a raid can bring out, asked for at start: they load in the
/// background while the valley does and the player is in the truck, and
/// are in `Models`' cache when their thing first appears.
pub struct PreloadPlugin;

impl Plugin for PreloadPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("EXTRACTION_PRELOAD").is_ok_and(|v| v == "0") {
            return;
        }
        app.add_systems(Startup, preload_models)
            .add_systems(Update, preload_dressing);
    }
}

/// Each building's dressing, asked for as the building appears, whatever
/// will draw it and whenever: its furniture and fittings, and what the
/// people who lived there left (`world::clutter::dress`, before the ending
/// and after), from the same calls that place them, so a new prop can't
/// be missed; and the boards Destruction nails over a window.
#[allow(clippy::type_complexity)]
fn preload_dressing(
    added: Query<
        (&crate::world::buildings::Building, &Transform),
        Added<crate::world::buildings::Building>,
    >,
    zone: Option<Res<crate::world::Zone>>,
    server: Res<AssetServer>,
    mut models: ResMut<crate::models::Models>,
    mut boards: Local<bool>,
) {
    use crate::render::world_props::path;
    if !*boards {
        *boards = true;
        models.request(&path(crate::destruction::boards::MODEL), &server);
    }
    for (b, xf) in &added {
        let mut names: Vec<&'static str> = Vec::new();
        if let Some(geo) = crate::world::buildings::prefab(&b.prefab) {
            for d in &geo.props {
                names.extend(d.models.iter().map(|(n, _)| *n));
                names.extend(d.lods.iter().flatten().copied());
            }
        }
        names.push("window_sash_broken");
        if let Some(zone) = &zone {
            let at = Vec2::new(xf.translation.x, xf.translation.z);
            let tag = zone
                .map
                .placements
                .iter()
                .find(|p| p.prefab == b.prefab && Vec2::from(p.at).distance(at) < 0.5)
                .map_or("", |p| p.tag.as_str());
            for ending in [false, true] {
                names.extend(
                    crate::world::clutter::dress(&zone.map, &b.prefab, tag, xf, ending)
                        .into_iter()
                        .map(|(n, _)| n),
                );
            }
        }
        names.sort_unstable();
        names.dedup();
        for n in names {
            models.request(&path(n), &server);
        }
    }
}

/// The folders whose models come out mid-raid, asked for whole at start:
/// items and magazines (loot, a wreck's salvage, a crate's spill, a pod's
/// cargo), guns, robots (each kind's body and its own looks, the pod and
/// harpoon), animals (a herd's kind comes when the herd does: a dog pack
/// mid-raid once asked for its model then, now and then), vehicles, the
/// kit and the hands. Every folder under `assets/models` is here but
/// `props`, which come with a building's dressing (`preload_dressing`):
/// `every_model_folder_is_preloaded` holds it.
const MID_RAID: [&str; 8] = [
    "models/items",
    "models/mags",
    "models/guns",
    "models/robots",
    "models/animals",
    "models/vehicles",
    "models/kit",
    "models/hands",
];

/// The one folder not asked for whole: a valley's props, with its
/// buildings' dressing (`world_props::path`). Named for the test.
#[cfg(test)]
const DRESSING: &str = "models/props";

fn preload_models(server: Res<AssetServer>, mut models: ResMut<crate::models::Models>) {
    let root = crate::asset_root_path();
    let mut n = 0;
    for dir in MID_RAID {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut files: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|f| f.ends_with(".glb"))
            .collect();
        files.sort();
        for f in files {
            if models.request(&format!("{dir}/{f}"), &server).is_some() {
                n += 1;
            }
        }
    }
    info!("preload: {n} models asked for at start (render::first_sight)");
}

/// Where things come out: before the flat camera, or the headset's eye.
fn bring_out_at(w: &mut World) -> Vec3 {
    let mut q = w.query_filtered::<&GlobalTransform, With<Camera3d>>();
    let eye = q
        .iter(w)
        .map(|g| (g.translation(), g.forward().as_vec3()))
        .next()
        .unwrap_or((Vec3::ZERO, Vec3::NEG_Z));
    let mut f = eye.1;
    f.y = 0.0;
    eye.0 + f.normalize_or(Vec3::NEG_Z) * 8.0
}

/// `EXTRACTION_EVERYTHING=<frame>`: at that frame, bring everything out
/// (`bring_everything_out`) in front of the camera, for
/// `EXTRACTION_PIPELINES=1` to show what still builds at first sight.
pub struct EverythingPlugin;

impl Plugin for EverythingPlugin {
    fn build(&self, app: &mut App) {
        if let Some(at) = std::env::var("EXTRACTION_EVERYTHING")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            app.add_systems(Update, move |world: &mut World, mut frame: Local<u32>| {
                *frame += 1;
                if *frame == at {
                    info!("everything: out at frame {at}");
                    bring_everything_out_world(world);
                }
            });
        }
    }
}

/// Everything a raid can bring out, near the player: a robot of each
/// kind that comes mid-raid, and one of every item (a wreck's loot, a
/// crate's spill, a pod's cargo, a gun picked up).
#[cfg(test)]
pub(crate) fn bring_everything_out(app: &mut App) {
    bring_everything_out_world(app.world_mut());
}

/// A robot of each kind that comes mid-raid, and one of every item (a
/// wreck's loot, a crate's spill, a pod's cargo, a gun picked up), near
/// the eye.
pub(crate) fn bring_everything_out_world(world: &mut World) {
    use crate::robots::{Job, JobKind};
    let zone = world.resource::<crate::world::Zone>().clone();
    let catalog_res = crate::items::Catalog(world.resource::<crate::items::Catalog>().0.clone());
    let catalog = catalog_res.0.clone();
    let at = bring_out_at(world);
    let ground = |dx: f32, dz: f32| zone.on_ground(at.x + dx, at.z + dz);
    let job = |site: Vec3| Job {
        kind: JobKind::Watch,
        site,
        duration: 600.0,
        exit: site + Vec3::X * 200.0,
    };
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut c = Commands::new(&mut queue, world);
        let site = ground(12.0, 0.0);
        crate::robots::drone::spawn_drone(&mut c, site + Vec3::Y * 6.0, job(site));
        crate::robots::walker::spawn_walker(&mut c, &zone, ground(-12.0, 4.0), job(site));
        crate::robots::hound::spawn_hound(&mut c, &zone, ground(10.0, 10.0), job(site));
        crate::robots::heavy::spawn_heavy(
            &mut c,
            &zone,
            &catalog_res,
            ground(-25.0, -20.0),
            job(site),
        );
        crate::robots::dirigible::spawn_dirigible(&mut c, site + Vec3::Y * 26.0, job(site));
        crate::robots::grappler::spawn_grappler(&mut c, &zone, ground(15.0, -10.0), job(site));
        crate::robots::spotter::spawn_spotter(&mut c, &zone, ground(-15.0, 15.0).xz());
        crate::robots::mender::spawn_mender(&mut c, ground(5.0, -15.0) + Vec3::Y * 4.0, site);
        for (i, (kind, _)) in catalog.iter().enumerate() {
            let (x, z) = ((i % 16) as f32 * 0.8 - 6.0, (i / 16) as f32 * 0.8 + 3.0);
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(ground(x, z) + Vec3::Y * 0.3),
                    condition: 1.0,
                },
            );
        }
    }
    queue.apply(world);
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    /// What loaded from disk while recording, by path.
    #[derive(Resource, Default)]
    struct Loads {
        recording: bool,
        paths: Vec<String>,
    }

    fn record(
        mut loads: ResMut<Loads>,
        server: Res<AssetServer>,
        mut gltfs: MessageReader<AssetEvent<bevy::gltf::Gltf>>,
        mut images: MessageReader<AssetEvent<Image>>,
    ) {
        let mut got = Vec::new();
        for ev in gltfs.read() {
            if let AssetEvent::LoadedWithDependencies { id } = ev {
                got.push(server.get_path(*id).map(|p| p.to_string()));
            }
        }
        for ev in images.read() {
            if let AssetEvent::LoadedWithDependencies { id } = ev {
                got.push(server.get_path(*id).map(|p| p.to_string()));
            }
        }
        let _ = loads.recording;
        loads.paths.extend(got.into_iter().flatten());
    }

    /// Every model folder is asked for at start: whole (`MID_RAID`), or
    /// with the buildings it dresses (`DRESSING`). A folder that's neither
    /// (a new one) would load mid-raid whatever first used it, and only
    /// when that came out: a test that brings things out catches it only
    /// sometimes. The source's model paths are in those folders too.
    #[test]
    fn every_model_folder_is_preloaded() {
        let root = crate::asset_root_path();
        let mut wrong = Vec::new();
        for d in std::fs::read_dir(root.join("models")).unwrap().flatten() {
            if !d.path().is_dir() {
                continue;
            }
            let dir = format!("models/{}", d.file_name().to_string_lossy());
            if !super::MID_RAID.contains(&dir.as_str()) && dir != super::DRESSING {
                wrong.push(format!("{dir}/ (on disk)"));
            }
        }
        let mut stack = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&p) else {
                    continue;
                };
                for part in text.split("\"models/").skip(1) {
                    // `"models/<folder>/...`: a name, then a slash.
                    let folder: String = part
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if folder.is_empty() || !part[folder.len()..].starts_with('/') {
                        continue;
                    }
                    let dir = format!("models/{folder}");
                    if !super::MID_RAID.contains(&dir.as_str()) && dir != super::DRESSING {
                        wrong.push(format!("{dir}/ ({})", p.display()));
                    }
                }
            }
        }
        wrong.sort();
        wrong.dedup();
        assert!(
            wrong.is_empty(),
            "model folders neither preloaded whole (first_sight::MID_RAID) nor dressing: {wrong:?}"
        );
    }

    /// Everything a raid can bring out comes out, and none of it asks for
    /// a model that wasn't asked for at start (every disk load of one
    /// begins with `Models::request`): the fast form of the guard below.
    #[test]
    fn nothing_new_is_asked_for_mid_raid() {
        let _one_at_a_time = super::super::budget::tests::heavy();
        let mut app = super::super::budget::tests::client_threads("marrow_creek", "clearing", None);
        let asked = |app: &App| -> Vec<String> {
            let mut v: Vec<String> = app
                .world()
                .resource::<crate::models::Models>()
                .requested()
                .map(str::to_owned)
                .collect();
            v.sort();
            v
        };
        // The valley in: every building dressed, and nothing more asked for
        // in 60 frames (what the start asks for, however its threads run;
        // `Loaded` never comes here: no GPU takes the impostors' pictures).
        // A fixed 60 frames once let a slow start's last requests count as
        // mid-raid ones.
        let mut quiet = 0;
        let mut seen = 0;
        for _ in 0..5000 {
            app.update();
            let n = app
                .world()
                .resource::<crate::models::Models>()
                .requested()
                .count();
            let dressed = {
                let w = app.world_mut();
                w.query_filtered::<(), With<crate::world::buildings::PropsPending>>()
                    .iter(w)
                    .next()
                    .is_none()
            };
            quiet = if n == seen && dressed { quiet + 1 } else { 0 };
            seen = n;
            if quiet >= 60 {
                break;
            }
        }
        assert!(quiet >= 60, "the valley never settled its requests");
        let before = asked(&app);
        super::bring_everything_out(&mut app);
        for _ in 0..60 {
            app.update();
        }
        let new: Vec<String> = asked(&app)
            .into_iter()
            .filter(|p| before.binary_search(p).is_err())
            .collect();
        println!(
            "{} models asked for at start; {} after",
            before.len(),
            new.len()
        );
        assert!(
            new.is_empty(),
            "asked for mid-raid (each loads, then lands in a frame: preload it, `PreloadPlugin`): {new:?}"
        );
    }

    /// After the valley has loaded, everything a raid can bring out comes
    /// out, and nothing loads from disk for it (models, textures). Slow: it
    /// waits for the start's loads to land (the gauge tier, `cargo test
    /// --lib -- --ignored gauge_`).
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_nothing_loads_mid_raid() {
        let _one_at_a_time = super::super::budget::tests::heavy();
        let mut app = super::super::budget::tests::client_threads("marrow_creek", "clearing", None);
        app.init_resource::<Loads>().add_systems(Last, record);
        // Loaded and settled: until nothing has landed for 300 frames (the
        // harness runs frames far faster than real time; what loads at
        // start must have had time to).
        let mut quiet = 0;
        for _ in 0..20_000 {
            app.update();
            let landed = !app.world().resource::<Loads>().paths.is_empty();
            app.world_mut().resource_mut::<Loads>().paths.clear();
            quiet = if landed { 0 } else { quiet + 1 };
            if quiet >= 300 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let t = std::time::Instant::now();
        // A frame of game time a frame from here: on the real clock each slow
        // harness frame (two hundred things just out) owed several physics
        // ticks, and the 600 frames took 400 s. Loads land on their own
        // clock either way.
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 90.0),
        ));
        app.world_mut().resource_mut::<Loads>().recording = true;
        super::bring_everything_out(&mut app);
        // 600 frames, and 10 s at least for anything asked for to land.
        let mut n = 0;
        while n < 600 || t.elapsed().as_secs_f64() < 10.0 {
            app.update();
            n += 1;
        }
        let mut paths = std::mem::take(&mut app.world_mut().resource_mut::<Loads>().paths);
        paths.sort();
        paths.dedup();
        println!("loaded mid-raid: {} files", paths.len());
        for p in &paths {
            println!("    {p}");
        }
        assert!(
            paths.is_empty(),
            "{} files loaded from disk mid-raid (each a judder when it lands): {:?}",
            paths.len(),
            paths
        );
    }
}
