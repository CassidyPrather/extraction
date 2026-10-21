//! The OpenXR driver: headset and controllers into [`RigInput`].
//!
//! The local player's root becomes the XR tracking root, so the headset's
//! playspace *is* the player's playspace: XR cameras and controller spaces
//! are its children with tracked local transforms. This driver copies those
//! into the rig input and reads the controller actions.
//!
//! Default bindings (Touch / Index / WMR / Vive):
//! - grip: grab; trigger: trigger
//! - A/X: primary (mag release, break open); B/Y: secondary (slide stop)
//! - left stick: walk (click to sprint); right stick: snap turn left/right,
//!   down to crouch-toggle; right stick click: fire selector
//!
//! No fallback to the Khronos simple controller: offered one, SteamVR
//! drove Index controllers through it (one "select" button, no sticks), and
//! the first real playtest had only the grip. The log says which profile
//! the runtime picked for each hand (`input: ... profile`) and the first
//! time each control is used.
//!
//! A hand's pose is the grip pose's position with the aim pose's
//! orientation: OpenXR's grip orientation runs -Z through the fist and X
//! through the palm, while the game's hands, guns and the flat rig all take
//! -Z as where you point and +Y as up, which is what the aim pose is.
//!
//! Fingers come from `XR_EXT_hand_tracking` where the runtime has it
//! (`read_fingers`, `rig::fingers`).
//!
//! The headset path runs in CI-less checks against Monado's simulated
//! headset (`scripts/xr_check.sh`): instance, session, swapchains, actions
//! and what the eyes see. Real controllers still need a real headset.

use bevy::prelude::*;
use bevy_mod_openxr::action_binding::{OxrSendActionBindings, OxrSuggestActionBinding};
use bevy_mod_openxr::action_set_attaching::OxrAttachActionSet;
use bevy_mod_openxr::action_set_syncing::{OxrActionSetSyncSet, OxrSyncActionSet};
use bevy_mod_openxr::openxr_session_running;
use bevy_mod_openxr::resources::OxrInstance;
use bevy_mod_openxr::session::OxrSession;
use bevy_mod_xr::camera::XrCamera;
use bevy_mod_xr::hands::{HandBone, LeftHand, RightHand};
use bevy_mod_xr::session::{XrSessionCreated, XrTrackingRoot, session_available};
use bevy_mod_xr::spaces::{XrSpaceLocationFlags, XrSpaceSyncSet};
use openxr::Posef;

use super::{LocalPlayer, RigDriverSystems, RigInput, Side};

pub struct XrDriverPlugin;

impl Plugin for XrDriverPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SessionGate>()
            .add_systems(PreUpdate, handle_session)
            .add_systems(Startup, create_actions.run_if(session_available))
            .add_systems(Update, headset_missing)
            .add_systems(Startup, size_swapchain)
            .add_systems(Update, report_input.run_if(openxr_session_running))
            .add_systems(OxrSendActionBindings, suggest_bindings)
            .add_systems(
                XrSessionCreated,
                (attach_actions, spawn_controller_spaces, make_tracking_root),
            )
            .add_systems(Update, (ride_player, dress_eyes, follow_mirror))
            .add_systems(
                PreUpdate,
                sync_actions
                    .before(OxrActionSetSyncSet)
                    .run_if(openxr_session_running),
            )
            .add_systems(PostUpdate, play_haptics.run_if(openxr_session_running))
            .add_systems(
                PreUpdate,
                (drive_xr_rig, read_fingers)
                    .chain()
                    .after(XrSpaceSyncSet)
                    .after(OxrActionSetSyncSet)
                    .in_set(RigDriverSystems)
                    .run_if(openxr_session_running),
            );
    }
}

/// Longest the headset waits for the load before the session starts
/// anyway (a stream that never finishes mustn't leave the player in the
/// runtime's loader).
pub const SESSION_WAIT_MAX: f32 = 20.0;

/// Seconds the headset's been waiting for the load, once the runtime is
/// ready for a session (`None`: not waiting).
#[derive(Resource, Default)]
pub struct SessionGate {
    pub waiting: Option<f32>,
}

