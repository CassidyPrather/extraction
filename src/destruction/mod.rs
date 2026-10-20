//! The world answers back: glass breaks, wood splinters and gives way,
//! and what's broken stays broken until the Surge. See
//! `docs/DESTRUCTION.md`.
//!
//! - [`glass`]: every window pane can break: shot, struck, thrown at,
//!   punched, or walked into by a machine. It shatters round where it was
//!   hit into physics shards and leaves teeth in the frame.
//! - [`wood`]: plank doors and supply crates take damage from rounds and
//!   blows and break into splinters; a crate broken open spills what was
//!   packed in it. Rounds go through plank doors as they go through plank
//!   walls (`firearm_terminal`).
//! - [`debris`]: the pieces, as physics bodies on their own layer, from a
//!   capped pool, asleep when they settle and gone after a while.
//!
//! Logic here is headless-safe; [`visuals`] draws panes and pieces.
//! Breaking is deterministic from what broke and where, so the same pane
//! breaks the same way for a replay and a restored save.

use bevy::prelude::*;
use std::collections::HashSet;

pub mod debris;
pub mod glass;
pub mod hooks;
pub mod scenario;
pub mod through;
pub mod visuals;
pub mod wood;

/// What a break was made of, for sound and noise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stuff {
    Glass,
    Wood,
}

/// Something broke (for sound, haptics and anything that listens).
#[derive(Message, Clone, Copy, Debug)]
pub struct Broke {
    pub at: Vec3,
    pub stuff: Stuff,
    /// How big a thing: a pane's area or a door's (m²): a shop window
    /// crashes, a vent pane tinkles.
    pub size: f32,
}

/// How far a robot hears a break, metres: glass carries further than
/// any wood but a door going down.
pub fn noise_radius(stuff: Stuff, size: f32) -> f32 {
    match stuff {
        Stuff::Glass => (25.0 + 30.0 * size).min(55.0),
        Stuff::Wood => (20.0 + 8.0 * size).min(40.0),
    }
}

/// Everything broken since the Surge, by where it stood ([`key`]); saved
/// with the Zone (`ZoneSave::broken`) and cleared when the Surge comes.
#[derive(Resource, Default, Debug, Clone)]
pub struct Broken(pub HashSet<[i32; 3]>);

/// A breakable thing's identity: where it stands, to the centimetre. The
/// world is built from the map deterministically, so this names the same
/// pane or door in every session.
pub fn key(at: Vec3) -> [i32; 3] {
    let c = (at * 100.0).round();
    [c.x as i32, c.y as i32, c.z as i32]
}

/// Breaking and restoring run here, after shots and blows have landed.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DestructionSystems;

pub struct DestructionPlugin;

impl Plugin for DestructionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Broken>()
            .add_message::<Broke>()
            .configure_sets(
                FixedUpdate,
                DestructionSystems.after(crate::weapons::ballistics::BallisticsSystems),
            )
            .add_systems(FixedUpdate, make_noise.after(DestructionSystems))
            .add_plugins((
                debris::DebrisPlugin,
                glass::GlassPlugin,
                wood::WoodPlugin,
                hooks::HooksPlugin,
            ));
    }
}

/// A break is heard: robots turn to it.
fn make_noise(mut broke: MessageReader<Broke>, mut noise: MessageWriter<crate::robots::Noise>) {
    for b in broke.read() {
        noise.write(crate::robots::Noise {
            at: b.at,
            radius: noise_radius(b.stuff, b.size),
            source: None,
        });
    }
}

/// The Surge has been through: every pane whole, every door back on its
/// hinges, every crate nailed shut again (`persist::reset_zone`).
pub fn surge(world: &mut World) {
    world.resource_mut::<Broken>().0.clear();
    glass::mend_all(world);
    wood::mend_all(world);
}

#[cfg(test)]
mod tests;
