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
                pose_body_for_gallery.after(crate::rig::RigDriverSystems),
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

/// `--gallery sights:<gun>`: the gun's sight line on the camera's, its
/// rear sight just ahead of the eye, and the eye down behind the sights
/// (so the front post goes soft, as `weapons::optics` draws it).
fn sights_to_the_eye(
    opts: Res<LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    mut guns: Query<(
        &crate::weapons::Gun,
        &mut Transform,
        &mut crate::weapons::feel::Handling,
    )>,
) {
    let (Some(spec), Some((cam, yaw, pitch)), Some(zone)) = (&opts.gallery, opts.camera, zone)
    else {
        return;
    };
    if !spec.starts_with("sights:") {
        return;
    }
    let ground = zone.ground(cam.x, cam.z);
    let eye_y = if cam.y == 0.0 {
        ground + 1.7
    } else {
        cam.y.max(ground + 0.3)
    };
    let eye = Vec3::new(cam.x, eye_y, cam.z);
    let look = Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
    for (gun, mut tf, mut handling) in &mut guns {
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
    let pivot = eye + (flat * 0.42 + right * 0.1 - Vec3::Y * 0.22) / zoom;
    let view = move |t: Transform| {
        let q = Quat::from_rotation_y(turn.to_radians());
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
                Transform::from_translation((item_at * handle.local).translation)
                    .with_rotation(level.rotation),
                hold_curl(Some(handle.role), 0.0, 0.0),
            ),
            None => glove(&mut commands, Side::Right, level, curl),
        }
        match point(GripRole::Support) {
            Some(support) => glove(
                &mut commands,
                Side::Left,
                item_at * support.local,
                hold_curl(Some(GripRole::Support), 0.0, 0.0),
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

/// `--gallery kit+stoop` (or any spec with `stoop`): after a moment
/// standing, the head drops 0.35 m and comes 0.25 m forward, looking down,
/// as you bend to your belt (the torso keeps its standing height until the
/// head has dropped further). `behind` puts the camera behind and above
/// the player instead, to see the pack on their back; `above` high behind,
/// looking down at the feet, to see the body's shadow. `down`: the head
/// tips forward and looks down at the body, as a head does (the camera is
/// the eye); `side` puts the camera 2 m to the right, to see the whole
/// body (`render::body`). The free camera otherwise stays where
/// `--camera` put it.
fn pose_body_for_gallery(
    opts: Res<LaunchOptions>,
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
    let (stoop, behind, above) = (
        spec.contains("stoop"),
        spec.contains("behind"),
        spec.contains("above"),
    );
    let (down, side) = (spec.contains("down"), spec.contains("side"));
    if !stoop && !behind && !above && !down && !side {
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
