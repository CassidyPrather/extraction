//! Would a round fired from here get there? The same rules the rounds
//! fly by (`firearm_terminal`, a struck thing's material and thickness
//! from `ballistics::Struck`), walked along a line without firing: for
//! anything deciding whether to shoot through a wall at what it heard
//! behind it.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::ammo::Load;
use extraction_core::firearm_terminal::{Outcome, strike_load};

use crate::weapons::ballistics::Struck;
use crate::world::Layer;

/// The line check as a system parameter: `Query`s and the spatial query
/// together, for a robot's brain to ask with.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct RoundPath<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    struck: Struck<'w, 's>,
}

impl RoundPath<'_, '_> {
    /// [`would_reach`], skipping the shooter (and anything parented to
    /// it) and stopping at the target if the line meets it.
    pub fn would_reach(
        &self,
        from: Vec3,
        to: Vec3,
        mass_g: f32,
        speed: f32,
        skip: &dyn Fn(Entity) -> bool,
    ) -> bool {
        would_reach(
            &self.spatial,
            &self.struck,
            from,
            to,
            mass_g,
            speed,
            skip,
            &|_| false,
        )
    }
}

/// How many things a round may go through on the way.
const MAX_PASSES: u8 = 3;

/// Whether a round of `mass_g` grams leaving `from` at `speed` m/s
/// toward `to` would reach it, going through what's between. `skip` are
/// things the line ignores (the shooter and its gun, say); `is_target`
/// ends the walk when the line meets the target itself.
#[allow(clippy::too_many_arguments)]
pub(crate) fn would_reach(
    spatial: &SpatialQuery,
    struck: &Struck,
    from: Vec3,
    to: Vec3,
    mass_g: f32,
    speed: f32,
    skip: &dyn Fn(Entity) -> bool,
    is_target: &dyn Fn(Entity) -> bool,
) -> bool {
    let filter =
        SpatialQueryFilter::from_mask([Layer::World, Layer::Item, Layer::Robot, Layer::Player]);
    let Ok(dir) = Dir3::new(to - from) else {
        return true;
    };
    let mut at = from;
    let mut speed = speed;
    for _ in 0..=MAX_PASSES {
        let left = (to - at).dot(*dir);
        if left <= 0.0 {
            return true;
        }
        let Some(hit) = spatial.cast_ray_predicate(at, dir, left, true, &filter, &|e| !skip(e))
        else {
            return true;
        };
        if is_target(hit.entity) {
            return true;
        }
        let point = at + *dir * hit.distance;
        let cos_in = (-*dir).dot(hit.normal).clamp(0.0, 1.0);
        let (stuff, mm, along) = struck.stuff(hit.entity, point, *dir, cos_in);
        // The middle of the odds: a glance is a miss.
        match strike_load(Load::Standard, stuff, mass_g, speed, cos_in, mm, 0.5) {
            Outcome::Through { speed: out } => {
                speed = out;
                at = point + *dir * (along + 0.01);
            }
            _ => return false,
        }
    }
    false
}
