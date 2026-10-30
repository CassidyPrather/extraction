//! Developer tooling: screenshot-and-quit, the camera override, a frame
//! time readout, and the model gallery.

use crate::launch::LaunchOptions;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameCount>()
            .add_systems(
                Update,
                (
                    count_frames,
                    screenshot_when_ready,
                    gallery,
                    first_person_hands,
                    wear_kit,
                    sights_to_the_eye,
                ),
            )
            .add_systems(
                PreUpdate,
                (
                    pose_body_for_gallery,
                    carry_for_gallery,
                    climb_for_gallery,
                    fight_for_gallery,
                    treat_for_gallery,
                    spall_for_gallery,
                    plate_hits_for_gallery,
                    present_for_gallery,
                )
                    .chain()
                    .after(crate::rig::RigDriverSystems),
            );
    }
}

#[derive(Resource, Default)]
struct FrameCount {
    frames: u32,
    requested: bool,
}

fn count_frames(mut count: ResMut<FrameCount>) {
    count.frames += 1;
}

/// The longest a screenshot waits on the world beyond its frames, seconds
/// (the software renderer is slow; a material that never readies mustn't
/// hang a shot).
const SHOT_WAIT_MAX: f32 = 180.0;

/// `--screenshot`: after `--frames` frames, and once the ground's and the
/// buildings' materials are on the GPU and the buildings dressed
/// (`timeline::WorldReady`): taken at a fixed frame, a slow machine's shot
/// had trees floating over no ground.
fn screenshot_when_ready(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    mut count: ResMut<FrameCount>,
    world: crate::render::timeline::WorldReady,
    time: Res<Time<Real>>,
    loading: Option<Res<crate::rig::loading::LoadingView>>,
) {
    let Some(path) = &opts.screenshot else {
        return;
    };
    if count.requested || count.frames < opts.frames {
        return;
    }
    // In a headset, the world once the loading view has lifted
    // (`EXTRACTION_SHOT_LOADING=1`: the loading view itself).
    let shown = loading.is_none_or(|l| {
        l.lifted_at
            .is_some_and(|t| crate::render::timeline::since_start() - t > 2.0)
    }) || std::env::var_os("EXTRACTION_SHOT_LOADING").is_some();
    if !(world.materials() && world.props() && shown) {
        if time.elapsed_secs() < SHOT_WAIT_MAX {
            return;
        }
        warn!(
            "screenshot: taken after {SHOT_WAIT_MAX} s with the world not all in \
             (materials ready: {}, buildings dressed: {})",
            world.materials(),
            world.props()
        );
    }
    count.requested = true;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut save = save_to_disk(path.clone());
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            save(captured);
            exit.write(AppExit::Success);
        },
    );
}

/// `--gallery sights:<gun>`: the gun's sight line on the camera's, its
/// rear sight just ahead of the eye, and the eye down behind the sights
/// (so the front post goes soft, as `weapons::optics` draws it).
fn sights_to_the_eye(
    opts: Res<LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    catalog: Res<crate::items::Catalog>,
    mut guns: Query<(
        &crate::weapons::Gun,
        &crate::items::Item,
        &mut Transform,
        &mut crate::weapons::feel::Handling,
    )>,
) {
    let (Some(spec), Some((cam, yaw, pitch)), Some(zone)) = (&opts.gallery, opts.camera, zone)
    else {
        return;
    };
    let Some(id) = spec.strip_prefix("sights:") else {
        return;
    };
    let ground = zone.ground(cam.x, cam.z);
    let eye_y = if cam.y == 0.0 {
        ground + 1.7
    } else {
        cam.y.max(ground + 0.3)
    };
    let eye = Vec3::new(cam.x, eye_y, cam.z);
    let look = Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
    // The gallery's gun only: one lying in the zone would come to the eye
    // too, over it.
    for (gun, item, mut tf, mut handling) in &mut guns {
        if catalog.get(item.kind()).id != id {
            continue;
        }
        // The eye 4 cm behind the rear sight, on the line to the front post.
        let rear = gun.layout.sight.translation;
        let front = crate::weapons::feel::front_post(gun);
        let line = Quat::from_rotation_arc(Vec3::NEG_Z, (front - rear).normalize());
        let rot = look * line.inverse();
        let at_eye = rear - (front - rear).normalize() * 0.04;
        *tf = Transform::from_translation(eye - rot * at_eye).with_rotation(rot);
        handling.at_eye = 1.0;
    }
}