/// `bevy_mod_xr`'s `auto_handle_session` (switched off in `lib.rs`), but
/// the session is only created once the load has settled: no building
/// still waiting on its props, the forest built and its impostors taken,
/// or [`SESSION_WAIT_MAX`] seconds on. Until then the runtime shows its
/// own loader, and the first frame the player sees is a steady one (the
/// load's own frames ran 60-100 ms each for a second, docs/PERFORMANCE.md
/// "Hitches").
#[allow(clippy::too_many_arguments)]
pub fn handle_session(
    mut state_changed: MessageReader<bevy_mod_xr::session::XrStateChanged>,
    mut create: MessageWriter<bevy_mod_xr::session::XrCreateSessionMessage>,
    mut begin: MessageWriter<bevy_mod_xr::session::XrBeginSessionMessage>,
    mut end: MessageWriter<bevy_mod_xr::session::XrEndSessionMessage>,
    mut destroy: MessageWriter<bevy_mod_xr::session::XrDestroySessionMessage>,
    mut gate: ResMut<SessionGate>,
    pending: Query<(), With<crate::world::buildings::PropsPending>>,
    forest: crate::world::trees::ForestSettled,
    time: Res<Time<Real>>,
    mut no_auto_restart: Local<bool>,
) {
    use bevy_mod_xr::session::{XrState, XrStateChanged};
    for XrStateChanged(state) in state_changed.read() {
        match state {
            XrState::Available => {
                if !*no_auto_restart {
                    gate.waiting = Some(0.0);
                }
            }
            XrState::Ready => {
                begin.write_default();
            }
            XrState::Stopping => {
                end.write_default();
            }
            XrState::Exiting { should_restart } => {
                *no_auto_restart = !should_restart;
                destroy.write_default();
            }
            _ => (),
        }
    }
    let Some(waited) = gate.waiting.as_mut() else {
        return;
    };
    *waited += time.delta_secs();
    let settled = pending.is_empty() && forest.settled();
    if settled || *waited >= SESSION_WAIT_MAX {
        info!(
            "headset: starting the session after {:.1} s ({})",
            *waited,
            if settled {
                "the load has settled"
            } else {
                "gave up waiting on the load"
            }
        );
        create.write_default();
        gate.waiting = None;
    }
}

/// The eyes' resolution: the runtime's recommendation times the comfort
/// render scale (auto: about 1800 px across an eye, so a very high-res
/// headset on a laptop GPU isn't asked for 3000 px). Read when the session
/// is created, so a change takes effect on the next launch.
fn size_swapchain(
    instance: Option<Res<OxrInstance>>,
    system: Option<Res<bevy_mod_openxr::resources::OxrSystemId>>,
    comfort: Res<super::comfort::Comfort>,
    mut config: Option<ResMut<bevy_mod_openxr::resources::OxrSessionConfig>>,
) {
    let (Some(instance), Some(system), Some(config)) = (instance, system, config.as_mut()) else {
        return;
    };
    let Ok(views) = instance.enumerate_view_configuration_views(
        system.0,
        openxr::ViewConfigurationType::PRIMARY_STEREO,
    ) else {
        return;
    };
    let Some(v) = views.first() else {
        return;
    };
    let rec = UVec2::new(
        v.recommended_image_rect_width,
        v.recommended_image_rect_height,
    );
    let scale = comfort.render_scale_for(rec.x);
    let size = (rec.as_vec2() * scale)
        .round()
        .as_uvec2()
        .max(UVec2::splat(256));
    info!(
        "eyes: {}x{} ({:.0}% of the runtime's {}x{})",
        size.x,
        size.y,
        scale * 100.0,
        rec.x,
        rec.y
    );
    config.resolutions = Some(vec![size]);
}

