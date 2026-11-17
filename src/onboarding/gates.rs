//! The yard in bays (#89, `docs/YARD_SHAPE.md`): you load standing in the
//! first bay with no truck in sight, and each bay's gate opens once its
//! stations are done or SKIPped. The truck comes only when it's called
//! from the last bay; the settings cards wait on a board by the start.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{GATES, MARK_LINE_FLAT, MARK_LINE_VR, Onboarding, Prop, gate_open};
use crate::raid::comfort_board::{CARDS, ComfortCard};
use crate::raid::truck::Truck;
use crate::raid::{Phase, Raid, Show};
use crate::rig::LocalPlayer;
use crate::world::Zone;
use crate::world::buildings::Door;
use crate::world::yard::{YardGate, gate_index};

/// Through a gate that's still shut (a headset's head and body go through
/// a leaf as through anything): nothing stops you, and it's said once.
pub const BACK_LINE: &str = "Driver: \"Back through the gate. That one's next.\"";

/// The yard starts on foot, out of any truck: the clock runs from the
/// first frame, and nobody's told they stepped off a truck they were
/// never in.
pub(super) fn on_foot(mut raid: ResMut<Raid>) {
    if raid.phase == Phase::AtBase {
        raid.phase = Phase::Out;
        raid.raids += 1;
    }
}

/// Each gate's leaf open or shut as its bay stands ([`gate_open`]): it
/// swings on its own (`buildings::swing_doors`).
pub(super) fn gates(ob: Res<Onboarding>, mut leaves: Query<(&YardGate, &mut Door)>) {
    for (g, mut d) in &mut leaves {
        let open = gate_open(&ob, g.index);
        if d.open != open {
            d.open = open;
        }
    }
}

/// What a hidden truck's part could touch, to put back when it comes.
#[derive(Component, Debug)]
pub struct Unseen(pub CollisionLayers);

/// The truck stays out of sight, and out of the way, till it's called and
/// has come (`raid::truck::drive_truck` moves it off its stop): the
/// designer, "we should not introduce the player to the truck/extract
/// mechanic until the end of the tutorial".
pub(super) fn truck_out_of_sight(
    mut commands: Commands,
    trucks: Query<(Entity, &Truck, &Visibility)>,
    children: Query<&Children>,
    parts: Query<(Option<&CollisionLayers>, Option<&Unseen>)>,
) {
    for (truck, t, vis) in &trucks {
        let hide = t.at_base;
        if hide && *vis != Visibility::Hidden {
            commands.entity(truck).insert(Visibility::Hidden);
        } else if !hide && *vis == Visibility::Hidden {
            commands.entity(truck).insert(Visibility::Inherited);
        }
        for e in children.iter_descendants(truck) {
            let Ok((layers, unseen)) = parts.get(e) else {
                continue;
            };
            match (hide, layers, unseen) {
                // Whatever put its layers back since (a panel unfolded)
                // goes out of reach again, and is remembered.
                (true, Some(l), _) if *l != CollisionLayers::NONE => {
                    commands
                        .entity(e)
                        .insert((Unseen(*l), CollisionLayers::NONE));
                }
                (false, _, Some(u)) => {
                    commands.entity(e).insert(u.0).remove::<Unseen>();
                }
                _ => {}
            }
        }
    }
}

/// The settings cards off the truck's wall onto the start's settings board,
/// as they stand there (columns across, rows up): SETTINGS is the first
/// thing pushed, and the truck isn't come yet.
pub(super) fn settings_to_the_start(
    mut commands: Commands,
    boards: Query<(Entity, &Prop)>,
    cards: Query<(Entity, &ComfortCard, &ChildOf)>,
    trucks: Query<(), With<Truck>>,
) {
    let Some(board) = boards
        .iter()
        .find(|(_, p)| **p == Prop::SettingsBoard)
        .map(|(e, _)| e)
    else {
        return;
    };
    // The columns' middle along the wall, to the board's middle.
    let (lo, hi) = CARDS.iter().fold((f32::MAX, f32::MIN), |(lo, hi), c| {
        (lo.min(c.2), hi.max(c.2))
    });
    let mid = (lo + hi) * 0.5;
    for (e, card, parent) in &cards {
        if !trucks.contains(parent.parent()) {
            continue;
        }
        let Some(&(_, y, z)) = CARDS.iter().find(|c| c.0 == *card) else {
            continue;
        };
        commands
            .entity(e)
            .insert((ChildOf(board), Transform::from_xyz(z - mid, y, 0.02)));
    }
}

/// The gates along the track: number, and how far north (-z) a body is
/// through it.
fn gate_lines(zone: &Zone) -> Vec<(u8, f32)> {
    zone.map
        .placements
        .iter()
        .filter(|p| p.prefab == "yard_gate")
        .filter_map(|p| Some((gate_index(&p.tag)?, p.at[1])))
        .collect()
}

/// Said as you go: through the first gate, how to mark a moment (said
/// once: nobody in the playtest found it); through one still shut, back
/// you go (once).
pub(super) fn through_the_gates(
    mut ob: ResMut<Onboarding>,
    mut raid: ResMut<Raid>,
    zone: Res<Zone>,
    players: Query<&Transform, With<LocalPlayer>>,
    driver: crate::rig::KnownDriver,
    mut lines: Local<Option<Vec<(u8, f32)>>>,
) {
    let Some(p) = players.iter().next() else {
        return;
    };
    let lines = lines.get_or_insert_with(|| gate_lines(&zone));
    let z = p.translation.z;
    // A body's width past the leaf.
    let past = |k: u8| lines.iter().any(|(i, gz)| *i == k && z < gz - 0.6);
    if !ob.told_mark && past(1) && gate_open(&ob, 1) {
        ob.told_mark = true;
        let flat = driver.flat().unwrap_or(false);
        raid.tell(if flat { MARK_LINE_FLAT } else { MARK_LINE_VR }, Show::View);
    }
    let through_shut = (1..=GATES).any(|k| past(k) && !gate_open(&ob, k));
    if through_shut && !ob.told_back {
        ob.told_back = true;
        raid.tell(BACK_LINE, Show::View);
    }
}

/// Where START OVER puts you: where you loaded, facing the first gate.
pub fn start_pose(zone: &Zone) -> Transform {
    let m = zone.marker(&extraction_core::map::MarkerKind::Insertion);
    let at = m.map(|m| m.at).unwrap_or([0.0, 0.0]);
    let yaw = m.map(|m| m.rot).unwrap_or(0.0);
    Transform::from_translation(zone.on_ground(at[0], at[1]) + Vec3::Y * 0.05)
        .with_rotation(Quat::from_rotation_y(-yaw.to_radians()))
}
