//! The rig: a head and two hands, and the input that drives them.
//!
//! Every player is a rig. A *driver* writes the rig's [`RigInput`] each
//! frame — the OpenXR driver from a headset, the flat driver from mouse and
//! keyboard, the bot driver from the simulation's brain — and everything
//! downstream (locomotion, grabbing, guns, the backpack) reads only that.
//! Poses are in *playspace*: relative to the player root, which sits at the
//! feet and carries the body's yaw.

pub mod comfort;
pub mod fingers;
pub mod flat;
pub mod haptics;
pub mod loading;
pub mod runtime;
pub mod seated;
pub mod shapes;
pub mod stance;
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

    /// The other side if `mirror` (a left-handed player's gun hand is
    /// the right-handed one's, mirrored).
    pub fn mirrored(self, mirror: bool) -> Self {
        if mirror { self.other() } else { self }
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
    /// The thumb rests on a face button, the stick or the thumb rest
    /// (controllers that sense touch); the glove's thumb lies over it.
    pub thumb_rest: bool,
    /// The fingers themselves, where the runtime tracks them (Index
    /// controllers, bare hands); `None` leaves the glove to its poses.
    pub fingers: Option<fingers::Fingers>,
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
    /// Playtest mark: flag this moment (flat F9; VR both sticks clicked).
    pub mark: bool,
    /// How far the head and hands have been lifted for seated play
    /// (`seated`): drivers write raw poses and set it to 0.
    pub lifted: f32,
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
            .add_plugins((
                haptics::HapticsPlugin,
                seated::SeatedPlugin,
                shapes::ShapesPlugin,
                stance::StancePlugin,
            ))
            .insert_resource(RigDriver::Flat)
            .insert_resource(comfort::Comfort::load(&args))
            .add_systems(
                PostUpdate,
                apply_rig_poses.before(TransformSystems::Propagate),
            );
    }
}

/// The headset was asked for and not found: what to do about it, shown in
/// the flat window instead of the key hints.
#[derive(Resource, Clone, Debug)]
pub struct XrMissing(pub String);

/// The headset's been asked for and its session hasn't started (nor been
/// given up on): till then nobody knows whether the player's in a headset
/// or at the window, so nothing picks headset or flat words
/// ([`KnownDriver`]). The runtime's own loader is all a headset shows
/// meanwhile, 3 to 20 s.
#[derive(Resource)]
pub struct DriverPending;

/// The driver, once it's known.
#[derive(bevy::ecs::system::SystemParam)]
pub struct KnownDriver<'w> {
    driver: Res<'w, RigDriver>,
    pending: Option<Res<'w, DriverPending>>,
}

impl KnownDriver<'_> {
    pub fn get(&self) -> Option<RigDriver> {
        self.pending.is_none().then_some(*self.driver)
    }

    /// Flat words or headset words: `None` till it's known.
    pub fn flat(&self) -> Option<bool> {
        self.get().map(|d| d == RigDriver::Flat)
    }
}

/// The headset session started: the rig is the headset's.
pub fn headset_started(world: &mut World) {
    world.insert_resource(RigDriver::Xr);
    world.remove_resource::<DriverPending>();
}

/// Run condition: the flat driver is in charge.
pub fn flat_driving(driver: Res<RigDriver>) -> bool {
    *driver == RigDriver::Flat
}

/// Copies rig input poses onto the head and hand entities.
pub(crate) fn apply_rig_poses(
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