/// What the controllers are, and the first time each control is used: so
/// the log of a playtest says whether the controls arrived at all.
fn report_input(
    actions: Option<Res<Actions>>,
    instance: Res<OxrInstance>,
    session: Res<OxrSession>,
    players: Query<&RigInput, With<LocalPlayer>>,
    mut profiles: Local<[String; 2]>,
    mut seen: Local<std::collections::HashSet<String>>,
    time: Res<Time>,
    mut next: Local<f32>,
) {
    let Some(a) = actions else {
        return;
    };
    if time.elapsed_secs() >= *next {
        *next = time.elapsed_secs() + 1.0;
        for (i, (side, path)) in [("left", a.left), ("right", a.right)]
            .into_iter()
            .enumerate()
        {
            let now = session
                .current_interaction_profile(path)
                .ok()
                .filter(|p| *p != openxr::Path::NULL)
                .and_then(|p| instance.path_to_string(p).ok())
                .unwrap_or_else(|| "none".into());
            if now != profiles[i] {
                info!("input: {side} hand profile {now}");
                profiles[i] = now;
            }
        }
    }
    let Some(input) = players.iter().next() else {
        return;
    };
    for side in [Side::Left, Side::Right] {
        let h = input.hand(side);
        for (name, on) in [
            ("grip", h.grip > 0.6),
            ("trigger", h.trigger > 0.6),
            ("primary", h.primary),
            ("secondary", h.secondary),
            ("stick", h.stick.length() > 0.5),
            ("stick click", h.stick_click),
        ] {
            let key = format!("{side:?} {name}");
            if on && seen.insert(key.clone()) {
                info!("input: {key} works");
            }
        }
    }
}

/// The headset was asked for and the runtime had none (not connected, or
/// the machine's active OpenXR runtime isn't the one driving it: a Meta
/// runtime left active on a SteamVR machine says "not available"). Reopen
/// once on SteamVR if it's installed and wasn't tried; otherwise say
/// what to do in the flat window, which is what's left.
fn headset_missing(
    mut commands: Commands,
    state: Option<Res<bevy_mod_xr::session::XrState>>,
    opts: Res<crate::launch::LaunchOptions>,
    mut done: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) {
    if *done {
        return;
    }
    let Some(state) = state else {
        return;
    };
    *done = true;
    if let Some(note) = super::runtime::NOTE.get() {
        info!("{note}");
    }
    if *state != bevy_mod_xr::session::XrState::Unavailable {
        return;
    }
    let tooling = opts.tooling();
    let tried = opts.steamvr || std::env::var_os("XR_RUNTIME_JSON").is_some();
    if !tooling && !tried {
        if let Some(steamvr) = super::runtime::steamvr_runtime() {
            warn!(
                "no headset on the active OpenXR runtime; SteamVR is installed ({}): \
                 reopening on SteamVR",
                steamvr.display()
            );
            match super::runtime::relaunch_on(&steamvr).and_then(|mut c| c.spawn()) {
                Ok(_) => {
                    exit.write(AppExit::Success);
                    return;
                }
                Err(e) => error!("couldn't reopen on SteamVR: {e}"),
            }
        }
    }
    let msg = if opts.steamvr {
        "NO HEADSET: SteamVR found none. Is it on and tracking? \
         Start SteamVR first, then the game. (Or play flat: --flat)"
    } else {
        "NO HEADSET: the OpenXR runtime found none. Start your headset's \
         runtime first; for SteamVR, set it as the OpenXR runtime \
         (SteamVR settings) or run with --steamvr. (Or play flat: --flat)"
    };
    error!("{msg}");
    commands.insert_resource(super::XrMissing(msg.into()));
}

#[derive(Resource)]
struct Actions {
    set: openxr::ActionSet,
    pose: [openxr::Action<Posef>; 2],
    aim: [openxr::Action<Posef>; 2],
    grip: openxr::Action<f32>,
    trigger: openxr::Action<f32>,
    primary: openxr::Action<bool>,
    secondary: openxr::Action<bool>,
    stick: openxr::Action<openxr::Vector2f>,
    stick_click: openxr::Action<bool>,
    /// The thumb resting on a face button, the stick or the thumb rest
    /// (Index and Touch sense it).
    thumb_rest: openxr::Action<bool>,
    haptic: openxr::Action<openxr::Haptic>,
    left: openxr::Path,
    right: openxr::Path,
}

/// A controller's tracked grip space, child of the tracking root.
#[derive(Component)]
struct ControllerSpace(Side);

/// A controller's tracked aim space: where it points.
#[derive(Component)]
struct AimSpace(Side);