/// `--gallery SUBJECT[,YAW[,DIST[,SHIFT]]]`: puts something in front of the
/// `--camera` to look at, held still. SUBJECT is a gun id (shown from its
/// right side with its magazine seated), a magazine id, `guns` (all four
/// stacked), `drone` or `walker`; `<gun>+open` works its action instead of
/// seating a magazine. YAW turns it about the vertical (degrees),
/// DIST is how far ahead it sits (metres), SHIFT slides it along its own
/// Z so another part of a long gun is centred (-0.4 frames an AK's muzzle).
///
/// ```text
/// scripts/shot.sh target/shots/ak.png 40,0,262,0,0 20 --gallery ak74
/// ```
fn gallery(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    catalog: Res<crate::items::Catalog>,
    mut guns: Query<&mut crate::weapons::Gun>,
    mut sockets: Query<&mut crate::interact::Socket>,
    mut events: MessageWriter<crate::interact::InteractEvent>,
    mut stage: Local<u32>,
    mut pending_mags: Local<Vec<(Entity, Entity)>>,
    mut opening: Local<Vec<Entity>>,
    mut worked: Local<Vec<(Entity, extraction_core::gunsmith::Build)>>,
    mut told: Local<Vec<(Entity, u8)>>,
    mut bipods: Query<&mut crate::weapons::armoury::Bipod>,
    mut jamming: Local<bool>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use crate::items::{SpawnItem, spawn_item};
    let (Some(spec), Some((cam, yaw, pitch)), Some(zone)) = (&opts.gallery, opts.camera, zone)
    else {
        return;
    };
    *stage += 1;
    if *stage == 3 {
        let mut parts = spec.split(',');
        let first = parts.next().unwrap_or("ak74");
        // `+grime`, `+wet`, `+fog`: shown weathered (`weapons::weather`).
        let first = first
            .replace("+grime", "")
            .replace("+wet", "")
            .replace("+fog", "")
            // `+present`: the mag release held (`present_for_gallery`).
            .replace("+present", "");
        let first = first.as_str();
        // `pm_pistol+dry`: run dry, the slide held back on an empty
        // magazine (`weapons::stoppages`).
        let (first, dry) = match first.strip_suffix("+dry") {
            Some(s) => (s, true),
            None => (first, false),
        };
        // `ak74+open`: work the action, to check the moving parts.
        // `mp133+jam`: a double feed, to see it drawn in the port.
        let (first, jam) = match first.strip_suffix("+jam") {
            Some(s) => (s, true),
            None => (first, false),
        };
        *jamming = jam;
        let (subject, open) = match first.strip_suffix("+open") {
            Some(s) => (s.to_owned(), true),
            None => (first.to_owned(), false),
        };
        let turn: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let dist: Option<f32> = parts.next().and_then(|v| v.parse().ok());
        let shift: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        // The camera as `render` places it.
        // As the camera places itself: y 0 stands you there, eyes 1.7 m up.
        let ground = zone.ground(cam.x, cam.z);
        let eye_y = if cam.y == 0.0 {
            ground + 1.7
        } else {
            cam.y.max(ground + 0.3)
        };
        let eye = Vec3::new(cam.x, eye_y, cam.z);
        let rot = Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
        let fwd = rot * Vec3::NEG_Z;
        let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
        // Show the right side: the subject's +X faces the camera.
        let x = -flat;
        let facing = Quat::from_mat3(&Mat3::from_cols(x, Vec3::Y, x.cross(Vec3::Y)))
            * Quat::from_rotation_y(turn.to_radians());
        let spawn_static = |commands: &mut Commands, id: &str, at: Vec3| -> Option<Entity> {
            let kind = catalog.kind(id)?;
            let e = spawn_item(
                commands,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_translation(at).with_rotation(facing),
                    condition: 1.0,
                },
            );
            commands.entity(e).insert(RigidBody::Static);
            Some(e)
        };
        let job = |at: Vec3| crate::robots::Job {
            kind: crate::robots::JobKind::Survey,
            site: at,
            duration: 1.0e6,
            exit: at,
        };
        match subject.as_str() {
            "drone" => {
                let at = eye + fwd * dist.unwrap_or(1.6);
                let d = crate::robots::drone::spawn_drone(&mut commands, at, job(at));
                commands.entity(d).insert((
                    RigidBody::Static,
                    Transform::from_translation(at).with_rotation(facing),
                ));
            }
            "walker" => {
                let at = eye + flat * dist.unwrap_or(3.5);
                let w = crate::robots::walker::spawn_walker(&mut commands, &zone, at, job(at));
                let ground = zone.ground(at.x, at.z);
                commands.entity(w).insert(
                    Transform::from_translation(Vec3::new(at.x, ground + 1.15, at.z))
                        .with_rotation(facing),
                );
            }
            s if s == "scopeview" || s.starts_with("scopeview:") => {
                // The scope held to the eye: its eyepiece just in front of
                // the camera, looking where the camera looks.
                // `scopeview:pso_scope,SIDE,RELIEF`: the eye SIDE mm right
                // of the axis and RELIEF mm behind the eyepiece (70).
                let id = s.strip_prefix("scopeview:").unwrap_or("pu_scope");
                let look =
                    Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
                let eye_piece = Vec3::new(0.0, 0.0, 0.2);
                let relief = dist.map_or(0.07, |d| d / 1000.0);
                let at = eye + look * (Vec3::NEG_Z * relief - eye_piece - Vec3::X * turn / 1000.0);
                if let Some(e) = spawn_static(&mut commands, id, at) {
                    commands
                        .entity(e)
                        .insert(Transform::from_translation(at).with_rotation(look));
                }
            }
            "pylon" => {
                // Fully up, as if its builders had just finished.
                let at = eye + flat * dist.unwrap_or(9.0);
                let at = zone.on_ground(at.x, at.z);
                let p = crate::robots::pylon::spawn_pylon(&mut commands, at, 999.0, at);
                commands.entity(p).insert(crate::robots::pylon::Pylon {
                    progress: 1.0,
                    state: crate::robots::pylon::PylonState::Running { left: 999.0 },
                    report_cooldown: 0.0,
                    age: 0.0,
                });
            }
            "dirigible" | "grappler" | "pod" => {
                let at = eye + fwd * dist.unwrap_or(30.0);
                match subject.as_str() {
                    "dirigible" => {
                        crate::robots::dirigible::spawn_dirigible(&mut commands, at, job(at));
                    }
                    "grappler" => {
                        let at = eye + flat * dist.unwrap_or(3.5);
                        crate::robots::grappler::spawn_grappler(&mut commands, &zone, at, job(at));
                    }
                    _ => {
                        let at = eye + flat * dist.unwrap_or(4.0);
                        let y = crate::robots::dirigible::pod_rest(|x, z| zone.ground(x, z), at);
                        let at = Vec3::new(at.x, y, at.z);
                        commands.spawn((
                            crate::robots::dirigible::CargoPod {
                                carrier: Entity::PLACEHOLDER,
                            },
                            Transform::from_translation(at).with_rotation(facing),
                            Visibility::default(),
                        ));
                    }
                }
            }
            "hands" => {
                // Hands, palm down, fingers away: open, fist, on a pistol
                // grip (index along the frame), and a left hand pinching.
                let poses = [
                    [0.0, 0.0, 0.0, 0.0, 0.0],
                    [1.0, 1.0, 1.0, 1.0, 0.85],
                    [0.22, 0.74, 0.76, 0.78, 0.62],
                    [0.85, 0.9, 0.9, 0.9, 0.8],
                ];
                let right = Vec3::new(-flat.z, 0.0, flat.x);
                for (i, curl) in poses.into_iter().enumerate() {
                    let at = eye + flat * dist.unwrap_or(0.45) + right * (i as f32 - 1.5) * 0.24
                        - Vec3::Y * 0.08;
                    let h = crate::render::props::spawn_hand(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        if i == 3 {
                            crate::rig::Side::Left
                        } else {
                            crate::rig::Side::Right
                        },
                        curl,
                    );
                    // Fingers away from the camera, palm down, turned a
                    // little to show the curl.
                    let look = Transform::from_translation(at).looking_to(flat, Vec3::Y);
                    commands.entity(h).insert(
                        look.with_rotation(
                            look.rotation * Quat::from_rotation_y(turn.to_radians()),
                        ),
                    );
                }
            }
            // `sights:ak74`: the gun at the cheek, its rear sight just in
            // front of the camera (`sights_to_the_eye` places it).
            s if s.starts_with("sights:") => {
                spawn_static(&mut commands, &s["sights:".len()..], eye + fwd * 0.5);
            }
            // `first_person_hands` and `wear_kit` have these.
            s if s.starts_with("fp:") || s.starts_with("kit") => {}
            // A Mosin as made, and under it one sawn down to an obrez at
            // the vice (`weapons::gunsmith`), and an AK with and without
            // its stock.
            "obrez" => {
                use extraction_core::gunsmith::Build;
                let obrez = Build {
                    barrel_mm: Some(250.0),
                    stock: false,
                    ..Default::default()
                };
                let bare = Build {
                    stock: false,
                    ..Default::default()
                };
                let d = dist.unwrap_or(1.3);
                for (id, up, build) in [
                    ("mosin", 0.32, None),
                    ("mosin", 0.1, Some(obrez)),
                    ("ak74", -0.12, None),
                    ("ak74", -0.32, Some(bare)),
                ] {
                    let at = eye + fwd * d + Vec3::Y * up;
                    if let (Some(e), Some(b)) = (spawn_static(&mut commands, id, at), build) {
                        worked.push((e, b));
                    }
                }
            }
            // A gun's safety, close up: the same gun on safe (above) and
            // off it (below), its trigger guard in front of the eye.
            s if s.starts_with("safety:") => {
                let id = &s["safety:".len()..];
                let d = dist.unwrap_or(0.35);
                for (up, how) in [(0.06, 0), (-0.06, 3)] {
                    let at = eye + fwd * d + Vec3::Y * up;
                    if let Some(e) = spawn_static(&mut commands, id, at) {
                        told.push((e, how));
                    }
                }
            }
            // The tells (`weapons::legibility`): a Mosin as it lies and
            // one cocked (its cocking piece back), the double as it lies
            // and cocked (its indicators up), an AK new and one worn out
            // (rust through the finish).
            "tells" => {
                let d = dist.unwrap_or(1.2);
                // (0 as it lies, 1 cocked, 2 worn out.)
                for (id, up, how) in [
                    ("mosin", 0.32, 0),
                    ("mosin", 0.16, 1),
                    ("toz_double", 0.0, 0),
                    ("toz_double", -0.16, 1),
                    ("ak74", -0.32, 0),
                    ("ak74", -0.5, 2),
                ] {
                    let at = eye + fwd * d + Vec3::Y * up;
                    if let Some(e) = spawn_static(&mut commands, id, at) {
                        told.push((e, how));
                    }
                }
            }
            // An SKS and a TOZ double as made, each over itself cut down
            // at the vice: barrel short and stock off.
            "sawn" => {
                use extraction_core::gunsmith::Build;
                let cut = |mm: f32| Build {
                    barrel_mm: Some(mm),
                    stock: false,
                    ..Default::default()
                };
                let d = dist.unwrap_or(1.2);
                for (id, up, build) in [
                    ("sks", 0.32, None),
                    ("sks", 0.12, Some(cut(260.0))),
                    ("toz_double", -0.1, None),
                    ("toz_double", -0.3, Some(cut(300.0))),
                ] {
                    let at = eye + fwd * d + Vec3::Y * up;
                    if let (Some(e), Some(b)) = (spawn_static(&mut commands, id, at), build) {
                        worked.push((e, b));
                    }
                }
            }
            "guns" => {
                for (i, id) in ["pm_pistol", "ak74", "mosin", "toz_double"]
                    .iter()
                    .enumerate()
                {
                    let at = eye + fwd * dist.unwrap_or(1.7) + Vec3::Y * (0.45 - i as f32 * 0.28);
                    spawn_static(&mut commands, id, at);
                }
            }
            id => {
                let default = match id {
                    "pm_pistol" => 0.4,
                    "ak74" | "toz_double" => 1.1,
                    "mosin" => 1.3,
                    _ => 0.3,
                };
                let at = eye + fwd * dist.unwrap_or(default) - facing * Vec3::Z * shift;
                let Some(gun) = spawn_static(&mut commands, id, at) else {
                    warn!("gallery: no item {id}");
                    return;
                };
                let family = catalog
                    .kind(id)
                    .and_then(|k| catalog.get(k).firearm.as_ref())
                    .and_then(|f| f.magazine_family.clone());
                let mag = family.and_then(|fam| {
                    catalog
                        .iter()
                        .find(|(_, d)| {
                            d.magazine
                                .as_ref()
                                .is_some_and(|m| m.fits == fam && !m.stripper)
                        })
                        .map(|(_, d)| d.id.clone())
                });
                if open || jam {
                    opening.push(gun);
                } else if let Some(mag) = mag.and_then(|m| spawn_static(&mut commands, &m, at)) {
                    pending_mags.push((gun, mag));
                    if dry {
                        told.push((gun, 4));
                    }
                }
            }
        }
    }
    // Seat magazines once their guns have wells; open actions.
    if *stage > 3 {
        told.retain(|(e, how)| {
            let Ok(mut g) = guns.get_mut(*e) else {
                return true;
            };
            match how {
                1 => g.firearm.hammers_cocked.iter_mut().for_each(|h| *h = true),
                2 => g.firearm.condition = 0.08,
                3 => {
                    while g.firearm.fire_mode() == extraction_core::firearm::FireMode::Safe {
                        g.firearm.cycle_mode();
                    }
                }
                // Run dry once its magazine's in: the last round gone, the
                // slide back on the empty magazine.
                4 => {
                    let f = &mut g.firearm;
                    let Some(m) = f.magazine.as_mut() else {
                        return true;
                    };
                    m.rounds = 0;
                    f.chambers.iter_mut().for_each(|c| *c = None);
                    f.bolt_travel = 1.0;
                    f.locked_back = true;
                }
                _ => {}
            }
            false
        });
        worked.retain(|(e, b)| {
            let Ok(mut g) = guns.get_mut(*e) else {
                return true;
            };
            g.firearm.build = *b;
            false
        });
        opening.retain(|&e| {
            let Ok(mut gun) = guns.get_mut(e) else {
                return true;
            };
            let f = &mut gun.firearm;
            if *jamming {
                f.malfunction = Some(extraction_core::firearm::Malfunction::DoubleFeed);
                f.bolt_travel = 0.4;
                return false;
            }
            let _ = f.set_handle_lift(1.0);
            let _ = f.set_bolt_travel(1.0);
            let _ = f.set_break_open(true);
            // And a bipod swung down.
            if let Ok(mut b) = bipods.get_mut(e) {
                b.deployed = true;
            }
            false
        });
        pending_mags.retain(|&(gun, mag)| {
            let Some(well) = guns.get(gun).ok().and_then(|g| g.magwell) else {
                return true;
            };
            let Ok(mut socket) = sockets.get_mut(well) else {
                return true;
            };
            crate::interact::seat_in_socket(&mut commands, mag, well, &mut socket);
            events.write(crate::interact::InteractEvent::Socketed {
                item: mag,
                socket: well,
            });
            false
        });
    }
}

