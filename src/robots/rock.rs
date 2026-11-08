//! A machine answers a hit with its body: struck, it rocks away from the
//! round, by as much as the round did (a .22 barely nudges a heavy, a
//! Mosin or a twelve-bore shoves it), and settles back in a few wobbles.
//!
//! Only what's drawn moves. A machine that [`Rocks`] has its meshes' world
//! transforms turned about its pivot after transforms are propagated; its
//! parts (their colliders, what's aimed at and hit) and its own transform
//! never move, and no piece's `Transform` is written (`wear` keeps a
//! piece's as it was, to put it back). A subtree posed by something else
//! (a heavy's knees, which ride its legs) is left out with [`Unrocked`].

use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

use super::RobotPart;
use super::damage::RobotStruck;

pub struct RockPlugin;

impl Plugin for RockPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (ready, take_hits, show, swing).chain())
            .add_systems(PostUpdate, lean.after(TransformSystems::Propagate));
    }
}

/// How a machine rocks when it's struck.
#[derive(Component, Clone, Copy, Debug)]
pub struct Rocks {
    /// What it turns about, in its own space (a heavy: the ground under
    /// it; a sentry's head: itself).
    pub pivot: Vec3,
    /// Radians a point of a round's harm turns it.
    pub per_harm: f32,
    /// The most one frame's hits turn it, radians.
    pub most: f32,
    /// How quickly it swings back (Hz), and how soon it settles (0..1, a
    /// share of critical damping).
    pub hz: f32,
    pub damping: f32,
}

/// A subtree that isn't rocked: posed by something else.
#[derive(Component)]
pub struct Unrocked;

/// A machine's rock now: its turn (axis times radians, world) and how
/// fast that's changing, and what of it is drawn while it moves.
#[derive(Component, Default, Debug)]
pub struct Rock {
    pub turn: Vec3,
    spin: Vec3,
    pieces: Vec<Entity>,
    /// Held where it is (`EXTRACTION_ROCK_SHOW`), not swinging.
    held: bool,
}

impl Rock {
    /// How far it's turned from at rest, radians.
    pub fn angle(&self) -> f32 {
        self.turn.length()
    }
    /// Back at rest at once (a gauge, before its next round, so what it
    /// measures is that round's).
    pub fn settle(&mut self) {
        self.turn = Vec3::ZERO;
        self.spin = Vec3::ZERO;
    }
}

/// What a machine's rock does to a point of it now (world to world): for
/// what's posed from it elsewhere, a heavy's hips and shoulder, so its
/// legs and arm stay on its rocking hull.
pub fn turn_of(r: &Rocks, rock: &Rock, gt: &GlobalTransform) -> Affine3A {
    if rock.turn.length() <= STILL {
        return Affine3A::IDENTITY;
    }
    let pivot = gt.transform_point(r.pivot);
    Affine3A::from_translation(pivot)
        * Affine3A::from_quat(Quat::from_scaled_axis(rock.turn))
        * Affine3A::from_translation(-pivot)
}

/// Below this (radians, and radians a second) it's at rest.
const STILL: f32 = 0.0005;

fn ready(mut commands: Commands, new: Query<Entity, (Added<Rocks>, Without<Rock>)>) {
    for e in &new {
        commands.entity(e).insert(Rock::default());
    }
}

/// A hit's push: about the axis the round turns it (from the pivot to
/// where it struck, across the way the round went), as much as the round
/// did, all of a frame's rounds together (a twelve-bore's pellets are one
/// shove).
fn take_hits(
    mut struck: MessageReader<RobotStruck>,
    mut machines: Query<(&Rocks, &GlobalTransform, &mut Rock)>,
) {
    let mut pushes: std::collections::HashMap<Entity, Vec3> = Default::default();
    for s in struck.read() {
        let Ok((r, gt, _)) = machines.get(s.robot) else {
            continue;
        };
        let pivot = gt.transform_point(r.pivot);
        let axis = (s.at - pivot).cross(s.dir).normalize_or_zero();
        *pushes.entry(s.robot).or_default() += axis * r.per_harm * s.dealt.max(0.0);
    }
    for (robot, push) in pushes {
        let Ok((r, _, mut rock)) = machines.get_mut(robot) else {
            continue;
        };
        let push = push.clamp_length_max(r.most);
        // A kick to its swing that peaks near `push` (damped, it loses
        // about a third on the way out).
        let w = std::f32::consts::TAU * r.hz;
        rock.spin += push * w * 1.4;
    }
}

/// How far a round of `harm` swings a machine at most (radians), from the
/// same kick and swing as a hit's.
pub fn peak(r: &Rocks, harm: f32) -> f32 {
    let w = std::f32::consts::TAU * r.hz;
    let (mut turn, mut spin) = (0.0f32, (r.per_harm * harm).min(r.most) * w * 1.4);
    let (dt, mut most) = (1.0 / 90.0, 0.0f32);
    for _ in 0..180 {
        spin += (-w * w * turn - 2.0 * r.damping * w * spin) * dt;
        turn += spin * dt;
        most = most.max(turn.abs());
    }
    most
}