fn create_actions(instance: Res<OxrInstance>, mut commands: Commands) {
    let make = || -> openxr::Result<Actions> {
        let left = instance.string_to_path("/user/hand/left")?;
        let right = instance.string_to_path("/user/hand/right")?;
        let both = [left, right];
        let set = instance.create_action_set("extraction", "Extraction", 0)?;
        Ok(Actions {
            pose: [
                set.create_action("left_grip_pose", "Left grip pose", &[])?,
                set.create_action("right_grip_pose", "Right grip pose", &[])?,
            ],
            aim: [
                set.create_action("left_aim_pose", "Left aim pose", &[])?,
                set.create_action("right_aim_pose", "Right aim pose", &[])?,
            ],
            grip: set.create_action("grip", "Grab", &both)?,
            trigger: set.create_action("trigger", "Trigger", &both)?,
            primary: set.create_action("primary", "Primary button", &both)?,
            secondary: set.create_action("secondary", "Secondary button", &both)?,
            stick: set.create_action("stick", "Thumbstick", &both)?,
            stick_click: set.create_action("stick_click", "Thumbstick click", &both)?,
            thumb_rest: set.create_action("thumb_rest", "Thumb resting", &both)?,
            haptic: set.create_action("haptic", "Vibration", &both)?,
            set,
            left,
            right,
        })
    };
    match make() {
        Ok(a) => commands.insert_resource(a),
        Err(e) => error!("could not create XR actions: {e}"),
    }
}

fn suggest_bindings(
    actions: Option<Res<Actions>>,
    mut bindings: MessageWriter<OxrSuggestActionBinding>,
) {
    let Some(a) = actions else {
        return;
    };
    let mut bind = |profile: &str, action: openxr::sys::Action, paths: &[&str]| {
        bindings.write(OxrSuggestActionBinding {
            action,
            interaction_profile: profile.to_owned().into(),
            bindings: paths.iter().map(|p| (*p).to_owned().into()).collect(),
        });
    };
    let pose_l = a.pose[0].as_raw();
    let pose_r = a.pose[1].as_raw();
    let aim_l = a.aim[0].as_raw();
    let aim_r = a.aim[1].as_raw();
    let grip = a.grip.as_raw();
    let trigger = a.trigger.as_raw();
    let primary = a.primary.as_raw();
    let secondary = a.secondary.as_raw();
    let stick = a.stick.as_raw();
    let click = a.stick_click.as_raw();
    let thumb_rest = a.thumb_rest.as_raw();
    let haptic = a.haptic.as_raw();
    for profile in [
        // (Quest 3's Touch Plus is offered to apps as plain Touch unless
        // they enable its extension.)
        "/interaction_profiles/oculus/touch_controller",
        "/interaction_profiles/valve/index_controller",
        "/interaction_profiles/microsoft/motion_controller",
        "/interaction_profiles/htc/vive_controller",
    ] {
        bind(profile, pose_l, &["/user/hand/left/input/grip/pose"]);
        bind(profile, pose_r, &["/user/hand/right/input/grip/pose"]);
        bind(profile, aim_l, &["/user/hand/left/input/aim/pose"]);
        bind(profile, aim_r, &["/user/hand/right/input/aim/pose"]);
        bind(
            profile,
            haptic,
            &[
                "/user/hand/left/output/haptic",
                "/user/hand/right/output/haptic",
            ],
        );
        let touchlike = profile.contains("touch") || profile.contains("index");
        if profile.contains("vive") {
            bind(
                profile,
                grip,
                &[
                    "/user/hand/left/input/squeeze/click",
                    "/user/hand/right/input/squeeze/click",
                ],
            );
            bind(
                profile,
                trigger,
                &[
                    "/user/hand/left/input/trigger/value",
                    "/user/hand/right/input/trigger/value",
                ],
            );
            bind(
                profile,
                primary,
                &[
                    "/user/hand/left/input/menu/click",
                    "/user/hand/right/input/menu/click",
                ],
            );
            bind(
                profile,
                stick,
                &[
                    "/user/hand/left/input/trackpad",
                    "/user/hand/right/input/trackpad",
                ],
            );
            bind(
                profile,
                click,
                &[
                    "/user/hand/left/input/trackpad/click",
                    "/user/hand/right/input/trackpad/click",
                ],
            );
            continue;
        }
        if profile.contains("index") {
            bind(
                profile,
                grip,
                &[
                    "/user/hand/left/input/squeeze/value",
                    "/user/hand/right/input/squeeze/value",
                ],
            );
        } else if profile.contains("microsoft") {
            bind(
                profile,
                grip,
                &[
                    "/user/hand/left/input/squeeze/click",
                    "/user/hand/right/input/squeeze/click",
                ],
            );
        } else {
            bind(
                profile,
                grip,
                &[
                    "/user/hand/left/input/squeeze/value",
                    "/user/hand/right/input/squeeze/value",
                ],
            );
        }
        bind(
            profile,
            trigger,
            &[
                "/user/hand/left/input/trigger/value",
                "/user/hand/right/input/trigger/value",
            ],
        );
        bind(
            profile,
            stick,
            &[
                "/user/hand/left/input/thumbstick",
                "/user/hand/right/input/thumbstick",
            ],
        );
        bind(
            profile,
            click,
            &[
                "/user/hand/left/input/thumbstick/click",
                "/user/hand/right/input/thumbstick/click",
            ],
        );
        if touchlike {
            // Every touch sensor the thumb rests on.
            let rests: &[&str] = if profile.contains("index") {
                &[
                    "/user/hand/left/input/a/touch",
                    "/user/hand/left/input/b/touch",
                    "/user/hand/left/input/thumbstick/touch",
                    "/user/hand/left/input/trackpad/touch",
                    "/user/hand/right/input/a/touch",
                    "/user/hand/right/input/b/touch",
                    "/user/hand/right/input/thumbstick/touch",
                    "/user/hand/right/input/trackpad/touch",
                ]
            } else {
                &[
                    "/user/hand/left/input/x/touch",
                    "/user/hand/left/input/y/touch",
                    "/user/hand/left/input/thumbstick/touch",
                    "/user/hand/left/input/thumbrest/touch",
                    "/user/hand/right/input/a/touch",
                    "/user/hand/right/input/b/touch",
                    "/user/hand/right/input/thumbstick/touch",
                    "/user/hand/right/input/thumbrest/touch",
                ]
            };
            bind(profile, thumb_rest, rests);
            let (p, s) = if profile.contains("index") {
                ("a", "b")
            } else {
                ("x", "y")
            };
            if profile.contains("index") {
                bind(
                    profile,
                    primary,
                    &[
                        "/user/hand/left/input/a/click",
                        "/user/hand/right/input/a/click",
                    ],
                );
                bind(
                    profile,
                    secondary,
                    &[
                        "/user/hand/left/input/b/click",
                        "/user/hand/right/input/b/click",
                    ],
                );
            } else {
                bind(
                    profile,
                    primary,
                    &[
                        &format!("/user/hand/left/input/{p}/click"),
                        "/user/hand/right/input/a/click",
                    ],
                );
                bind(
                    profile,
                    secondary,
                    &[
                        &format!("/user/hand/left/input/{s}/click"),
                        "/user/hand/right/input/b/click",
                    ],
                );
            }
        } else {
            bind(
                profile,
                primary,
                &[
                    "/user/hand/left/input/menu/click",
                    "/user/hand/right/input/menu/click",
                ],
            );
        }
    }
}