/// Where `--gallery kit+armour` puts a piece of armour.
#[derive(Clone, Copy)]
enum KitPlace {
    Body(extraction_core::inventory::SlotKind),
    Pocket(Entity, extraction_core::firearm_terminal::ArmourArea),
}

/// `--gallery fp:WHAT`: the player's gloves as you'd see them, at eye
/// height, held still: `fp:open` (both hands open), `fp:point` (the right
/// index on the nearest push button), or `fp:<item id>` (a gun on its grip
/// in the right hand, the left on its handguard if it has one;
/// `fp:<gun>+handle` puts the right hand on its slide or bolt; anything
/// else in the left fist); `fp:WHAT,ZOOM,TURN,TRIGGER` brings it all
/// closer, turns it about the right hand and pulls the trigger. Each glove takes the curl the game gives that
/// hold (`render::props::hold_curl`).
///
/// ```text
/// scripts/shot.sh target/shots/fp.png 40,0,262,0,-10 20 --gallery fp:ak74
/// ```
fn first_person_hands(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    catalog: Res<crate::items::Catalog>,
    grabbables: Query<&crate::interact::Grabbable>,
    guns: Query<&crate::weapons::Gun>,
    buttons: Query<&GlobalTransform, With<crate::interact::poke::PokeButton>>,
    mut players: Query<&mut Visibility, With<crate::render::props::PlayerGlove>>,
    mut stage: Local<u32>,
    mut waiting: Local<Option<(Entity, Transform)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use crate::interact::GripRole;
    use crate::render::props::{POINTING, hold_curl, spawn_hand};
    use crate::rig::Side;
    let (Some(spec), Some((cam, yaw, pitch)), Some(zone)) = (&opts.gallery, opts.camera, zone)
    else {
        return;
    };
    let Some(what) = spec.split(',').next().and_then(|s| s.strip_prefix("fp:")) else {
        return;
    };
    // `fp:<gun>+handle`: the right hand on its slide, charging handle or
    // bolt instead of the grip.
    let (what, on_handle) = match what.strip_suffix("+handle") {
        Some(w) => (w, true),
        None => (what, false),
    };
    *stage += 1;
    for mut v in &mut players {
        *v = Visibility::Hidden;
    }
    let eye = Vec3::new(cam.x, cam.y.max(zone.ground(cam.x, cam.z) + 0.3), cam.z);
    let rot = Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
    let fwd = rot * Vec3::NEG_Z;
    let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
    let right = flat.cross(Vec3::Y);
    // `fp:WHAT,ZOOM` brings everything ZOOM times closer, for a close look.
    let zoom: f32 = spec
        .split(',')
        .nth(1)
        .and_then(|z| z.parse().ok())
        .unwrap_or(1.0);
    // `fp:WHAT,ZOOM,TURN` turns it all TURN degrees about the right hand,
    // to see a grip from the side.
    let turn: f32 = spec
        .split(',')
        .nth(2)
        .and_then(|z| z.parse().ok())
        .unwrap_or(0.0);
    // `fp:WHAT,ZOOM,TURN,TRIGGER` pulls the gun hand's trigger (0..1).
    let trigger: f32 = spec
        .split(',')
        .nth(3)
        .and_then(|z| z.parse().ok())
        .unwrap_or(0.0);
    // `fp:WHAT,ZOOM,TURN,TRIGGER,TILT` then tips it TILT degrees about the
    // view's left-right axis, its underside up to the eye (a supporting
    // hand under a handguard).
    let tilt: f32 = spec
        .split(',')
        .nth(4)
        .and_then(|z| z.parse().ok())
        .unwrap_or(0.0);
    let pivot = eye + (flat * 0.42 + right * 0.1 - Vec3::Y * 0.22) / zoom;
    let view = move |t: Transform| {
        let q = Quat::from_axis_angle(right, tilt.to_radians())
            * Quat::from_rotation_y(turn.to_radians());
        Transform::from_translation(pivot + q * (t.translation - pivot))
            .with_rotation(q * t.rotation)
    };
    // Hands out in front at about a forearm's reach, low, pointing ahead
    // and a little down and in, as they rest on a controller.
    let rest = |side: Side| {
        let s = side.sign();
        let at = eye + (flat * 0.36 + right * (0.15 * s) - Vec3::Y * 0.27) / zoom;
        let aim = Quat::from_axis_angle(Vec3::Y, -0.18 * s) * Quat::from_axis_angle(right, -0.1);
        Transform::from_translation(at)
            .with_rotation(aim * Transform::IDENTITY.looking_to(flat, Vec3::Y).rotation)
    };
    let mut glove = |commands: &mut Commands, side: Side, at: Transform, curl: [f32; 5]| {
        let h = spawn_hand(commands, &mut meshes, &mut materials, side, curl);
        commands.entity(h).insert(view(at));
        // The left wrist's display, as the game mounts it.
        if side == Side::Left {
            commands.spawn((
                crate::render::legibility::TextSource::Dev,
                crate::raid::bench::ScreenText {
                    text: "SURGE 12M  14.2KG\nHP 100".into(),
                    size: crate::render::comfort::WRIST_SIZE,
                    style: crate::raid::bench::ScreenStyle::Lcd,
                },
                crate::render::props::wrist_mount(crate::rig::Side::Left),
                ChildOf(h),
            ));
        }
    };
    // Late enough for a map's cards and buttons to be up.
    if *stage == 15 {
        match what {
            "open" => {
                glove(&mut commands, Side::Left, rest(Side::Left), [0.0; 5]);
                glove(&mut commands, Side::Right, rest(Side::Right), [0.0; 5]);
            }
            "point" => {
                glove(&mut commands, Side::Left, rest(Side::Left), [0.15; 5]);
                let mut at = rest(Side::Right);
                if let Some(b) = buttons.iter().min_by(|a, b| {
                    let d = |g: &GlobalTransform| g.translation().distance(eye);
                    d(a).total_cmp(&d(b))
                }) {
                    // Fingertip a centimetre off the cap, pressing straight in.
                    let normal = b.rotation() * Vec3::Z;
                    at.rotation = Transform::IDENTITY.looking_to(-normal, Vec3::Y).rotation;
                    at.translation = b.translation() + normal * 0.012
                        - at.rotation * crate::interact::poke::FINGERTIP;
                }
                glove(&mut commands, Side::Right, at, POINTING);
            }
            id => {
                let Some(kind) = catalog.kind(id) else {
                    warn!("gallery: no item {id}");
                    return;
                };
                let e = crate::items::spawn_item(
                    &mut commands,
                    &catalog,
                    crate::items::SpawnItem {
                        kind,
                        transform: rest(Side::Left),
                        condition: 1.0,
                    },
                );
                commands.entity(e).insert(RigidBody::Static);
                // A gun is held level, a hand's width right of centre.
                let level = Transform::from_translation(
                    eye + (flat * 0.42 + right * 0.1 - Vec3::Y * 0.22) / zoom,
                )
                .looking_to(flat, Vec3::Y);
                *waiting = Some((e, level));
            }
        }
    }
    // Put the item in the hands once its grab points are there.
    let Some((item, level)) = *waiting else {
        return;
    };
    let Ok(grab) = grabbables.get(item) else {
        return;
    };
    *waiting = None;
    let point = |role: GripRole| grab.points.iter().find(|p| p.role == role);
    if let Some(primary) = point(GripRole::Primary) {
        let item_at =
            Transform::from_matrix(level.to_matrix() * primary.local.to_matrix().inverse());
        commands.entity(item).insert(view(item_at));
        let mut curl = hold_curl(Some(GripRole::Primary), trigger, 0.0);
        if let Ok(gun) = guns.get(item) {
            curl[0] = crate::render::props::gun_index_curl(Side::Right, &gun.layout, trigger);
        }
        match grab
            .points
            .iter()
            .find(|p| matches!(p.role, GripRole::Handle(_)))
            .filter(|_| on_handle)
        {
            Some(handle) => glove(
                &mut commands,
                Side::Right,
                match guns.get(item).ok().and_then(|g| g.layout.handle.as_ref()) {
                    Some(h) => {
                        item_at * crate::render::props::handle_pose(h, 0.0, 0.0, Side::Right)
                    }
                    None => Transform::from_translation((item_at * handle.local).translation)
                        .with_rotation(level.rotation),
                },
                hold_curl(Some(handle.role), 0.0, 0.0),
            ),
            None => glove(&mut commands, Side::Right, level, curl),
        }
        match point(GripRole::Support) {
            Some(support) => glove(
                &mut commands,
                Side::Left,
                item_at
                    * support.local
                    * guns.get(item).map_or(Transform::IDENTITY, |g| {
                        crate::render::props::support_pose(&g.layout, Side::Left)
                    }),
                if guns.get(item).is_ok_and(|g| g.layout.support.is_none()) {
                    crate::render::props::CUP_CURL
                } else {
                    hold_curl(Some(GripRole::Support), 0.0, 0.0)
                },
            ),
            None => glove(&mut commands, Side::Left, rest(Side::Left), [0.15; 5]),
        }
    } else {
        // Upright in the left fist (round the hand's origin, as a grip is),
        // the right open.
        let thinnest = catalog.kind(what).map_or(0.04, |k| {
            let def = catalog.get(k);
            def.dims.map_or((def.bulk / 1000.0).cbrt(), |d| {
                d.into_iter().fold(f32::MAX, f32::min)
            })
        });
        let hand = rest(Side::Left);
        commands.entity(item).insert(view(hand));
        glove(
            &mut commands,
            Side::Left,
            hand,
            hold_curl(None, 0.0, thinnest),
        );
        glove(&mut commands, Side::Right, rest(Side::Right), [0.15; 5]);
    }
}

