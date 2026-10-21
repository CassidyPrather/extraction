//! Bolt cutters. Take a handle in each hand, the jaws open as the hands
//! spread. Put the jaws to a padlock's shackle or the wire of a chain-link
//! panel and squeeze the handles together: once your hands have closed far
//! enough, the jaws bite through. It takes both hands; one can't close
//! them.
//!
//! What can be cut is `destruction::hooks::Cuttable`, and the cut is a
//! `destruction::hooks::Cut` (Destruction takes the cut thing away). A cut
//! chain-link panel is a gap you can pass through; a cut padlock lets go
//! of the panel it held shut.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{CLICK, KNOCK, TICK, pulse};
use crate::destruction::hooks::{Cut, Cuttable};
use crate::interact::{GrabPoint, Grabbable, GripRole, Hand, Held, InteractSystems, Supported};
use crate::items::{Catalog, Item};
use crate::rig::haptics::Haptic;

/// In the cutters' space: the two handle ends, and the jaws (-Z).
pub const LEFT: Vec3 = Vec3::new(-0.07, 0.0, 0.26);
pub const RIGHT: Vec3 = Vec3::new(0.07, 0.0, 0.26);
pub const JAWS: Vec3 = Vec3::new(0.0, 0.0, -0.3);
/// How far the hands must close from where they were when the jaws went
/// on (m), and how near together they must end.
const CLOSE: f32 = 0.1;
const SHUT: f32 = 0.16;

pub struct CuttersPlugin;

impl Plugin for CuttersPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(setup_cutters).add_systems(
            FixedUpdate,
            (jaws_touch, squeeze, cut_through)
                .chain()
                .after(InteractSystems::Follow),
        );
    }
}

/// A chain-link panel: cut, it's a gap.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ChainLink {
    pub cut: bool,
}

/// A padlock holding something shut (a chain-link gate): cut, it falls
/// and the gate is a gap.
#[derive(Component, Clone, Copy, Debug)]
pub struct Padlock {
    pub holds: Entity,
}

/// Bolt cutters, and what their jaws are on.
#[derive(Component, Debug, Default)]
pub struct Cutters {
    /// What the jaws are on, and the widest the hands have been since.
    pub on: Option<(Entity, f32)>,
    /// What the jaws touch now.
    touching: Option<Entity>,
    /// How far apart the hands are on the handles (m; 0 when they aren't
    /// both on), for the handles to spread to.
    pub spread: f32,
}

/// How near the jaws something must be to be between them (m).
const JAW_REACH: f32 = 0.04;

/// What the jaws are touching: a Cuttable's collider (or a collider on a
/// child of one).
fn jaws_touch(
    mut cutters: Query<(Entity, &mut Cutters, &GlobalTransform)>,
    cuttables: Query<(), With<Cuttable>>,
    parents: Query<&ChildOf>,
    spatial: SpatialQuery,
) {
    for (e, mut c, gt) in &mut cutters {
        let jaws = gt.transform_point(JAWS);
        let hits = spatial.shape_intersections(
            &Collider::sphere(JAW_REACH),
            jaws,
            Quat::IDENTITY,
            &SpatialQueryFilter::default().with_excluded_entities([e]),
        );
        c.touching = hits.into_iter().find_map(|mut up| {
            for _ in 0..3 {
                if cuttables.contains(up) {
                    return Some(up);
                }
                up = parents.get(up).ok()?.parent();
            }
            None
        });
    }
}

fn setup_cutters(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    if catalog.get(item.kind()).id != "bolt_cutters" {
        return;
    }
    commands.entity(add.entity).insert((
        Cutters::default(),
        Grabbable {
            points: vec![
                GrabPoint::new(Transform::from_translation(LEFT), GripRole::Primary, 0.12),
                GrabPoint::new(Transform::from_translation(RIGHT), GripRole::Support, 0.12),
            ],
        },
    ));
}

/// The jaws go on what they touch (both hands on the handles); squeezed
/// shut from there, they cut.
#[allow(clippy::type_complexity)]
fn squeeze(
    mut cutters: Query<(
        &mut Cutters,
        Option<&Held>,
        Option<&Supported>,
        &mut Position,
        &Rotation,
        &mut LinearVelocity,
    )>,
    hands: Query<&Hand>,
    mut cut: MessageWriter<Cut>,
    mut haptics: MessageWriter<Haptic>,
) {
    for (mut c, held, support, mut pos, rot, mut lin) in &mut cutters {
        let (Some(held), Some(support)) = (held, support) else {
            c.on = None;
            c.spread = 0.0;
            continue;
        };
        let (Ok(a), Ok(b)) = (hands.get(held.hand), hands.get(support.hand)) else {
            continue;
        };
        let apart = a.world.translation.distance(b.world.translation);
        c.spread = apart;
        // Held between the hands (each on its handle as they spread), not
        // hung off the first one.
        let mid = (a.world.translation + b.world.translation) * 0.5;
        pos.0 = mid - rot.0 * ((LEFT + RIGHT) * 0.5);
        lin.0 = Vec3::ZERO;
        let touching = c.touching;
        match c.on {
            Some((target, from)) if touching == Some(target) => {
                // From the widest they've opened since the jaws went on.
                let from = from.max(apart);
                c.on = Some((target, from));
                if from - apart >= CLOSE && apart <= SHUT {
                    cut.write(Cut {
                        target,
                        by: held.player,
                    });
                    c.on = None;
                    pulse(&mut haptics, held.side, KNOCK);
                    pulse(&mut haptics, support.side, KNOCK);
                }
            }
            Some(_) => {
                c.on = None;
                pulse(&mut haptics, held.side, TICK);
            }
            None => {
                if let Some(target) = touching {
                    c.on = Some((target, apart));
                    pulse(&mut haptics, held.side, CLICK);
                }
            }
        }
    }
}

/// A cut panel opens; a cut padlock opens what it held. (Destruction
/// takes the cut thing itself away.)
fn cut_through(
    mut commands: Commands,
    mut cuts: MessageReader<Cut>,
    mut panels: Query<(Entity, &mut ChainLink, Option<&Children>)>,
    locks: Query<&Padlock>,
    colliders: Query<(), With<Collider>>,
) {
    let mut open = |commands: &mut Commands, e: Entity| {
        let Ok((e, mut panel, children)) = panels.get_mut(e) else {
            return;
        };
        if panel.cut {
            return;
        }
        panel.cut = true;
        for c in std::iter::once(e).chain(children.into_iter().flatten().copied()) {
            if colliders.contains(c) {
                commands.entity(c).try_insert(ColliderDisabled);
            }
        }
    };
    for cut in cuts.read() {
        if let Ok(lock) = locks.get(cut.target) {
            open(&mut commands, lock.holds);
        } else {
            open(&mut commands, cut.target);
        }
    }
}