fn attach_actions(actions: Option<Res<Actions>>, mut attach: MessageWriter<OxrAttachActionSet>) {
    if let Some(a) = actions {
        attach.write(OxrAttachActionSet(a.set.clone()));
    }
}

fn sync_actions(actions: Option<Res<Actions>>, mut sync: MessageWriter<OxrSyncActionSet>) {
    if let Some(a) = actions {
        sync.write(OxrSyncActionSet(a.set.clone()));
    }
}

fn spawn_controller_spaces(
    mut commands: Commands,
    actions: Option<Res<Actions>>,
    session: Res<OxrSession>,
) {
    let Some(a) = actions else {
        return;
    };
    for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
        match session.create_action_space(&a.pose[i], openxr::Path::NULL, Isometry3d::IDENTITY) {
            Ok(space) => {
                commands.spawn((
                    Name::new(format!("{side:?} controller")),
                    ControllerSpace(side),
                    space,
                    bevy_mod_xr::session::XrTracker,
                ));
            }
            Err(e) => error!("could not create {side:?} controller space: {e}"),
        }
        match session.create_action_space(&a.aim[i], openxr::Path::NULL, Isometry3d::IDENTITY) {
            Ok(space) => {
                commands.spawn((
                    Name::new(format!("{side:?} aim")),
                    AimSpace(side),
                    space,
                    bevy_mod_xr::session::XrTracker,
                ));
            }
            Err(e) => error!("could not create {side:?} aim space: {e}"),
        }
    }
}