/// `--gallery kit` fills the worn slots, to see the rig full: the pistol
/// holstered, magazines in three of the four pouches, a clip in one hip
/// pocket, the compass on the chest mount. `kit+pack` also hangs the pack
/// open in front; `kit+armour` puts on the vest, with a plate in each
/// pocket, and the helmet.
fn wear_kit(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    catalog: Res<crate::items::Catalog>,
    mut slots: Query<(
        Entity,
        &crate::interact::body::BodySlot,
        &mut crate::interact::Socket,
    )>,
    mut pockets: Query<
        (
            Entity,
            &crate::weapons::armour::PlatePocket,
            &crate::interact::SocketOwner,
            &mut crate::interact::Socket,
        ),
        Without<crate::interact::body::BodySlot>,
    >,
    players: Query<Entity, With<crate::rig::LocalPlayer>>,
    mut events: MessageWriter<crate::interact::InteractEvent>,
    mut toggle: MessageWriter<crate::interact::body::ToggleBackpack>,
    mut stage: Local<u32>,
    mut pending: Local<Vec<(Entity, usize)>>,
    mut armour: Local<Vec<(Entity, KitPlace)>>,
) {
    let Some(spec) = opts.gallery.as_deref() else {
        return;
    };
    let Some(rest) = spec.split(',').next().and_then(|s| s.strip_prefix("kit")) else {
        return;
    };
    *stage += 1;
    if *stage == 15 {
        let fill = [
            (2, "pm_pistol"),
            (3, "ak_mag"),
            (4, "ak_mag"),
            (6, "pm_mag"),
            (8, "mosin_clip"),
            (9, "compass"),
        ];
        for (slot, id) in fill {
            let Some(kind) = catalog.kind(id) else {
                continue;
            };
            let e = crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::default(),
                    condition: 1.0,
                },
            );
            pending.push((e, slot));
        }
        if rest.contains("+armour") {
            let mut spawn = |id: &str| {
                catalog.kind(id).map(|kind| {
                    crate::items::spawn_item(
                        &mut commands,
                        &catalog,
                        crate::items::SpawnItem {
                            kind,
                            transform: Transform::default(),
                            condition: 1.0,
                        },
                    )
                })
            };
            if let (Some(vest), Some(helmet), Some(front), Some(back)) = (
                spawn("vest"),
                spawn("helmet"),
                spawn("steel_plate"),
                spawn("steel_plate"),
            ) {
                use extraction_core::firearm_terminal::ArmourArea;
                use extraction_core::inventory::SlotKind;
                armour.extend([
                    (vest, KitPlace::Body(SlotKind::Vest)),
                    (helmet, KitPlace::Body(SlotKind::Head)),
                    (front, KitPlace::Pocket(vest, ArmourArea::Front)),
                    (back, KitPlace::Pocket(vest, ArmourArea::Back)),
                ]);
            }
        }
        if rest.contains("+pack") {
            if let Some(player) = players.iter().next() {
                toggle.write(crate::interact::body::ToggleBackpack { player });
            }
        }
    }
    // Seat each once the item exists.
    if *stage > 15 {
        pending.retain(|&(item, slot)| {
            let Some((socket_e, _, mut socket)) = slots.iter_mut().find(|(_, s, _)| s.0 == slot)
            else {
                return true;
            };
            crate::interact::seat_in_socket(&mut commands, item, socket_e, &mut socket);
            events.write(crate::interact::InteractEvent::Socketed {
                item,
                socket: socket_e,
            });
            false
        });
        // The vest's pockets come with it, a frame after it's made.
        armour.retain(|&(item, place)| {
            let found = match place {
                KitPlace::Body(kind) => slots
                    .iter_mut()
                    .find(|(_, _, s)| s.kind == crate::interact::SocketKind::Body(kind))
                    .map(|(e, _, s)| (e, s)),
                KitPlace::Pocket(vest, area) => pockets
                    .iter_mut()
                    .find(|(_, p, o, _)| o.0 == vest && p.0 == area)
                    .map(|(e, _, _, s)| (e, s)),
            };
            let Some((socket_e, mut socket)) = found else {
                return true;
            };
            crate::interact::seat_in_socket(&mut commands, item, socket_e, &mut socket);
            events.write(crate::interact::InteractEvent::Socketed {
                item,
                socket: socket_e,
            });
            false
        });
    }
}

