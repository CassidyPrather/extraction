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
            .add_systems(Update, (count_frames, screenshot_when_ready, gallery));
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

fn screenshot_when_ready(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    mut count: ResMut<FrameCount>,
) {
    let Some(path) = &opts.screenshot else {
        return;
    };
    if count.requested || count.frames < opts.frames {
        return;
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
        // `ak74+open`: work the action, to check the moving parts.
        let (subject, open) = match first.strip_suffix("+open") {
            Some(s) => (s.to_owned(), true),
            None => (first.to_owned(), false),
        };
        let turn: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let dist: Option<f32> = parts.next().and_then(|v| v.parse().ok());
        let shift: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        // The camera as `render` places it.
        let eye = Vec3::new(cam.x, cam.y.max(zone.ground(cam.x, cam.z) + 0.3), cam.z);
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
                        let at = Vec3::new(at.x, zone.ground(at.x, at.z) + 0.45, at.z);
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
                if open {
                    opening.push(gun);
                } else if let Some(mag) = mag.and_then(|m| spawn_static(&mut commands, &m, at)) {
                    pending_mags.push((gun, mag));
                }
            }
        }
    }
    // Seat magazines once their guns have wells; open actions.
    if *stage > 3 {
        opening.retain(|&e| {
            let Ok(mut gun) = guns.get_mut(e) else {
                return true;
            };
            let f = &mut gun.firearm;
            let _ = f.set_handle_lift(1.0);
            let _ = f.set_bolt_travel(1.0);
            let _ = f.set_break_open(true);
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
