//! The crowbar. Hold it by the end and steady it along the shaft. Put
//! its claw to a seam (a robot's armour plate, the latch edge of a jammed
//! door) and it seats there. Then lever: the bar stays on the seam while
//! your hands pull the handle round, and the force is what your hands put
//! in times how far out on the bar they are. Two hands at the end of the
//! bar move what one hand choked up near the claw never will. Keep up the
//! force for a moment and it gives.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{CLICK, KNOCK, TICK, pulse};
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
/// How long the force must be kept up for the thing to give, seconds.
const GIVE: f32 = 0.25;

pub struct CrowbarPlugin;

impl Plugin for CrowbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Pried>()
            .add_message::<PriedFree>()
            .add_observer(setup_crowbar)
            .add_systems(
                FixedUpdate,
                (lever, give_way).chain().after(InteractSystems::Follow),
            )
            .add_systems(Update, (pryable_plates, pryable_doors));
    }
}

/// Something a crowbar can be put to: its seam (where the claw seats, in
/// its own space, within `radius`) and the torque it takes to give, N·m.
#[derive(Component, Clone, Copy, Debug)]
pub struct Pryable {
    pub needs: f32,
    pub seam: Vec3,
    pub radius: f32,
}

/// Each tick a seated bar is levered: how hard (N·m), by whom.
#[derive(Message, Clone, Copy, Debug)]
pub struct Pried {
    pub target: Entity,
    pub torque: f32,
    pub by: Entity,
}

/// It gave.
#[derive(Message, Clone, Copy, Debug)]
pub struct PriedFree {
    pub target: Entity,
    pub by: Entity,
}

/// A crowbar, and where its claw is seated.
#[derive(Component, Debug, Default)]
pub struct Crowbar {
    pub seated: Option<Seat>,
    /// Seconds the force has been enough.
    strain: f32,
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

/// Seat the claw on a seam it touches; lever while seated, holding the bar
/// on the seam; give when the force has been enough for long enough.
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
    mut free: MessageWriter<PriedFree>,
    mut haptics: MessageWriter<Haptic>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut bar, gt, held, support, mut pos, mut rot, mut lin, mut ang) in &mut bars {
        let Some(held) = held else {
            bar.seated = None;
            bar.strain = 0.0;
            continue;
        };
        let claw = gt.transform_point(CLAW);
        let Some(seat) = bar.seated else {
            let found = pryables
                .iter()
                .find(|(_, p, pgt)| pgt.transform_point(p.seam).distance(claw) < p.radius);
            if let Some((target, _, _)) = found {
                let line = (gt.transform_point(GRIP) - claw).normalize_or_zero();
                bar.seated = Some(Seat {
                    target,
                    pivot: claw,
                    line,
                });
                pulse(&mut haptics, held.side, CLICK);
            }
            continue;
        };
        let Ok((_, pryable, _)) = pryables.get(seat.target) else {
            bar.seated = None;
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
            bar.strain = 0.0;
            pulse(&mut haptics, held.side, TICK);
            continue;
        }
        let t = torque(seat.pivot, seat.line, &on);
        pried.write(Pried {
            target: seat.target,
            torque: t,
            by: held.player,
        });
        bar.strain = if t >= pryable.needs {
            bar.strain + dt
        } else {
            (bar.strain - dt).max(0.0)
        };
        if bar.strain >= GIVE {
            free.write(PriedFree {
                target: seat.target,
                by: held.player,
            });
            bar.seated = None;
            bar.strain = 0.0;
            pulse(&mut haptics, held.side, KNOCK);
            if let Some(s) = support {
                pulse(&mut haptics, s.side, KNOCK);
            }
            continue;
        }
        // The strain in the hands, rising with the force.
        bar.buzz -= dt;
        if bar.buzz <= 0.0 && t > 1.0 {
            bar.buzz = 0.1;
            let a = (0.12 + 0.4 * (t / pryable.needs).min(1.0)).min(0.52);
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

/// A robot's armour plates can be prised off.
fn pryable_plates(
    mut commands: Commands,
    parts: Query<(Entity, &crate::robots::RobotPart), Added<crate::robots::RobotPart>>,
) {
    for (e, part) in &parts {
        if matches!(part.role, crate::robots::PartRole::Armour(_)) {
            commands.entity(e).insert(Pryable {
                needs: 140.0,
                seam: Vec3::ZERO,
                radius: 0.25,
            });
        }
    }
}

/// A door is prised at its latch edge.
fn pryable_doors(
    mut commands: Commands,
    doors: Query<
        (Entity, &crate::world::buildings::DoorLeaf),
        Added<crate::world::buildings::DoorLeaf>,
    >,
) {
    for (e, leaf) in &doors {
        let (w, h) = (leaf.0.width, leaf.0.height);
        commands.entity(e).insert(Pryable {
            needs: 100.0,
            seam: Vec3::new(w - 0.04, h * 0.5, 0.0),
            radius: h * 0.5,
        });
    }
}

/// What gives: a plate comes off its robot; a jammed door bursts open,
/// its wedge knocked out. (A door the power holds doesn't move.)
#[allow(clippy::type_complexity)]
fn give_way(
    mut commands: Commands,
    mut free: MessageReader<PriedFree>,
    mut parts: Query<(&mut crate::robots::RobotPart, &GlobalTransform)>,
    mut destroyed: MessageWriter<crate::robots::PartDestroyed>,
    mut doors: Query<(
        &mut crate::world::buildings::Door,
        &GlobalTransform,
        Option<&crate::robots::doors::Wedged>,
        Has<crate::world::underground::PowerLocked>,
    )>,
) {
    for f in free.read() {
        if let Ok((mut part, gt)) = parts.get_mut(f.target) {
            if part.alive() {
                part.hp = 0.0;
                destroyed.write(crate::robots::PartDestroyed {
                    robot: part.robot,
                    part: f.target,
                    role: part.role,
                    at: gt.translation(),
                });
            }
            continue;
        }
        if let Ok((mut door, gt, wedged, locked)) = doors.get_mut(f.target) {
            if locked {
                continue;
            }
            if let Some(w) = wedged {
                commands
                    .entity(w.0)
                    .remove::<(crate::robots::doors::Wedging, RigidBodyDisabled)>()
                    .insert(LinearVelocity(gt.rotation() * Vec3::new(0.0, 1.0, 2.5)));
                commands
                    .entity(f.target)
                    .remove::<crate::robots::doors::Wedged>();
            }
            door.open = true;
        }
    }
}