/// Keeps the tracking root on the local player (whenever both exist).
fn ride_player(
    mut commands: Commands,
    players: Query<Entity, With<LocalPlayer>>,
    roots: Query<(Entity, Option<&ChildOf>), (With<XrTrackingRoot>, Without<LocalPlayer>)>,
) {
    let Some(p) = players.iter().next() else {
        return;
    };
    for (root, parent) in &roots {
        if parent.map(|c| c.parent()) != Some(p) {
            commands
                .entity(root)
                .insert((ChildOf(p), Transform::IDENTITY));
        }
    }
}

/// The headset's eyes get the game's look (fog, exposure, tonemapping),
/// which the backend's cameras don't come with, and a near plane close
/// enough for iron sights. The left eye is where shadow detail is measured
/// from: with no camera drawing to the window, Bevy would otherwise take
/// whichever camera is oldest (an impostor or scope camera, anywhere).
fn dress_eyes(
    mut commands: Commands,
    mut eyes: Query<(Entity, &XrCamera, &mut Projection), Added<XrCamera>>,
    weather: Res<crate::world::sky::Weather>,
) {
    for (e, eye, mut projection) in &mut eyes {
        if eye.0 == 0 {
            commands.entity(e).insert(bevy::camera::ShadowLodOrigin);
        }
        if let Projection::Custom(custom) = &mut *projection {
            if let Some(p) = custom.get_mut::<bevy_mod_xr::camera::XrProjection>() {
                p.near = crate::render::NEAR_CLIP;
            }
        }
        commands
            .entity(e)
            .insert(crate::render::camera_look(&weather));
    }
}

/// `--mirror`: a window view of what the left eye sees, for whoever is
/// watching the player (and for screenshots of the headset's view).
#[derive(Component)]
struct Mirror;

fn follow_mirror(
    mut commands: Commands,
    opts: Res<crate::launch::LaunchOptions>,
    weather: Res<crate::world::sky::Weather>,
    eyes: Query<(&XrCamera, &GlobalTransform)>,
    mut mirrors: Query<&mut Transform, With<Mirror>>,
) {
    if !opts.mirror {
        return;
    }
    let Some((_, eye)) = eyes.iter().find(|(c, _)| c.0 == 0) else {
        return;
    };
    let pose = eye.compute_transform();
    match mirrors.single_mut() {
        Ok(mut t) => *t = pose,
        Err(_) => {
            commands.spawn((
                Name::new("Mirror"),
                Mirror,
                pose,
                crate::render::camera_bundle(&weather),
            ));
        }
    }
}

/// The local player's root is the playspace; the flat camera goes away.
///
/// `bevy_mod_xr` makes its own tracking root at startup and parents the
/// eyes and controllers to it, and renders from the one root there is. So
/// that root rides on the player, rather than the player becoming a second
/// root (which left the eyes at the world origin, under the map).
fn make_tracking_root(
    mut commands: Commands,
    flat_cams: Query<Entity, (With<super::RigHead>, With<Camera3d>)>,
    mut driver: ResMut<super::RigDriver>,
) {
    for c in &flat_cams {
        commands.entity(c).remove::<Camera3d>().remove::<Camera>();
    }
    *driver = super::RigDriver::Xr;
    info!("XR session created: driving the rig from the headset");
}

#[derive(Default)]
struct TurnState {
    armed: bool,
    sprint: bool,
}

/// The fingers, where the runtime tracks them (`XR_EXT_hand_tracking`,
/// asked for at start-up: SteamVR fills it from Index controllers' finger
/// sensing). A hand with any joint untracked, or a runtime without the
/// extension, leaves `fingers` empty and the glove to its own poses.
fn read_fingers(
    bones: Query<(
        &HandBone,
        &Transform,
        &XrSpaceLocationFlags,
        Has<LeftHand>,
        Has<RightHand>,
    )>,
    mut players: Query<&mut RigInput, With<LocalPlayer>>,
    mut said: Local<bool>,
) {
    let Some(mut input) = players.iter_mut().next() else {
        return;
    };
    let mut joints = [[Transform::IDENTITY; super::fingers::JOINTS]; 2];
    let mut seen = [0usize; 2];
    for (bone, t, flags, left, right) in &bones {
        if !(flags.position_tracked && flags.rotation_tracked) || left == right {
            continue;
        }
        let side = if left { Side::Left } else { Side::Right };
        joints[side.index()][*bone as usize] = *t;
        seen[side.index()] += 1;
    }
    for side in Side::BOTH {
        let i = side.index();
        let fingers = (seen[i] == super::fingers::JOINTS)
            .then(|| super::fingers::fingers_from_joints(&joints[i]))
            .flatten();
        if fingers.is_some() && !*said {
            *said = true;
            info!("input: fingers tracked ({side:?} hand first)");
        }
        input.hand_mut(side).fingers = fingers;
    }
}