/// `--gallery carry` (or `drag`, `pack`; with `side` to see it from
/// beside you): your hands on things (`interact::carry`). `carry`: a car
/// door taken by its two ends off the ground and held up across you;
/// `drag`: taken by its near end in the right hand and drawn back, the
/// far end on the ground; `pack`: your pack off the left shoulder, hanging
/// open in front of you, a dressing in it and the right hand reaching in.
#[allow(clippy::too_many_arguments)]
fn carry_for_gallery(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    catalog: Res<crate::items::Catalog>,
    mut players: Query<
        (Entity, &Transform, &mut crate::rig::RigInput, &Children),
        With<crate::rig::LocalPlayer>,
    >,
    torsos: Query<&Children, With<crate::interact::body::Torso>>,
    bags: Query<Entity, With<crate::interact::body::Backpack>>,
    mut actions: MessageWriter<crate::interact::HandAction>,
    mut state: Local<(u32, Option<Entity>)>,
) {
    use crate::rig::Side;
    let Some(spec) = opts.gallery.as_deref() else {
        return;
    };
    let (carry, drag, pack) = (
        spec.contains("carry"),
        spec.contains("drag"),
        spec.contains("pack"),
    );
    if !carry && !drag && !pack {
        return;
    }
    let Some((player, root, mut input, children)) = players.iter_mut().next() else {
        return;
    };
    state.0 += 1;
    let f = state.0;
    let local = |v: Vec3| Transform::from_translation(v);
    let hand = |input: &mut crate::rig::RigInput, side: Side, at: Vec3| {
        let h = input.hand_mut(side);
        h.pose = local(at);
        h.tracked = true;
    };
    if f == 10 && (carry || drag) {
        let id = catalog.kind("car_door").expect("a car door");
        // Flat on the ground ahead; across you to carry, pointing away to
        // drag.
        let turn = if carry {
            Quat::IDENTITY
        } else {
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
        };
        let at = root.mul_transform(
            Transform::from_xyz(0.0, 0.08, -0.6)
                .with_rotation(turn * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        );
        let mut c = commands.reborrow();
        state.1 = Some(crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: id,
                transform: at,
                condition: 0.6,
            },
        ));
    }
    if f == 10 && pack {
        let id = catalog.kind("field_dressing").expect("a dressing");
        let mut c = commands.reborrow();
        state.1 = Some(crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: id,
                transform: *root,
                condition: 1.0,
            },
        ));
    }
    let Some(thing) = state.1 else {
        return;
    };
    if carry {
        // Down to its ends, take hold, and up.
        let (l, r) = if f < 60 {
            (Vec3::new(-0.44, 0.12, -0.6), Vec3::new(0.44, 0.12, -0.6))
        } else {
            (Vec3::new(-0.42, 0.95, -0.42), Vec3::new(0.42, 0.95, -0.42))
        };
        hand(&mut input, Side::Left, l);
        hand(&mut input, Side::Right, r);
        if f == 40 {
            actions.write(crate::interact::HandAction::Grab {
                player,
                side: Side::Right,
                item: thing,
                point: None,
            });
        }
        if f == 45 {
            actions.write(crate::interact::HandAction::Grab {
                player,
                side: Side::Left,
                item: thing,
                point: None,
            });
        }
    } else if drag {
        let near = Vec3::new(0.0, 0.12, -0.16);
        let back = Vec3::new(0.25, 0.8, 0.15);
        let t = ((f as f32 - 50.0) / 80.0).clamp(0.0, 1.0);
        hand(&mut input, Side::Right, near.lerp(back, t));
        if f == 40 {
            actions.write(crate::interact::HandAction::Grab {
                player,
                side: Side::Right,
                item: thing,
                point: None,
            });
        }
    } else if pack {
        let Some(bag) = children
            .iter()
            .filter_map(|c| torsos.get(c).ok())
            .flat_map(|k| k.iter())
            .find(|e| bags.contains(*e))
        else {
            return;
        };
        if f == 20 {
            // A dressing in the pack.
            commands.queue(move |world: &mut World| {
                if let Some(mut b) = world.get_mut::<crate::interact::body::Backpack>(bag) {
                    b.contents.push(thing);
                }
                world.entity_mut(thing).insert((
                    crate::interact::body::InBackpack { bag },
                    ChildOf(bag),
                    Transform::from_xyz(0.05, 0.12, 0.0),
                    avian3d::prelude::RigidBodyDisabled,
                    avian3d::prelude::ColliderDisabled,
                    Visibility::Hidden,
                ));
            });
        }
        let shoulder = Vec3::new(-0.15, 1.6, 0.12);
        let front = Vec3::new(-0.05, 1.2, -0.42);
        let t = ((f as f32 - 40.0) / 30.0).clamp(0.0, 1.0);
        hand(&mut input, Side::Left, shoulder.lerp(front, t));
        hand(&mut input, Side::Right, Vec3::new(0.15, 1.0, -0.38));
        if f == 30 {
            actions.write(crate::interact::HandAction::Grab {
                player,
                side: Side::Left,
                item: bag,
                point: None,
            });
        }
    }
}

