//! Tools worked with the hands: the crowbar ([`crowbar`]). Each sets up
//! its grips when the item appears, and reads what the hands do with it
//! while it's held.
//!
//! What gives way is the target's business. A tool sends a message (see
//! [`crowbar::PriedFree`]) and whoever owns the thing decides what that
//! means. Where nothing else claims a target that already exists (a robot's
//! armour plate, a wedged door), the tool's own module does the small part.

use bevy::prelude::*;

use crate::rig::Side;
use crate::rig::haptics::Haptic;

pub mod crowbar;

pub struct ToolsPlugin;

impl Plugin for ToolsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crowbar::CrowbarPlugin);
    }
}

/// The shared haptic scale (`rig::haptics`): a tick says "this is here",
/// a click "that happened", a knock "that took force".
pub(crate) const TICK: (f32, f32) = (0.12, 0.012);
pub(crate) const CLICK: (f32, f32) = (0.3, 0.025);
pub(crate) const KNOCK: (f32, f32) = (0.55, 0.06);

pub(crate) fn pulse(out: &mut MessageWriter<Haptic>, side: Side, (amplitude, seconds): (f32, f32)) {
    out.write(Haptic {
        side,
        amplitude,
        seconds,
    });
}
