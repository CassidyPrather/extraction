//! How much a place's echo smears where a sound seems to come from, for
//! robots' ears ([`Reverb::smear`]): 1 in the open, more down in a valley
//! whose sides throw a shot back, more again under a roof, most in a room
//! built to carry sound (`world::echo`'s gallery).
//!
//! Precomputed so a robot can ask once per sound it hears: the ground on
//! a coarse grid when the map loads, the buildings' shells and carrying
//! rooms when they change.

use bevy::prelude::*;
use extraction_core::underground as ug;

use crate::robots::doors::{Entrances, Shell};
use crate::world::buildings::Building;

/// Grid cell (m).
pub const CELL: f32 = 8.0;
/// Under a roof.
pub const INDOORS: f32 = 2.0;
/// In a room built to carry sound.
pub const GALLERY: f32 = 3.0;
/// Most a valley smears.
pub const VALLEY_MAX: f32 = 2.2;
/// Ground this much higher than you, within [`VALLEY_SPAN`], throws a
/// shot back (m).
pub const WALL_RISE: f32 = 6.0;
/// How far out a valley's sides are looked for (m).
pub const VALLEY_SPAN: (f32, f32) = (15.0, 70.0);

pub struct ReverbPlugin;

impl Plugin for ReverbPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Reverb>()
            .add_systems(Update, (grid_the_ground, take_the_rooms));
    }
}

/// A carrying room: from the world into its building, and its box there.
#[derive(Clone, Debug)]
struct Gallery {
    into: bevy::math::Affine3A,
    centre: Vec2,
    half: Vec2,
    floor: f32,
    height: f32,
}

/// The echo, everywhere robots listen.
#[derive(Resource, Default, Debug)]
pub struct Reverb {
    origin: Vec2,
    n: usize,
    ground: Vec<f32>,
    shells: Vec<Shell>,
    galleries: Vec<Gallery>,
}

impl Reverb {
    /// How much a sound made at `at` has its whereabouts smeared by echo:
    /// 1 in the open, up to [`GALLERY`].
    pub fn smear(&self, at: Vec3) -> f32 {
        if self.galleries.iter().any(|g| {
            let q = g.into.transform_point3(at);
            (q.x - g.centre.x).abs() <= g.half.x
                && (q.z - g.centre.y).abs() <= g.half.y
                && q.y > g.floor - 0.5
                && q.y < g.floor + g.height
        }) {
            return GALLERY;
        }
        let indoors = self.shells.iter().any(|s| {
            let l = s.xf.compute_affine().inverse().transform_point3(at);
            l.x.abs() < s.half.x && l.z.abs() < s.half.y && l.y > -1.0 && l.y < s.height - 0.6
        });
        if indoors {
            return INDOORS;
        }
        self.ground_smear(at.x, at.z)
    }

    fn ground_smear(&self, x: f32, z: f32) -> f32 {
        if self.n == 0 {
            return 1.0;
        }
        let i = ((x - self.origin.x) / CELL).floor();
        let j = ((z - self.origin.y) / CELL).floor();
        if i < 0.0 || j < 0.0 || i >= self.n as f32 || j >= self.n as f32 {
            return 1.0;
        }
        self.ground[j as usize * self.n + i as usize]
    }
}

/// How much the ground round (`x`, `z`) smears a sound made there: the
/// share of eight bearings on which it rises [`WALL_RISE`] within
/// [`VALLEY_SPAN`], and more when the walls face each other (a valley's
/// two sides, a flutter between them).
pub fn valley_smear(ground: impl Fn(f32, f32) -> f32, x: f32, z: f32) -> f32 {
    let h0 = ground(x, z);
    let mut walls = [false; 8];
    for (k, wall) in walls.iter_mut().enumerate() {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        let d = Vec2::new(a.cos(), a.sin());
        let mut r = VALLEY_SPAN.0;
        while r <= VALLEY_SPAN.1 {
            if ground(x + d.x * r, z + d.y * r) - h0 >= WALL_RISE {
                *wall = true;
                break;
            }
            r += 10.0;
        }
    }
    let closed = walls.iter().filter(|w| **w).count() as f32 / 8.0;
    let facing = (0..4).filter(|&k| walls[k] && walls[k + 4]).count() as f32 / 4.0;
    (1.0 + 0.6 * closed + 0.6 * facing).min(VALLEY_MAX)
}

/// The ground's echo on the grid, when the map loads.
fn grid_the_ground(zone: Option<Res<crate::world::Zone>>, mut reverb: ResMut<Reverb>) {
    let Some(zone) = zone.filter(|z| z.is_changed()) else {
        return;
    };
    let half = zone.map.size * 0.5;
    let n = (zone.map.size / CELL).ceil() as usize;
    let origin = Vec2::splat(-half);
    let mut ground = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let x = origin.x + (i as f32 + 0.5) * CELL;
            let z = origin.y + (j as f32 + 0.5) * CELL;
            ground.push(valley_smear(|x, z| zone.ground(x, z), x, z));
        }
    }
    reverb.origin = origin;
    reverb.n = n;
    reverb.ground = ground;
}

/// The buildings' shells and carrying rooms, when they change.
fn take_the_rooms(
    entrances: Res<Entrances>,
    sites: Query<(&Building, &GlobalTransform)>,
    added: Query<(), Added<Building>>,
    mut gone: RemovedComponents<Building>,
    mut reverb: ResMut<Reverb>,
) {
    let changed = gone.read().count() > 0 || !added.is_empty();
    if entrances.is_changed() {
        reverb.shells = entrances.shells.clone();
    }
    if !changed {
        return;
    }
    reverb.galleries = sites
        .iter()
        .filter_map(|(b, gt)| Some((ug::layout(&b.prefab)?, gt)))
        .flat_map(|(l, gt)| {
            let into = gt.affine().inverse();
            l.rooms
                .iter()
                .filter(|r| r.carries)
                .map(|r| Gallery {
                    into,
                    centre: Vec2::new(r.at[0], r.at[1]),
                    half: Vec2::new(r.size[0], r.size[1]) * 0.5,
                    floor: l.floor_of(r),
                    height: r.height,
                })
                .collect::<Vec<_>>()
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open ground doesn't smear; a valley between two ridges does, and a
    /// hollow ringed by hills more.
    #[test]
    fn valleys_smear_and_plains_dont() {
        let flat = |_: f32, _: f32| 10.0;
        let valley = |x: f32, _: f32| 10.0 + 0.4 * x.abs().min(40.0);
        let bowl = |x: f32, z: f32| 10.0 + 0.4 * (x * x + z * z).sqrt().min(40.0);
        let (f, v, b) = (
            valley_smear(flat, 0.0, 0.0),
            valley_smear(valley, 0.0, 0.0),
            valley_smear(bowl, 0.0, 0.0),
        );
        println!("smear: flat {f:.2}, valley {v:.2}, bowl {b:.2}");
        assert_eq!(f, 1.0);
        assert!(v > 1.4, "{v}");
        assert!(b > v, "{b} vs {v}");
        // Up on the ridge, it's open again.
        assert!(valley_smear(valley, 60.0, 0.0) < v);
    }
}