/// `--gallery climb` (with `side` to see it from beside you): a wall 2 m
/// high ahead, both hands gripping its top's near edge
/// (`interact::hand_climb`), drawn down: you pull yourself up it. A flat
/// rig's hands don't grip the world themselves, so the holds are put on
/// them here; the climbing is the game's.
#[allow(clippy::type_complexity)]
fn climb_for_gallery(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    mut players: Query<
        (Entity, &Transform, &mut crate::rig::RigInput, &Children),
        With<crate::rig::LocalPlayer>,
    >,
    hands: Query<(Entity, &crate::rig::RigHand)>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: Local<(u32, f32)>,
) {
    use crate::interact::hand_climb::{HandHold, Hold};
    use crate::rig::Side;
    if !opts.gallery.as_deref().is_some_and(|s| s.contains("climb")) {
        return;
    }
    let Some((_, root, mut input, children)) = players.iter_mut().next() else {
        return;
    };
    state.0 += 1;
    let f = state.0;
    if f == 10 {
        state.1 = root.translation.y;
        commands.spawn((
            Name::new("Gallery wall"),
            avian3d::prelude::RigidBody::Static,
            avian3d::prelude::Collider::cuboid(2.4, 2.0, 0.25),
            avian3d::prelude::CollisionLayers::new(
                crate::world::Layer::World,
                avian3d::prelude::LayerMask::ALL,
            ),
            root.mul_transform(Transform::from_xyz(0.0, 1.0, -0.48)),
            Mesh3d(meshes.add(Cuboid::new(2.4, 2.0, 0.25))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.46, 0.44, 0.4),
                perceptual_roughness: 0.95,
                ..default()
            })),
        ));
    }
    if f < 10 {
        return;
    }
    let top = state.1 + 2.0;
    // Up on the edge, then drawn down a metre over a second and a half.
    let pull = ((f as f32 - 40.0) / 96.0).clamp(0.0, 1.0);
    let up = top + 0.03 - root.translation.y - pull * 1.0;
    for side in [Side::Left, Side::Right] {
        let h = input.hand_mut(side);
        h.pose = Transform::from_xyz(0.22 * side.sign(), up, -0.38)
            .with_rotation(Quat::from_rotation_x(-0.4));
        h.tracked = true;
        h.grip = 1.0;
    }
    if f == 30 {
        for c in children.iter() {
            if let Ok((e, rh)) = hands.get(c) {
                let at = root.transform_point(Vec3::new(
                    0.22 * rh.side.sign(),
                    top + 0.03 - root.translation.y,
                    -0.38,
                ));
                commands.entity(e).insert(HandHold {
                    on: Hold::Edge { top },
                    at,
                    since: time.elapsed_secs(),
                });
            }
        }
    }
}

/// `--gallery fight` (with `side` to see it from beside you): a crawler
/// a metre ahead (`robots::indoor`), a crowbar raised in the right hand
/// to strike, the left palm laid on the machine (`interact::fight`).
#[allow(clippy::too_many_arguments)]
fn fight_for_gallery(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    catalog: Res<crate::items::Catalog>,
    zone: Option<Res<crate::world::Zone>>,
    mut players: Query<
        (Entity, &Transform, &mut crate::rig::RigInput),
        With<crate::rig::LocalPlayer>,
    >,
    parts: Query<(&crate::robots::RobotPart, &GlobalTransform)>,
    mut actions: MessageWriter<crate::interact::HandAction>,
    mut state: Local<(u32, Option<Entity>, Option<Entity>)>,
) {
    use crate::rig::Side;
    if !opts.gallery.as_deref().is_some_and(|s| s.contains("fight")) {
        return;
    }
    let (Some((player, root, mut input)), Some(zone)) = (players.iter_mut().next(), zone) else {
        return;
    };
    state.0 += 1;
    let f = state.0;
    if f == 10 {
        let at = root.transform_point(Vec3::new(0.0, 0.0, -1.0));
        let at = Vec3::new(at.x, zone.ground(at.x, at.z), at.z);
        state.1 = Some(crate::robots::walker::spawn_walker_sized(
            &mut commands,
            &zone,
            at,
            crate::robots::Job {
                kind: crate::robots::JobKind::Watch,
                site: at,
                duration: f32::INFINITY,
                exit: at,
            },
            false,
            crate::robots::indoor::CRAWLER_HP,
            crate::robots::indoor::CRAWLER_SIZE,
            None,
        ));
        let bar = catalog.kind("crowbar").expect("a crowbar");
        state.2 = Some(crate::items::spawn_item(
            &mut commands,
            &catalog,
            crate::items::SpawnItem {
                kind: bar,
                transform: root.mul_transform(Transform::from_xyz(0.3, 1.2, -0.35)),
                condition: 0.7,
            },
        ));
    }
    let (Some(crawler), Some(bar)) = (state.1, state.2) else {
        return;
    };
    // The right hand on the bar, raised; the left on the crawler's body.
    let raised = Vec3::new(0.28, 1.5, -0.3);
    let body = parts
        .iter()
        .filter(|(p, _)| p.robot == crawler)
        .map(|(_, g)| g.translation())
        .max_by(|a, b| a.y.total_cmp(&b.y));
    let palm = body
        .map(|b| root.compute_affine().inverse().transform_point3(b) + Vec3::new(-0.05, 0.05, 0.22))
        .unwrap_or(Vec3::new(-0.2, 0.8, -0.7));
    let right = if f < 30 {
        Vec3::new(0.3, 1.2, -0.35)
    } else {
        raised
    };
    for (side, at, rot) in [
        (Side::Right, right, Quat::from_rotation_x(0.9)),
        (Side::Left, palm, Quat::from_rotation_x(-0.3)),
    ] {
        let h = input.hand_mut(side);
        h.pose = Transform::from_translation(at).with_rotation(rot);
        h.tracked = true;
    }
    if f == 20 {
        actions.write(crate::interact::HandAction::Grab {
            player,
            side: Side::Right,
            item: bar,
            point: None,
        });
    }
}

