//! The flat-screen driver: mouse and keyboard pretend to be a headset.
//!
//! Mouse turns the body (yaw) and tilts the head (pitch). Hands hang in a
//! comfortable "ready" pose in front of the chest and follow the gaze.
//!
//! WASD move, Shift sprint, Space jump, C crouch. Click to capture the
//! mouse, Escape to release it.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use super::{LocalPlayer, RigInput, Side};

pub struct FlatDriverPlugin;

impl Plugin for FlatDriverPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlatLook>().add_systems(
            PreUpdate,
            (grab_cursor, drive_flat_rig)
                .chain()
                .in_set(super::RigDriverSystems)
                .run_if(super::flat_driving),
        );
    }
}

/// Accumulated head pitch and the standing eye height.
#[derive(Resource)]
pub struct FlatLook {
    pub pitch: f32,
    pub sensitivity: f32,
    pub eye_height: f32,
    pub crouch_height: f32,
}

impl Default for FlatLook {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            sensitivity: 0.0022,
            eye_height: 1.68,
            crouch_height: 1.05,
        }
    }
}

fn grab_cursor(
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) && cursor.grab_mode == CursorGrabMode::None {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn drive_flat_rig(
    mut players: Query<
        (&mut RigInput, &mut Transform),
        (With<LocalPlayer>, Without<crate::bot::Bot>),
    >,
    mut look: ResMut<FlatLook>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let captured = cursor
        .single()
        .map(|c| c.grab_mode != CursorGrabMode::None)
        .unwrap_or(false);
    for (mut input, mut root) in &mut players {
        if captured {
            let d = motion.delta * look.sensitivity;
            root.rotate_y(-d.x);
            look.pitch = (look.pitch - d.y).clamp(-1.5, 1.5);
        }
        let mut axis = Vec2::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            axis.y += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            axis.y -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            axis.x += 1.0;
        }
        if keys.pressed(KeyCode::KeyA) {
            axis.x -= 1.0;
        }
        input.locomotion = axis.clamp_length_max(1.0);
        input.sprint = keys.pressed(KeyCode::ShiftLeft);
        input.jump = keys.just_pressed(KeyCode::Space);
        if keys.just_pressed(KeyCode::KeyC) {
            input.crouch = !input.crouch;
        }
        input.turn = 0.0;
        let eye = if input.crouch {
            look.crouch_height
        } else {
            look.eye_height
        };
        let head_rot = Quat::from_rotation_x(look.pitch);
        input.head = Transform::from_xyz(0.0, eye, 0.0).with_rotation(head_rot);
        // Hands in a low ready position, swinging with the gaze.
        for side in Side::BOTH {
            let offset = Vec3::new(0.2 * side.sign(), -0.32, -0.38);
            let pose = Transform::from_translation(input.head.translation + head_rot * offset)
                .with_rotation(head_rot * Quat::from_rotation_x(-0.3));
            let hand = input.hand_mut(side);
            hand.pose = pose;
            hand.tracked = true;
        }
    }
}