fn drive_xr_rig(
    actions: Option<Res<Actions>>,
    session: Res<OxrSession>,
    cameras: Query<&Transform, (With<XrCamera>, Without<LocalPlayer>)>,
    controllers: Query<(&ControllerSpace, &Transform), Without<LocalPlayer>>,
    aims: Query<(&AimSpace, &Transform), Without<LocalPlayer>>,
    mut players: Query<(&mut RigInput, &mut Transform), With<LocalPlayer>>,
    mut turn: Local<TurnState>,
    comfort: Res<super::comfort::Comfort>,
    time: Res<Time>,
) {
    let Some(a) = actions else {
        return;
    };
    let Some((mut input, mut root)) = players.iter_mut().next() else {
        return;
    };
    // Head: the average of the eye views.
    let eyes: Vec<&Transform> = cameras.iter().collect();
    if !eyes.is_empty() {
        let pos = eyes.iter().map(|t| t.translation).sum::<Vec3>() / eyes.len() as f32;
        input.head = Transform::from_translation(pos).with_rotation(eyes[0].rotation);
    }
    for (space, t) in &controllers {
        let hand = input.hand_mut(space.0);
        hand.pose = *t;
        // Pointing orientation where the runtime gives one (see the module
        // doc); an untracked aim (identity) leaves the grip's.
        if let Some((_, aim)) = aims.iter().find(|(s, _)| s.0 == space.0)
            && aim.rotation != Quat::IDENTITY
        {
            hand.pose.rotation = aim.rotation;
        }
        hand.tracked = true;
    }
    let read_f = |action: &openxr::Action<f32>, path: openxr::Path| {
        action
            .state(&session, path)
            .map(|s| s.current_state)
            .unwrap_or(0.0)
    };
    let read_b = |action: &openxr::Action<bool>, path: openxr::Path| {
        action
            .state(&session, path)
            .map(|s| s.current_state)
            .unwrap_or(false)
    };
    let read_v = |path: openxr::Path| {
        a.stick
            .state(&session, path)
            .map(|s| Vec2::new(s.current_state.x, s.current_state.y))
            .unwrap_or(Vec2::ZERO)
    };
    for (side, path) in [(Side::Left, a.left), (Side::Right, a.right)] {
        let grip = read_f(&a.grip, path);
        let trigger = read_f(&a.trigger, path);
        let primary = read_b(&a.primary, path);
        let secondary = read_b(&a.secondary, path);
        let click = read_b(&a.stick_click, path);
        let thumb_rest = read_b(&a.thumb_rest, path);
        let stick = read_v(path);
        let hand = input.hand_mut(side);
        hand.grip = grip;
        hand.trigger = trigger;
        hand.primary = primary;
        hand.secondary = secondary;
        hand.stick = stick;
        hand.stick_click = click && side == Side::Right;
        hand.thumb_rest = thumb_rest;
    }
    // Both sticks clicked, standing still: a playtest mark.
    input.mark = read_b(&a.stick_click, a.left)
        && read_b(&a.stick_click, a.right)
        && input.hand(Side::Left).stick.length() < 0.3;
    // Locomotion and comfort turning.
    let left_stick = input.hand(Side::Left).stick;
    if read_b(&a.stick_click, a.left) && left_stick.length() > 0.3 {
        turn.sprint = true;
    }
    if left_stick.length() < 0.2 {
        turn.sprint = false;
    }
    input.locomotion = if left_stick.length() > 0.15 {
        left_stick
    } else {
        Vec2::ZERO
    };
    input.sprint = turn.sprint;
    let right_stick = input.hand(Side::Right).stick;
    input.turn = 0.0;
    if right_stick.x.abs() < 0.3 && right_stick.y > -0.3 {
        turn.armed = true;
    }
    // Turn around the head, not the playspace origin: snaps, or smoothly.
    let angle = if comfort.smooth_turn {
        if right_stick.x.abs() > 0.25 {
            -right_stick.x * comfort.turn_speed.to_radians() * time.delta_secs()
        } else {
            0.0
        }
    } else if turn.armed && right_stick.x.abs() > 0.7 {
        turn.armed = false;
        -right_stick.x.signum() * comfort.snap_angle.to_radians()
    } else {
        0.0
    };
    if angle != 0.0 {
        let head_world = root.transform_point(input.head.translation);
        let rot = Quat::from_rotation_y(angle);
        root.translation = head_world + rot * (root.translation - head_world);
        root.rotation = rot * root.rotation;
    }
    if turn.armed && right_stick.y < -0.7 {
        turn.armed = false;
        input.crouch = !input.crouch;
    }
}