/// `EXTRACTION_ROCK_SHOW=<harm>`: every machine that rocks held at the
/// most a round of that harm from the eye swings it, for a still of how
/// far a gun's round shoves it (a pistol's 34, a Mosin's 110).
fn show(
    mut machines: Query<(&Rocks, &GlobalTransform, &mut Rock)>,
    eyes: Query<&GlobalTransform, With<crate::render::FreeCamera>>,
    mut harm: Local<Option<Option<f32>>>,
) {
    let harm = *harm.get_or_insert_with(|| {
        std::env::var("EXTRACTION_ROCK_SHOW")
            .ok()
            .and_then(|v| v.parse().ok())
    });
    let (Some(harm), Some(eye)) = (harm, eyes.iter().next()) else {
        return;
    };
    for (r, gt, mut rock) in &mut machines {
        let pivot = gt.transform_point(r.pivot);
        let away = (gt.translation() - eye.translation())
            .with_y(0.0)
            .normalize_or_zero();
        let axis = (gt.translation() - pivot).cross(away).normalize_or_zero();
        rock.turn = axis * peak(r, harm);
        rock.spin = Vec3::ZERO;
        rock.held = true;
    }
}

/// Swings each machine's rock back to rest, and has what's drawn of it
/// re-placed while it moves (so `lean` turns it from where it is, not from
/// where it was last turned to).
#[allow(clippy::type_complexity)]
fn swing(
    mut machines: Query<(Entity, &Rocks, &mut Rock)>,
    children: Query<&Children>,
    meshes: Query<(), (With<Mesh3d>, Without<RobotPart>)>,
    unrocked: Query<(), With<Unrocked>>,
    mut transforms: Query<&mut Transform>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);
    for (robot, r, mut rock) in &mut machines {
        let moving = rock.turn.length() > STILL || rock.spin.length() > STILL;
        if !moving {
            if !rock.pieces.is_empty() {
                // Back at rest: once more from their own, then left be.
                for e in std::mem::take(&mut rock.pieces) {
                    if let Ok(mut t) = transforms.get_mut(e) {
                        t.set_changed();
                    }
                }
                rock.turn = Vec3::ZERO;
                rock.spin = Vec3::ZERO;
            }
            continue;
        }
        if rock.pieces.is_empty() {
            let mut stack = vec![robot];
            while let Some(e) = stack.pop() {
                if unrocked.contains(e) {
                    continue;
                }
                if meshes.contains(e) {
                    rock.pieces.push(e);
                }
                if let Ok(c) = children.get(e) {
                    stack.extend(c.iter());
                }
            }
        }
        if !rock.held {
            let w = std::f32::consts::TAU * r.hz;
            let acc = -w * w * rock.turn - 2.0 * r.damping * w * rock.spin;
            rock.spin += acc * dt;
            let spin = rock.spin;
            rock.turn += spin * dt;
        }
        for e in &rock.pieces {
            if let Ok(mut t) = transforms.get_mut(*e) {
                t.set_changed();
            }
        }
    }
}

/// Turns what's drawn of each rocking machine about its pivot.
fn lean(
    machines: Query<(&Rocks, &Rock, &GlobalTransform)>,
    mut drawn: Query<&mut GlobalTransform, Without<Rock>>,
) {
    for (r, rock, gt) in &machines {
        if rock.pieces.is_empty() {
            continue;
        }
        let turn = turn_of(r, rock, gt);
        for e in &rock.pieces {
            if let Ok(mut g) = drawn.get_mut(*e) {
                *g = GlobalTransform::from(turn * g.affine());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heavy_like() -> Rocks {
        Rocks {
            pivot: Vec3::new(0.0, -2.0, 0.0),
            per_harm: 0.0006,
            most: 0.1,
            hz: 2.2,
            damping: 0.3,
        }
    }

    /// Kicked by `harm`, how far it swings at most and how long until it's
    /// still.
    fn swing_of(r: Rocks, harm: f32) -> (f32, f32) {
        let mut rock = Rock::default();
        let w = std::f32::consts::TAU * r.hz;
        rock.spin = Vec3::X * (r.per_harm * harm).min(r.most) * w * 1.4;
        let (dt, mut t, mut peak, mut still) = (1.0 / 90.0, 0.0, 0.0f32, 0.0);
        while t < 5.0 {
            let acc = -w * w * rock.turn - 2.0 * r.damping * w * rock.spin;
            rock.spin += acc * dt;
            let s = rock.spin;
            rock.turn += s * dt;
            peak = peak.max(rock.angle());
            if rock.angle() > 0.1 * peak {
                still = t;
            }
            t += dt;
        }
        (peak, still)
    }

    /// The round tells: a Mosin shoves a heavy some times what a pistol
    /// does, and a .22 hardly at all; nothing swings for long.
    #[test]
    fn a_heavier_round_shoves_harder() {
        let r = heavy_like();
        let (twenty_two, _) = swing_of(r, 16.0);
        let (pistol, _) = swing_of(r, 34.0);
        let (mosin, settles) = swing_of(r, 110.0);
        assert!(mosin > 2.5 * pistol, "{mosin} vs {pistol}");
        assert!(pistol > 2.0 * twenty_two, "{pistol} vs {twenty_two}");
        assert!(settles < 1.2, "still after {settles} s");
        // About as far as it's asked, give or take the damping.
        let asked = r.per_harm * 110.0;
        assert!(
            (0.7 * asked..1.3 * asked).contains(&mosin),
            "{mosin} for {asked}"
        );
    }
}