/// `--gallery spall`: a car door, a plank wall and a plaster wall side on,
/// a couple of metres ahead, one over another, each shot through from
/// behind and to the left by a walker's round every frame: the spall out of
/// the face toward the camera.
fn spall_for_gallery(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    mut shots: MessageWriter<crate::weapons::ballistics::Shot>,
    mut state: Local<(u32, Option<Entity>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use crate::render::materials::Mat;
    if opts.gallery.as_deref() != Some("spall") {
        return;
    }
    let (Some((cam, yaw, pitch)), Some(zone)) = (opts.camera, zone) else {
        return;
    };
    let eye = if cam.y == 0.0 {
        Vec3::new(cam.x, zone.ground(cam.x, cam.z) + 1.7, cam.z)
    } else {
        cam
    };
    let rot = Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
    let (fwd, right) = (rot * Vec3::NEG_Z, rot * Vec3::X);
    // The rounds come square on to each wall, across and back toward the
    // camera, so the face they leave by is the one it sees.
    let line = (right - fwd * 0.7).normalize();
    let walls = [
        (
            Mat::PaintedMetal,
            0.092,
            0.45,
            Color::srgb(0.35, 0.42, 0.38),
        ),
        (Mat::Planks, 0.1, 0.0, Color::srgb(0.45, 0.33, 0.2)),
        (Mat::Plaster, 0.15, -0.45, Color::srgb(0.75, 0.72, 0.66)),
    ];
    state.0 += 1;
    if state.0 == 3 {
        for (mat, thick, up, colour) in walls {
            let half = Vec3::new(0.45, 0.19, thick * 0.5);
            commands.spawn((
                Name::new("Gallery wall"),
                crate::world::surfaces::Surfaces(vec![(half, Transform::IDENTITY, mat)]),
                Transform::from_translation(eye + fwd * 2.2 + Vec3::Y * (up - 0.3))
                    .looking_to(line, Vec3::Y),
                Mesh3d(meshes.add(Cuboid::from_size(half * 2.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: colour,
                    perceptual_roughness: 0.8,
                    ..default()
                })),
                RigidBody::Static,
                Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
                CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            ));
        }
        state.1 = Some(commands.spawn(Name::new("Walker's gun")).id());
    }
    let Some(gun) = state.1 else {
        return;
    };
    if state.0 > 5 {
        for (i, (_, _, up, _)) in walls.iter().enumerate() {
            shots.write(crate::weapons::ballistics::Shot {
                gun,
                shooter: Some(gun),
                origin: eye + fwd * 2.2 + Vec3::Y * (up - 0.3) - line * 4.0,
                direction: line,
                speed: 450.0,
                damage: 14.0,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: u64::from(state.0) << 4 | i as u64,
            });
        }
    }
}

/// `--gallery pm_pistol+present`: the gun with its mag release held, the
/// magazine out of its catch and hanging out of the well for the other
/// hand (`weapons::reload::Presented`).
fn present_for_gallery(
    opts: Res<LaunchOptions>,
    guns: Query<(Entity, &crate::weapons::Gun)>,
    mut present: MessageWriter<crate::weapons::reload::PresentMagazine>,
    mut frames: Local<u32>,
) {
    if !opts
        .gallery
        .as_deref()
        .is_some_and(|g| g.contains("+present"))
    {
        return;
    }
    *frames += 1;
    // Once the gallery's magazine is seated.
    if *frames == 12 {
        for (e, g) in &guns {
            if g.firearm.magazine.is_some() {
                present.write(crate::weapons::reload::PresentMagazine { gun: e });
            }
        }
    }
}

/// `--gallery plate`: five pistol rounds from the camera into the steel
/// plate of the range target nearest it, one a frame, spread over its
/// face: the sparks and the marks they leave (or, on a plate with no
/// collider, nothing).
fn plate_hits_for_gallery(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    targets: Query<(&crate::world::buildings::Building, &GlobalTransform)>,
    mut shots: MessageWriter<crate::weapons::ballistics::Shot>,
    mut impacts: MessageReader<crate::weapons::impacts::Impact>,
    mut state: Local<(u32, Option<Entity>)>,
) {
    if opts.gallery.as_deref() != Some("plate") {
        return;
    }
    for im in impacts.read() {
        info!(
            "gallery plate: {:?} at {} (marked: {})",
            im.kind, im.point, im.decal
        );
    }
    let (Some((cam, ..)), Some(zone)) = (opts.camera, zone) else {
        return;
    };
    let eye = if cam.y == 0.0 {
        Vec3::new(cam.x, zone.ground(cam.x, cam.z) + 1.7, cam.z)
    } else {
        cam
    };
    state.0 += 1;
    if state.0 == 3 {
        state.1 = Some(commands.spawn(Name::new("Gallery pistol")).id());
    }
    let Some(gun) = state.1 else {
        return;
    };
    let Some(plate) = targets
        .iter()
        .filter(|(b, _)| b.prefab == "target")
        .min_by(|a, b| {
            a.1.translation()
                .distance(eye)
                .total_cmp(&b.1.translation().distance(eye))
        })
    else {
        return;
    };
    const ON_FACE: [(f32, f32); 5] = [
        (0.0, 0.0),
        (0.15, 0.1),
        (-0.12, 0.14),
        (-0.1, -0.15),
        (0.14, -0.12),
    ];
    let i = state.0 as usize;
    // Late enough that the yard's colliders are in.
    const FROM: usize = 60;
    if (FROM..FROM + ON_FACE.len()).contains(&i) {
        let (x, y) = ON_FACE[i - FROM];
        let at = plate.1.transform_point(Vec3::new(x, 1.2 + y, 0.08));
        // From in front of the camera's own body, which would take them.
        let dir = (at - eye).normalize();
        shots.write(crate::weapons::ballistics::Shot {
            gun,
            shooter: Some(gun),
            origin: eye + dir * 0.6,
            direction: dir,
            speed: 315.0,
            damage: 20.0,
            projectiles: 1,
            spread: 0.0,
            recoil: 0.0,
            seed: i as u64,
        });
    }
}

/// `--gallery treat` (with `side` or `down`, `pose_body_for_gallery`):
/// a body hurt and treated (`interact::wounds`, drawn by
/// `render::wounds`). An open wound on the right thigh and the left
/// forearm, a dressing round the right upper arm, a splint strapped on the
/// right shin, and the left glove, turned palm up, bloody from the wound.
/// What a dressing round that arm says is said (the upper arm's bound, the
/// thigh still bleeds); with `wrist` the left wrist is raised to the eye
/// instead, its screen showing it, and BLEED.
#[allow(clippy::too_many_arguments)]
fn treat_for_gallery(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    mut players: Query<
        (
            Entity,
            &Transform,
            &mut crate::rig::RigInput,
            &mut crate::health::Health,
            Option<&mut crate::interact::wounds::Wounds>,
        ),
        With<crate::rig::LocalPlayer>,
    >,
    hands: Query<(Entity, &crate::rig::RigHand)>,
    raid: Option<ResMut<crate::raid::Raid>>,
    mut frames: Local<u32>,
) {
    use crate::interact::wounds::{Bloodied, Fracture, Limb, Wound};
    use crate::rig::Side;
    if !opts.gallery.as_deref().is_some_and(|s| s.contains("treat")) {
        return;
    }
    let Some((player, root, mut input, mut health, wounds)) = players.iter_mut().next() else {
        return;
    };
    let Some(mut wounds) = wounds else {
        return;
    };
    *frames += 1;
    let wound = |limb, along, around, bleed, dressed| Wound {
        limb,
        along,
        around,
        bleed,
        dressed,
    };
    // Round a limb hanging down, the angle that faces out to your right
    // and a little forward (`Segment::point`'s frame).
    let x = Vec3::NEG_Y.any_orthonormal_vector();
    let y = Vec3::NEG_Y.cross(x);
    let out = root.rotation * Vec3::new(1.0, 0.0, -0.5).normalize();
    let facing = out.dot(y).atan2(out.dot(x));
    wounds.0 = vec![
        wound(Limb::Thigh(Side::Right), 0.45, facing, 1.0, false),
        wound(Limb::Forearm(Side::Left), 0.5, 0.5, 0.6, false),
        wound(Limb::UpperArm(Side::Right), 0.55, 0.0, 0.0, true),
        wound(Limb::Shin(Side::Right), 0.5, 0.0, 0.0, false),
    ];
    // Bleeding as the wounds have it, and in no danger of going down.
    health.bleeding = 1.6;
    health.hp = health.max;
    if *frames == 2 {
        commands.entity(player).insert(Fracture {
            leg: Side::Right,
            splinted: true,
        });
        for (h, rh) in &hands {
            if rh.side == Side::Left {
                commands.entity(h).insert(Bloodied(1.0));
            }
        }
    }
    // Said, and said again whenever it's gone from the log (a line lasts
    // seconds; frames here are slow), so the wrist shows it.
    if let Some(mut raid) = raid {
        let line = crate::interact::wounds::dressed_line(
            Some(Limb::UpperArm(Side::Right)),
            Some(&wounds),
            health.bleeding,
        );
        if !raid.log.iter().any(|(t, _)| *t == line) {
            raid.say(line);
        }
    }
    let left = input.hand_mut(Side::Left);
    left.pose = if opts.gallery.as_deref().is_some_and(|s| s.contains("wrist")) {
        // Across the chest, fingers to the right, the back of the wrist
        // (its screen) turned up to the eye.
        Transform::from_xyz(-0.08, 1.2, -0.32).with_rotation(
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
                * Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)
                * Quat::from_rotation_y(0.4),
        )
    } else {
        // Low and turned palm up (its palm faces +X, inward).
        Transform::from_xyz(-0.16, 1.1, -0.36).with_rotation(
            Quat::from_rotation_x(-0.5) * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        )
    };
    left.tracked = true;
}

