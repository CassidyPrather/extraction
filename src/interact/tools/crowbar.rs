//! The crowbar. Hold it by the end and steady it along the shaft. Put
//! its claw to a seam (a plank door's latch edge, a crate's lid, a robot's
//! armour plate) and it seats there. Then lever: the bar stays on the seam
//! while your hands pull the handle round, and the force is what your hands
//! put in times how far out on the bar they are. Two hands at the end of
//! the bar move what one hand choked up near the claw never will.
//!
//! The seams are `destruction::hooks::Pryable`, and a seated bar sends
//! `destruction::hooks::Pried` every tick it's levered. Whoever owns the
//! thing decides when it gives: wood is Destruction's, the heavy machine's
//! plates are Heavy's. An ordinary robot's armour plates are this module's
//! ([`pryable_plates`], [`pry_plates`]).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{CLICK, KNOCK, TICK, pulse};
use crate::destruction::hooks::PRY_HOLD;
pub use crate::destruction::hooks::{Pried, Pryable};
use crate::interact::{GrabPoint, Grabbable, GripRole, Hand, Held, InteractSystems, Supported};
use crate::items::{Catalog, Item};
use crate::rig::haptics::Haptic;

/// In the bar's space: the handle end (+Z), where the other hand steadies
/// it, and the claw (-Z).
pub const GRIP: Vec3 = Vec3::new(0.0, 0.0, 0.3);
pub const SHAFT: Vec3 = Vec3::new(0.0, 0.0, 0.0);
pub const CLAW: Vec3 = Vec3::new(0.0, 0.012, -0.37);
/// How stiff what you're prying is, N per metre the hand moves the bar
/// round: the force at a hand is this times how far it's drawn the bar off
/// its seated line, and its torque that force times its lever arm.
const STIFF: f32 = 1000.0;
/// One hand alone on the bar puts in this share of what it would with the
/// other hand bracing.
const ONE_HAND: f32 = 0.5;
/// Past this angle off the seated line the claw slips out (radians), and
/// the force counted stops growing at the lesser.
const SLIP: f32 = 1.0;
const MOST: f32 = 0.6;
/// A seam runs this far either side of its point (`Pryable::along`), and
/// the claw seats within this of it (m).
pub const SEAM_RUN: f32 = 0.6;
const SEAT: f32 = 0.06;
/// An ordinary robot's armour plate: the torque, held, that prises it off.
const PLATE_PRY: f32 = 140.0;

pub struct CrowbarPlugin;

impl Plugin for CrowbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(setup_crowbar)
            .add_systems(
                FixedUpdate,
                (lever, pry_plates).chain().after(InteractSystems::Follow),
            )
            .add_systems(Update, pryable_plates);
    }
}