/// Game haptics onto the controllers.
fn play_haptics(
    actions: Option<Res<Actions>>,
    session: Res<OxrSession>,
    mut pulses: MessageReader<super::haptics::Haptic>,
) {
    let Some(a) = actions else {
        return;
    };
    // The runtime plays only the last pulse it's given per controller, so
    // a frame's pulses merge: the strongest, for the longest.
    let mut merged: [Option<(f32, f32)>; 2] = [None, None];
    for p in pulses.read() {
        let m = &mut merged[p.side as usize];
        let (a, t) = m.unwrap_or((0.0, 0.0));
        *m = Some((a.max(p.amplitude), t.max(p.seconds)));
    }
    for (side, m) in [Side::Left, Side::Right].into_iter().zip(merged) {
        let Some((amplitude, seconds)) = m else {
            continue;
        };
        let path = match side {
            Side::Left => a.left,
            Side::Right => a.right,
        };
        let vib = openxr::HapticVibration::new()
            .amplitude(amplitude.clamp(0.0, 1.0))
            .duration(openxr::Duration::from_nanos((seconds * 1e9) as i64))
            .frequency(openxr::FREQUENCY_UNSPECIFIED);
        if let Err(e) = a.haptic.apply_feedback(&session, path, &vib) {
            debug!("haptics: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_mod_xr::session::{XrCreateSessionMessage, XrState, XrStateChanged};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<XrStateChanged>()
            .add_message::<XrCreateSessionMessage>()
            .add_message::<bevy_mod_xr::session::XrBeginSessionMessage>()
            .add_message::<bevy_mod_xr::session::XrEndSessionMessage>()
            .add_message::<bevy_mod_xr::session::XrDestroySessionMessage>()
            .init_resource::<SessionGate>()
            .init_resource::<Created>()
            .add_systems(
                Update,
                (
                    handle_session,
                    |mut r: MessageReader<XrCreateSessionMessage>, mut n: ResMut<Created>| {
                        n.0 += r.read().count();
                    },
                )
                    .chain(),
            );
        app.world_mut()
            .write_message(XrStateChanged(XrState::Available));
        app
    }

    /// Sessions asked for so far.
    #[derive(Resource, Default)]
    struct Created(usize);

    fn created(app: &App) -> usize {
        app.world().resource::<Created>().0
    }

    /// The headset waits while a building waits on its props or the forest
    /// isn't built, and starts its session once both are done.
    #[test]
    fn the_session_waits_for_the_load() {
        let mut app = app();
        let building = app
            .world_mut()
            .spawn(crate::world::buildings::PropsPending(0))
            .id();
        for _ in 0..5 {
            app.update();
            assert_eq!(created(&app), 0, "not while a building waits on props");
        }
        app.world_mut().despawn(building);
        for _ in 0..5 {
            app.update();
            assert_eq!(created(&app), 0, "not before the forest is built");
        }
        app.world_mut()
            .insert_resource(crate::world::trees::ForestBuilt);
        app.update();
        assert_eq!(created(&app), 1, "settled: the session starts");
        for _ in 0..5 {
            app.update();
            assert_eq!(created(&app), 1, "once");
        }
    }

    /// A load that never settles still lets the player in.
    #[test]
    fn the_session_gives_up_waiting() {
        let mut app = app();
        app.world_mut()
            .spawn(crate::world::buildings::PropsPending(0));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs(1),
        ));
        let mut at = None;
        for i in 0..40 {
            app.update();
            if created(&app) > 0 {
                at = Some(i);
                break;
            }
        }
        let at = at.expect("the session started in the end");
        assert!(at >= 19, "not before {SESSION_WAIT_MAX} s (frame {at})");
    }
}