/// `--gallery kit+stoop` (or any spec with `stoop`): after a moment
/// standing, the head drops 0.35 m and comes 0.25 m forward, looking down,
/// as you bend to your belt (the torso keeps its standing height until the
/// head has dropped further). `behind` puts the camera behind and above
/// the player instead, to see the pack on their back; `above` high behind,
/// looking down at the feet, to see the body's shadow. `down`: the head
/// tips forward and looks down at the body, as a head does (the camera is
/// the eye); `side` puts the camera 2 m to the right, to see the whole
/// body (`render::body`); `crouch` squats. The free camera otherwise stays where
/// `--camera` put it.
fn pose_body_for_gallery(
    opts: Res<LaunchOptions>,
    comfort: Option<ResMut<crate::rig::comfort::Comfort>>,
    mut players: Query<(&Transform, &mut crate::rig::RigInput), With<crate::rig::LocalPlayer>>,
    mut cams: Query<
        &mut Transform,
        (
            With<crate::render::FreeCamera>,
            Without<crate::rig::LocalPlayer>,
        ),
    >,
    mut frames: Local<u32>,
) {
    let Some(spec) = opts.gallery.as_deref() else {
        return;
    };
    // A gallery of the body (or of wounds on it) draws it, whatever the
    // setting (off by default).
    if let Some(mut c) = comfort
        && (spec.contains("body") || spec.contains("treat"))
        && !c.body
    {
        c.body = true;
    }
    let (stoop, behind, above) = (
        spec.contains("stoop"),
        spec.contains("behind"),
        spec.contains("above"),
    );
    let (down, side) = (spec.contains("down"), spec.contains("side"));
    let crouch = spec.contains("crouch");
    if !stoop && !behind && !above && !down && !side && !crouch {
        return;
    }
    *frames += 1;
    let Some((root, mut input)) = players.iter_mut().next() else {
        return;
    };
    if stoop && *frames > 20 {
        let fwd = input.head.rotation * Vec3::NEG_Z;
        let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
        input.head.translation += flat * 0.25 - Vec3::Y * 0.35;
        input.head.rotation =
            Transform::IDENTITY.looking_to(flat, Vec3::Y).rotation * Quat::from_rotation_x(-1.1);
    }
    if crouch && *frames > 20 {
        // Squatting, eyes 0.85 m up, looking down past the knees.
        let fwd = input.head.rotation * Vec3::NEG_Z;
        let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
        input.head.translation =
            Vec3::new(input.head.translation.x, 0.85, input.head.translation.z) + flat * 0.12;
        input.head.rotation =
            Transform::IDENTITY.looking_to(flat, Vec3::Y).rotation * Quat::from_rotation_x(-0.9);
    }
    if down && *frames > 20 {
        // A head looking down pivots on the neck: the eyes come forward.
        let fwd = input.head.rotation * Vec3::NEG_Z;
        let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
        // (The flat rig writes the head afresh each frame, as for `stoop`.)
        input.head.translation += flat * 0.08 - Vec3::Y * 0.04;
        input.head.rotation =
            Transform::IDENTITY.looking_to(flat, Vec3::Y).rotation * Quat::from_rotation_x(-1.05);
    }
    let head = root.mul_transform(input.head);
    let Ok(mut cam) = cams.single_mut() else {
        return;
    };
    if side {
        let fwd = head.rotation * Vec3::NEG_Z;
        let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
        let right = flat.cross(Vec3::Y);
        let mid = root.translation + Vec3::Y * 0.9;
        *cam = Transform::from_translation(mid + right * 2.2 + flat * 0.6 + Vec3::Y * 0.2)
            .looking_at(mid, Vec3::Y);
    } else if above {
        // High behind, looking down at the feet: the body's shadow.
        let fwd = head.rotation * Vec3::NEG_Z;
        let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
        let feet = root.translation;
        *cam = Transform::from_translation(feet - flat * 1.8 + Vec3::Y * 3.2)
            .looking_at(feet, Vec3::Y);
    } else if behind {
        let fwd = head.rotation * Vec3::NEG_Z;
        let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
        let at = head.translation - flat * 1.3 + Vec3::Y * 0.25;
        *cam =
            Transform::from_translation(at).looking_at(head.translation - Vec3::Y * 0.45, Vec3::Y);
    } else if *frames > 20 {
        *cam = head;
    }
}