/// A crowbar, and where its claw is seated.
#[derive(Component, Debug, Default)]
pub struct Crowbar {
    pub seated: Option<Seat>,
    /// Seconds to the next strain pulse in the hands.
    buzz: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Seat {
    pub target: Entity,
    /// The claw's point on the seam, world.
    pub pivot: Vec3,
    /// From the claw toward the handle, as seated.
    pub line: Vec3,
}

fn setup_crowbar(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    if catalog.get(item.kind()).id != "crowbar" {
        return;
    }
    commands.entity(add.entity).insert((
        Crowbar::default(),
        Grabbable {
            points: vec![
                GrabPoint::new(Transform::from_translation(GRIP), GripRole::Primary, 0.12),
                GrabPoint::new(Transform::from_translation(SHAFT), GripRole::Support, 0.18),
            ],
        },
    ));
}

/// The bar's torque on what it's seated in: each hand's pull round the
/// claw, by its arm. `line` is the seated line from the claw toward the
/// handle; `hands` the hands on the bar (world).
pub fn torque(pivot: Vec3, line: Vec3, hands: &[Vec3]) -> f32 {
    let one = |p: Vec3| {
        let arm = p - pivot;
        let r = arm.length();
        if r < 1e-3 {
            return 0.0;
        }
        let bend = line.angle_between(arm / r).min(MOST);
        STIFF * bend * r * r
    };
    let sum: f32 = hands.iter().map(|p| one(*p)).sum();
    if hands.len() < 2 { sum * ONE_HAND } else { sum }
}

/// The nearest point of a seam to `p` (world), within its run.
pub fn on_seam(seam: &Pryable, gt: &GlobalTransform, p: Vec3) -> Vec3 {
    let local = gt.affine().inverse().transform_point3(p);
    let along = seam.along.normalize_or(Vec3::Y);
    let t = (local - seam.seam).dot(along).clamp(-SEAM_RUN, SEAM_RUN);
    gt.transform_point(seam.seam + along * t)
}

/// Seat the claw on a seam it touches; lever while seated, holding the bar
/// on the seam. When the seam is gone (it gave), a knock.
#[allow(clippy::type_complexity)]
fn lever(
    mut bars: Query<(
        &mut Crowbar,
        &GlobalTransform,
        Option<&Held>,
        Option<&Supported>,
        &mut Position,
        &mut Rotation,
        &mut LinearVelocity,
        &mut AngularVelocity,
    )>,
    hands: Query<&Hand>,
    pryables: Query<(Entity, &Pryable, &GlobalTransform)>,
    mut pried: MessageWriter<Pried>,
    mut haptics: MessageWriter<Haptic>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut bar, gt, held, support, mut pos, mut rot, mut lin, mut ang) in &mut bars {
        let Some(held) = held else {
            bar.seated = None;
            continue;
        };
        let claw = gt.transform_point(CLAW);
        let Some(seat) = bar.seated else {
            let found = pryables
                .iter()
                .map(|(e, p, pgt)| (e, on_seam(p, pgt, claw)))
                .find(|(_, q)| q.distance(claw) < SEAT);
            if let Some((target, q)) = found {
                let line = (gt.transform_point(GRIP) - claw).normalize_or_zero();
                bar.seated = Some(Seat {
                    target,
                    pivot: q,
                    line,
                });
                pulse(&mut haptics, held.side, CLICK);
            }
            continue;
        };
        let Ok((_, pryable, _)) = pryables.get(seat.target) else {
            // It gave (broke, came off, was taken away).
            bar.seated = None;
            pulse(&mut haptics, held.side, KNOCK);
            if let Some(s) = support {
                pulse(&mut haptics, s.side, KNOCK);
            }
            continue;
        };
        let Ok(main) = hands.get(held.hand) else {
            continue;
        };
        let mut on = vec![main.world.translation];
        if let Some(s) = support.and_then(|s| hands.get(s.hand).ok()) {
            on.push(s.world.translation);
        }
        let reach = main.world.translation - seat.pivot;
        if seat.line.angle_between(reach.normalize_or_zero()) > SLIP || reach.length() > 1.3 {
            // Pulled off the seam.
            bar.seated = None;
            pulse(&mut haptics, held.side, TICK);
            continue;
        }
        let t = torque(seat.pivot, seat.line, &on);
        pried.write(Pried {
            target: seat.target,
            torque: t,
            by: held.player,
        });
        // The strain in the hands, rising with the force.
        bar.buzz -= dt;
        if bar.buzz <= 0.0 && t > 1.0 {
            bar.buzz = 0.1;
            let a = (0.12 + 0.4 * (t / pryable.needs.max(1.0)).min(1.0)).min(0.52);
            pulse(&mut haptics, held.side, (a, 0.02));
        }
        // Held on the seam: the claw stays put and the bar gives only a
        // little toward the hands.
        let toward = seat.line.slerp(reach.normalize_or(seat.line), 0.12);
        let r = rot.0;
        rot.0 = Quat::from_rotation_arc(r * Vec3::Z, toward) * r;
        pos.0 = seat.pivot - rot.0 * CLAW;
        lin.0 = Vec3::ZERO;
        ang.0 = Vec3::ZERO;
    }
}

/// An ordinary robot's armour plates can be prised off. (The heavy
/// machine's come off by their bolts: Heavy's business.)
#[allow(clippy::type_complexity)]
fn pryable_plates(
    mut commands: Commands,
    parts: Query<
        (Entity, &crate::robots::RobotPart),
        (
            Added<crate::robots::RobotPart>,
            Without<Pryable>,
            Without<crate::robots::heavy::HeavyPart>,
        ),
    >,
) {
    for (e, part) in &parts {
        if matches!(part.role, crate::robots::PartRole::Armour(_)) {
            commands.entity(e).insert((
                PlateSeam,
                Pryable {
                    needs: PLATE_PRY,
                    seam: Vec3::ZERO,
                    along: Vec3::X,
                    held: 0.0,
                },
            ));
        }
    }
}

/// On a robot's armour plate whose prying is this module's.
#[derive(Component)]
pub struct PlateSeam;

/// A plate levered hard enough for long enough comes off, as if shot off.
fn pry_plates(
    mut commands: Commands,
    mut pried: MessageReader<Pried>,
    mut plates: Query<
        (
            Entity,
            &mut Pryable,
            &mut crate::robots::RobotPart,
            &GlobalTransform,
        ),
        With<PlateSeam>,
    >,
    mut destroyed: MessageWriter<crate::robots::PartDestroyed>,
    time: Res<Time>,
) {
    let mut levered: Vec<(Entity, f32)> = pried.read().map(|p| (p.target, p.torque)).collect();
    levered.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
    levered.dedup_by_key(|p| p.0);
    for (e, mut seam, mut part, gt) in &mut plates {
        // Let go, or not hard enough: it starts over.
        let torque = levered
            .binary_search_by(|p| p.0.cmp(&e))
            .map_or(0.0, |i| levered[i].1);
        if torque < seam.needs {
            seam.held = 0.0;
            continue;
        }
        seam.held += time.delta_secs();
        if seam.held < PRY_HOLD || !part.alive() {
            continue;
        }
        part.hp = 0.0;
        destroyed.write(crate::robots::PartDestroyed {
            robot: part.robot,
            part: e,
            role: part.role,
            at: gt.translation(),
        });
        commands.entity(e).remove::<(Pryable, PlateSeam)>();
    }
}
