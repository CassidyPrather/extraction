//! How other things break things here, besides a [`Hit`]:
//!
//! - [`Smash`]: something big moving through a place (a heavy machine's
//!   arm, a falling beam): every pane within reach breaks and every
//!   piece of wood takes the blow.
//! - [`Pryable`] / [`Pried`]: a crowbar's tip seated in a seam and
//!   levered. Enough torque, held, and it gives: a door's leaf is wrenched
//!   off (quieter than battering it down), a crate's lid pops and it
//!   spills open.
//! - [`Cuttable`] / [`Cut`]: bolt cutters through a padlock or a wire.
//!
//! A [`Hit`] with no round mass (`mass_g: 0`) is a blow: a butt stroke, a
//! blade, a blast. Wood takes blows at full weight and rounds at less
//! (`wood::ROUND_ON_DOOR`). Glass needs no message at all: anything
//! moving through a pane fast enough breaks it (`glass`).
//!
//! [`Hit`]: crate::weapons::ballistics::Hit

use bevy::prelude::*;

use super::glass::{Pane, PaneGrid};
use super::wood::Timber;
use super::{Broke, Broken};

/// Something heavy went through here: panes within `radius` of `at`
/// break and wood within it takes `damage`, all pushed along `push`.
#[derive(Message, Clone, Copy, Debug)]
pub struct Smash {
    pub at: Vec3,
    pub radius: f32,
    pub damage: f32,
    pub push: Vec3,
}

/// Tear a doorway-sized hole in the wall at `at` (within a few
/// centimetres of the wall's middle), its boards flying along `out`: a
/// heavy machine's arm opening a building up (`destruction::breach`).
/// Concrete and brick don't tear.
#[derive(Message, Clone, Copy, Debug)]
pub struct Breach {
    pub at: Vec3,
    pub out: Vec3,
}

/// A seam a crowbar's tip seats in: torque (N·m) that, held for
/// [`PRY_HOLD`] seconds, makes it give. `seam` is where the tip goes and
/// `along` the way the seam runs, both in the thing's own space.
#[derive(Component, Clone, Copy, Debug)]
pub struct Pryable {
    pub needs: f32,
    pub seam: Vec3,
    pub along: Vec3,
    /// Seconds the torque has been enough, so far.
    pub held: f32,
}

/// A crowbar levering at a [`Pryable`] this tick.
#[derive(Message, Clone, Copy, Debug)]
pub struct Pried {
    pub target: Entity,
    pub torque: f32,
    pub by: Entity,
}

/// Something bolt cutters go through (a padlock, a run of wire).
#[derive(Component, Clone, Copy, Debug)]
pub struct Cuttable {
    pub needs: f32,
}

/// The cutters' jaws closed through a [`Cuttable`].
#[derive(Message, Clone, Copy, Debug)]
pub struct Cut {
    pub target: Entity,
    pub by: Entity,
}

/// How long enough torque must be held before a seam gives, seconds.
pub const PRY_HOLD: f32 = 0.4;
/// A plank door's latch side: torque to wrench it (a long bar, a hard
/// pull: about 60 kg on a 0.6 m bar).
pub const DOOR_PRY: f32 = 350.0;
/// A crate's lid: nails, not a latch.
pub const LID_PRY: f32 = 120.0;

pub struct HooksPlugin;

impl Plugin for HooksPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Smash>()
            .add_message::<Breach>()
            .add_message::<Pried>()
            .add_message::<Cut>()
            .add_systems(
                FixedUpdate,
                (smash, pry, cut).in_set(super::DestructionSystems),
            );
    }
}

/// Seams on what can be pried: a plank door's latch edge, a crate's lid.
pub fn door_seam(width: f32) -> Pryable {
    Pryable {
        needs: DOOR_PRY,
        seam: Vec3::new(width - 0.02, 1.0, 0.0),
        along: Vec3::Y,
        held: 0.0,
    }
}

pub fn lid_seam(half: Vec3) -> Pryable {
    Pryable {
        needs: LID_PRY,
        seam: Vec3::new(0.0, half.y, -half.z),
        along: Vec3::X,
        held: 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn smash(
    mut commands: Commands,
    mut smashes: MessageReader<Smash>,
    mut panes: Query<&mut Pane>,
    grid: Res<PaneGrid>,
    mut timber: Query<(Entity, &mut Timber, &GlobalTransform)>,
    mut pool: ResMut<super::debris::DebrisPool>,
    mut broken: ResMut<Broken>,
    mut out: MessageWriter<Broke>,
    mut hits: MessageWriter<crate::weapons::ballistics::Hit>,
) {
    for s in smashes.read() {
        let near: Vec<Entity> = grid.near(s.at, s.at, s.radius).collect();
        for e in near {
            let Ok(mut pane) = panes.get_mut(e) else {
                continue;
            };
            if let Some(x) = pane.touching(s.at, s.radius) {
                super::glass::shatter(
                    &mut commands,
                    &mut pool,
                    &mut broken,
                    &mut out,
                    &mut pane,
                    x,
                    s.push,
                );
            }
        }
        // Wood takes it as a blow (through the same door as any other).
        for (e, mut t, gt) in &mut timber {
            if gt.translation().distance(s.at) <= s.radius + 1.0 {
                t.push = s.push;
                hits.write(crate::weapons::ballistics::Hit {
                    blow: None,
                    entity: e,
                    point: s.at,
                    normal: -s.push.normalize_or(Vec3::Y),
                    velocity: s.push,
                    damage: s.damage,
                    shooter: None,
                    mass_g: 0.0,
                    load: Default::default(),
                });
            }
        }
    }
}

/// Enough torque held and the seam gives: the thing breaks (a door off
/// its hinges, a crate open), with half the noise of smashing it.
fn pry(
    mut commands: Commands,
    mut pried: MessageReader<Pried>,
    mut seams: Query<(Entity, &mut Pryable, &mut Timber, &GlobalTransform)>,
    mut broken: ResMut<Broken>,
    mut out: MessageWriter<Broke>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let mut levered: Vec<(Entity, f32)> = pried.read().map(|p| (p.target, p.torque)).collect();
    levered.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
    levered.dedup_by_key(|p| p.0);
    for (e, mut seam, mut t, gt) in &mut seams {
        // Let go, or not hard enough: it starts over.
        let torque = levered
            .binary_search_by(|p| p.0.cmp(&e))
            .map_or(0.0, |i| levered[i].1);
        if torque < seam.needs {
            seam.held = 0.0;
            continue;
        }
        seam.held += dt;
        if seam.held < PRY_HOLD || t.hp <= 0.0 {
            continue;
        }
        t.hp = 0.0;
        // Levered away from the seam: gently.
        t.push = gt.rotation() * Vec3::Z * 1.2;
        broken.0.insert(t.key);
        out.write(Broke {
            at: gt.transform_point(seam.seam),
            stuff: super::Stuff::Wood,
            size: 0.3,
        });
        commands.entity(e).despawn();
    }
}

/// Nothing here is cuttable yet (padlocks and wire would be); a cut
/// through a [`Cuttable`] takes it away.
fn cut(mut commands: Commands, mut cuts: MessageReader<Cut>, things: Query<&Cuttable>) {
    for c in cuts.read() {
        if things.contains(c.target) {
            commands.entity(c.target).try_despawn();
        }
    }
}
