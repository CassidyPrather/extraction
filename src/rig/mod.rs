//! The rig: a head and two hands, and the input that drives them.
//!
//! Every player is a rig. A *driver* writes the rig's [`RigInput`] each
//! frame — the OpenXR driver from a headset, the flat driver from mouse and
//! keyboard, the bot driver from the simulation's brain — and everything
//! downstream (locomotion, grabbing, guns, the backpack) reads only that.
//! Poses are in *playspace*: relative to the player root, which sits at the
//! feet and carries the body's yaw.

pub mod comfort;
pub mod flat;
#[cfg(feature = "xr")]
pub mod xr;

use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub const BOTH: [Self; 2] = [Self::Left, Self::Right];

    pub fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// +1 for right, -1 for left: mirror offsets with it.
    pub fn sign(self) -> f32 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }
}

/// One controller's worth of input.
#[derive(Clone, Copy, Debug, Default, Reflect)]
pub struct HandInput {
    /// Pose of the grip point (the middle of a closed fist), in playspace.
    pub pose: Transform,
    /// 0..=1 grip squeeze.
    pub grip: f32,
    /// 0..=1 trigger pull.
    pub trigger: f32,
    /// A / X: the "do the thing" face button (mag release, eject).
    pub primary: bool,
    /// B / Y: the second face button (slide stop, menu).
    pub secondary: bool,
    pub stick: Vec2,
    pub stick_click: bool,
    /// Tracking is valid this frame.
    pub tracked: bool,
}

/// What the rig wants this frame.
#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct RigInput {
    /// Head pose in playspace.
    pub head: Transform,
    pub hands: [HandInput; 2],
    /// Locomotion: x = strafe right, y = forward, relative to the head's yaw.
    pub locomotion: Vec2,
    /// Smooth turn rate (radians/s) or a one-shot snap (radians), applied to the root.
    pub turn: f32,
    pub sprint: bool,
    pub jump: bool,
    pub crouch: bool,
}

impl RigInput {
    pub fn hand(&self, side: Side) -> &HandInput {
        &self.hands[side.index()]
    }

    pub fn hand_mut(&mut self, side: Side) -> &mut HandInput {
        &mut self.hands[side.index()]
    }
}

/// Drivers write [`RigInput`] in `PreUpdate`, in this set, so the fixed
/// tick that follows sees this frame's poses.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RigDriverSystems;

/// Marks the entity that is the rig's head (the camera, for local players).
#[derive(Component)]
pub struct RigHead;

/// Marks a hand entity; child of the player root.
#[derive(Component, Clone, Copy, Debug)]
pub struct RigHand {
    pub side: Side,
}

/// The local player: whose rig the screen (or headset) shows.
#[derive(Component)]
pub struct LocalPlayer;

/// Which driver feeds the local rig. Starts `Flat`; the XR driver switches
/// it to `Xr` when a headset session actually starts.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigDriver {
    Flat,
    Xr,
    Bot,
}

pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        let args: Vec<String> = std::env::args().collect();
        app.register_type::<RigInput>()
            .insert_resource(RigDriver::Flat)
            .insert_resource(comfort::Comfort::load(&args))
            .add_systems(
                PostUpdate,
                apply_rig_poses.before(TransformSystems::Propagate),
            );
    }
}

/// Run condition: the flat driver is in charge.
pub fn flat_driving(driver: Res<RigDriver>) -> bool {
    *driver == RigDriver::Flat
}

/// Copies rig input poses onto the head and hand entities.
fn apply_rig_poses(
    rigs: Query<(&RigInput, &Children)>,
    mut heads: Query<&mut Transform, (With<RigHead>, Without<RigHand>)>,
    mut hands: Query<(&RigHand, &mut Transform), Without<RigHead>>,
) {
    for (input, children) in &rigs {
        for child in children.iter() {
            if let Ok(mut t) = heads.get_mut(child) {
                *t = input.head;
            }
            if let Ok((hand, mut t)) = hands.get_mut(child) {
                *t = input.hand(hand.side).pose;
            }
        }
    }
}
