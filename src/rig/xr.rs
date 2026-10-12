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
//! None of this can be exercised without a headset; it is written against
//! `bevy_mod_openxr` 0.6's API and its `raw_actions` example.

use bevy::prelude::*;
use bevy_mod_openxr::action_binding::{OxrSendActionBindings, OxrSuggestActionBinding};
use bevy_mod_openxr::action_set_attaching::OxrAttachActionSet;
use bevy_mod_openxr::action_set_syncing::{OxrActionSetSyncSet, OxrSyncActionSet};
use bevy_mod_openxr::openxr_session_running;
use bevy_mod_openxr::resources::OxrInstance;
use bevy_mod_openxr::session::OxrSession;
use bevy_mod_xr::camera::XrCamera;
use bevy_mod_xr::session::{XrSessionCreated, XrTrackingRoot, session_available};
use bevy_mod_xr::spaces::XrSpaceSyncSet;
use openxr::Posef;

use super::{LocalPlayer, RigDriverSystems, RigInput, Side};

pub struct XrDriverPlugin;

impl Plugin for XrDriverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_actions.run_if(session_available))
            .add_systems(OxrSendActionBindings, suggest_bindings)
            .add_systems(
                XrSessionCreated,
                (attach_actions, spawn_controller_spaces, make_tracking_root),
            )
            .add_systems(
                PreUpdate,
                sync_actions
                    .before(OxrActionSetSyncSet)
                    .run_if(openxr_session_running),
            )
            .add_systems(PostUpdate, play_haptics.run_if(openxr_session_running))
            .add_systems(
                PreUpdate,
                drive_xr_rig
                    .after(XrSpaceSyncSet)
                    .after(OxrActionSetSyncSet)
                    .in_set(RigDriverSystems)
                    .run_if(openxr_session_running),
            );
    }
}

#[derive(Resource)]
struct Actions {
    set: openxr::ActionSet,
    pose: [openxr::Action<Posef>; 2],
    grip: openxr::Action<f32>,
    trigger: openxr::Action<f32>,
    primary: openxr::Action<bool>,
    secondary: openxr::Action<bool>,
    stick: openxr::Action<openxr::Vector2f>,
    stick_click: openxr::Action<bool>,
    haptic: openxr::Action<openxr::Haptic>,
    left: openxr::Path,
    right: openxr::Path,
}

/// A controller's tracked space, child of the tracking root.
#[derive(Component)]
struct ControllerSpace(Side);

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
            grip: set.create_action("grip", "Grab", &both)?,
            trigger: set.create_action("trigger", "Trigger", &both)?,
            primary: set.create_action("primary", "Primary button", &both)?,
            secondary: set.create_action("secondary", "Secondary button", &both)?,
            stick: set.create_action("stick", "Thumbstick", &both)?,
            stick_click: set.create_action("stick_click", "Thumbstick click", &both)?,
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
    let grip = a.grip.as_raw();
    let trigger = a.trigger.as_raw();
    let primary = a.primary.as_raw();
    let secondary = a.secondary.as_raw();
    let stick = a.stick.as_raw();
    let click = a.stick_click.as_raw();
    let haptic = a.haptic.as_raw();
    for profile in [
        "/interaction_profiles/oculus/touch_controller",
        "/interaction_profiles/meta/touch_controller_plus",
        "/interaction_profiles/valve/index_controller",
        "/interaction_profiles/microsoft/motion_controller",
        "/interaction_profiles/htc/vive_controller",
    ] {
        bind(profile, pose_l, &["/user/hand/left/input/grip/pose"]);
        bind(profile, pose_r, &["/user/hand/right/input/grip/pose"]);
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
    }
}

/// The local player's root is the playspace; the flat camera goes away.
fn make_tracking_root(
    mut commands: Commands,
    players: Query<Entity, With<LocalPlayer>>,
    flat_cams: Query<Entity, (With<super::RigHead>, With<Camera3d>)>,
    mut driver: ResMut<super::RigDriver>,
) {
    for p in &players {
        commands.entity(p).insert(XrTrackingRoot);
    }
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

fn drive_xr_rig(
    actions: Option<Res<Actions>>,
    session: Res<OxrSession>,
    cameras: Query<&Transform, (With<XrCamera>, Without<LocalPlayer>)>,
    controllers: Query<(&ControllerSpace, &Transform), Without<LocalPlayer>>,
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
        let stick = read_v(path);
        let hand = input.hand_mut(side);
        hand.grip = grip;
        hand.trigger = trigger;
        hand.primary = primary;
        hand.secondary = secondary;
        hand.stick = stick;
        hand.stick_click = click && side == Side::Right;
    }
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
    for p in pulses.read() {
        let path = match p.side {
            Side::Left => a.left,
            Side::Right => a.right,
        };
        let vib = openxr::HapticVibration::new()
            .amplitude(p.amplitude.clamp(0.0, 1.0))
            .duration(openxr::Duration::from_nanos((p.seconds * 1e9) as i64))
            .frequency(openxr::FREQUENCY_UNSPECIFIED);
        if let Err(e) = a.haptic.apply_feedback(&session, path, &vib) {
            debug!("haptics: {e}");
        }
    }
}
