//! Robots get away from a live grenade they know about: one they can see
//! (in line of sight, within their eyes' reach), or one they heard land
//! close by. They run (walkers, hounds, crawlers) or climb away (drones)
//! out past its blast and fragments, then, once it's gone off, their
//! brains carry on: the bang is a noise like any other, so they come back
//! to look where it was.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::grenade::Fuse;

use super::{Robot, Senses};
use crate::weapons::grenades::Live;
use crate::world::Layer;

/// Out past the blast radius by this much, for the fragments (m).
pub const FLEE_MARGIN: f32 = 3.0;
/// It notices one it can see within this of it (m)...
pub const SEE_GRENADE: f32 = 25.0;
/// ...or one that lands within this, by ear (m).
pub const HEAR_GRENADE: f32 = 12.0;

/// Getting away from a grenade at `from` (blast `radius`).
#[derive(Component, Debug, Clone, Copy)]
pub struct Fleeing {
    pub grenade: Entity,
    pub from: Vec3,
    pub radius: f32,
}

impl Fleeing {
    /// Where to run to from `pos`: straight away from it, out past its
    /// reach.
    pub fn away(&self, pos: Vec3) -> Vec3 {
        let dir = Vec3::new(pos.x - self.from.x, 0.0, pos.z - self.from.z).normalize_or(Vec3::X);
        self.from + dir * (self.radius + FLEE_MARGIN + 1.0)
    }
}

pub struct FleePlugin;

impl Plugin for FleePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, notice_grenades.after(super::perceive));
    }
}

fn notice_grenades(
    mut commands: Commands,
    grenades: Query<(Entity, &Live, &GlobalTransform)>,
    robots: Query<(Entity, &Robot, &Senses, &GlobalTransform, Option<&Fleeing>)>,
    spatial: SpatialQuery,
) {
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    let live: Vec<(Entity, Vec3, f32)> = grenades
        .iter()
        .filter(|(_, l, _)| matches!(l.grenade.fuse, Fuse::Burning(_)))
        .map(|(e, l, g)| (e, g.translation(), l.grenade.def.blast_radius))
        .collect();
    for (e, robot, senses, gt, fleeing) in &robots {
        if let Some(f) = fleeing {
            // Gone off (or gone): back to its brain.
            if !live.iter().any(|(g, ..)| *g == f.grenade) {
                commands.entity(e).remove::<Fleeing>();
            }
            continue;
        }
        if robot.wrecked {
            continue;
        }
        let pos = gt.translation();
        let eye = gt.transform_point(senses.eye);
        for &(g, at, radius) in &live {
            let d = at.distance(pos);
            if d > radius + FLEE_MARGIN {
                continue;
            }
            let heard = d < HEAR_GRENADE;
            let seen = d < SEE_GRENADE
                && Dir3::new(at - eye).ok().is_some_and(|dir| {
                    spatial
                        .cast_ray(eye, dir, at.distance(eye) - 0.2, true, &world)
                        .is_none()
                });
            if heard || seen {
                commands.entity(e).insert(Fleeing {
                    grenade: g,
                    from: at,
                    radius,
                });
                break;
            }
        }
    }
}
