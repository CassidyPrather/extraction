//! Flat-screen interaction: look at a thing and press a key, instead of
//! reaching for it.
//!
//! | Key | Does |
//! | --- | --- |
//! | E | Grab what the crosshair is on (right hand, else left) |
//! | F | Second hand on the held gun's handguard (toggle) |
//! | Q (hold to throw) | Drop/throw the right-hand item; Shift+Q the left |
//! | G | Stow the right-hand item (body slot, else backpack) |
//! | 1-9, 0 | Draw from body slot 1-10 |
//! | Tab | Open/close the backpack |
//! | LMB / RMB | Trigger / aim down sights |
//! | B / N | Right-hand face buttons (mag release / slide stop) |

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use super::body::{BodySlot, ToggleBackpack};
use super::{Grabbable, GripRole, Hand, HandAction, Held};
use crate::rig::{LocalPlayer, RigHand, RigHead, RigInput, Side};
use crate::world::Layer;

pub struct FlatInteractPlugin;

impl Plugin for FlatInteractPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlatTarget>()
            .add_systems(Update, (aim_crosshair, flat_actions).chain())
            .add_systems(
                PreUpdate,
                flat_buttons
                    .after(crate::rig::RigDriverSystems)
                    .run_if(crate::rig::flat_driving),
            );
    }
}

/// What the crosshair is on.
#[derive(Resource, Default, Debug)]
pub struct FlatTarget {
    pub entity: Option<Entity>,
    pub point: Vec3,
    pub distance: f32,
}

/// Reach from the eyes, metres.
const REACH: f32 = 2.4;

fn aim_crosshair(
    heads: Query<&GlobalTransform, (With<RigHead>, With<Camera3d>)>,
    spatial: SpatialQuery,
    grabbable: Query<(), With<Grabbable>>,
    held: Query<(), With<Held>>,
    parents: Query<&ChildOf>,
    bags: Query<(Entity, &super::body::Backpack, &GlobalTransform)>,
    item_poses: Query<&GlobalTransform>,
    mut target: ResMut<FlatTarget>,
) {
    *target = FlatTarget::default();
    let Some(head) = heads.iter().next() else {
        return;
    };
    // An open backpack: its contents have no live colliders, so test the
    // ray against little spheres around them, and the bag itself.
    let origin = head.translation();
    let dir = head.forward().as_vec3();
    let ray_hits = |centre: Vec3, radius: f32| -> Option<f32> {
        let t = (centre - origin).dot(dir);
        if t < 0.0 || t > REACH {
            return None;
        }
        ((origin + dir * t).distance(centre) <= radius).then_some(t)
    };
    let mut best: Option<(f32, Entity)> = None;
    for (bag_e, bag, bgt) in &bags {
        if !bag.open {
            continue;
        }
        for item in &bag.contents {
            if let Ok(gt) = item_poses.get(*item) {
                if let Some(t) = ray_hits(gt.translation(), 0.07) {
                    if best.is_none_or(|b| t < b.0) {
                        best = Some((t, *item));
                    }
                }
            }
        }
        if best.is_none() {
            if let Some(t) = ray_hits(bgt.translation(), 0.2) {
                best = Some((t, bag_e));
            }
        }
    }
    if let Some((t, e)) = best {
        target.entity = Some(e);
        target.point = origin + dir * t;
        target.distance = t;
        return;
    }
    let filter = SpatialQueryFilter::from_mask([Layer::Item, Layer::World]);
    let Some(hit) = spatial.cast_ray_predicate(
        head.translation(),
        head.forward(),
        REACH,
        true,
        &filter,
        &|e| !held.contains(e),
    ) else {
        return;
    };
    // The collider may be on a child of the grabbable.
    let mut e = hit.entity;
    for _ in 0..4 {
        if grabbable.contains(e) {
            target.entity = Some(e);
            break;
        }
        match parents.get(e) {
            Ok(p) => e = p.parent(),
            Err(_) => break,
        }
    }
    target.point = head.translation() + head.forward() * hit.distance;
    target.distance = hit.distance;
}

/// Seconds Q has been held, for throw strength.
#[derive(Default)]
struct ThrowCharge(f32);

fn flat_actions(
    keys: Res<ButtonInput<KeyCode>>,
    target: Res<FlatTarget>,
    players: Query<(Entity, &Children), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand)>,
    grabbables: Query<&Grabbable>,
    slots: Query<&BodySlot>,
    mut actions: MessageWriter<HandAction>,
    mut toggle: MessageWriter<ToggleBackpack>,
    mut uses: MessageWriter<crate::items::usage::UseItem>,
    mut charge: Local<ThrowCharge>,
    time: Res<Time>,
) {
    let Some((player, children)) = players.iter().next() else {
        return;
    };
    let hand = |side: Side| {
        children.iter().find_map(|c| {
            hands
                .get(c)
                .ok()
                .filter(|(rh, _)| rh.side == side)
                .map(|(_, h)| h)
        })
    };
    let (Some(right), Some(left)) = (hand(Side::Right), hand(Side::Left)) else {
        return;
    };
    let empty = |h: &Hand| h.holding.is_none() && h.supporting.is_none() && h.handle.is_none();

    if keys.just_pressed(KeyCode::KeyE) {
        if let Some(item) = target.entity {
            let side = if empty(right) {
                Some(Side::Right)
            } else if empty(left) {
                Some(Side::Left)
            } else {
                None
            };
            if let Some(side) = side {
                actions.write(HandAction::Grab {
                    player,
                    side,
                    item,
                    point: None,
                });
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyF) {
        if left.supporting.is_some() {
            actions.write(HandAction::Release {
                player,
                side: Side::Left,
                throw: 0.0,
            });
        } else if let (Some(item), true) = (right.holding, empty(left)) {
            if let Ok(g) = grabbables.get(item) {
                if let Some(point) = g.points.iter().position(|p| p.role == GripRole::Support) {
                    actions.write(HandAction::Grab {
                        player,
                        side: Side::Left,
                        item,
                        point: Some(point),
                    });
                }
            }
        }
    }

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if keys.pressed(KeyCode::KeyQ) {
        charge.0 += time.delta_secs();
    }
    if keys.just_released(KeyCode::KeyQ) {
        let throw = if charge.0 > 0.25 {
            (charge.0 * 6.0).min(9.0)
        } else {
            0.0
        };
        charge.0 = 0.0;
        let side = if shift { Side::Left } else { Side::Right };
        // Dropping the main item lets go of its support too.
        if side == Side::Right && left.supporting.is_some() {
            actions.write(HandAction::Release {
                player,
                side: Side::Left,
                throw: 0.0,
            });
        }
        actions.write(HandAction::Release {
            player,
            side,
            throw,
        });
    }

    if keys.just_pressed(KeyCode::KeyG) && right.holding.is_some() {
        if left.supporting.is_some() {
            actions.write(HandAction::Release {
                player,
                side: Side::Left,
                throw: 0.0,
            });
        }
        actions.write(HandAction::Stow {
            player,
            side: Side::Right,
        });
    }

    if keys.just_pressed(KeyCode::KeyU) {
        if let Some(item) = right.holding {
            uses.write(crate::items::usage::UseItem { player, item });
        }
    }

    if keys.just_pressed(KeyCode::Tab) {
        toggle.write(ToggleBackpack { player });
    }

    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
        KeyCode::Digit0,
    ];
    for (i, key) in digits.iter().enumerate() {
        if keys.just_pressed(*key) && slots.iter().any(|s| s.0 == i) {
            let side = if empty(right) {
                Side::Right
            } else {
                Side::Left
            };
            actions.write(HandAction::Draw {
                player,
                side,
                slot: i,
            });
        }
    }
}

/// Mouse buttons and face buttons into the rig input, and the aim pose.
fn flat_buttons(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    mut players: Query<
        (&mut RigInput, &crate::weapons::AimHint, &Children),
        (With<LocalPlayer>, Without<crate::bot::Bot>),
    >,
    hands: Query<(&RigHand, &Hand)>,
) {
    let captured = cursor
        .single()
        .map(|c| c.grab_mode != CursorGrabMode::None)
        .unwrap_or(false);
    for (mut input, hint, children) in &mut players {
        let head = input.head;
        let supporting = children.iter().any(|c| {
            hands
                .get(c)
                .is_ok_and(|(rh, h)| rh.side == Side::Left && h.supporting.is_some())
        });
        let aiming = captured && mouse.pressed(MouseButton::Right);
        let right = input.hand_mut(Side::Right);
        right.trigger = if captured && mouse.pressed(MouseButton::Left) {
            1.0
        } else {
            0.0
        };
        right.primary = keys.pressed(KeyCode::KeyB);
        right.secondary = keys.pressed(KeyCode::KeyN);
        if aiming {
            // Put the sights on the eye line, a forearm's length out.
            let sight = hint.sight;
            let eye_target = Transform::from_translation(
                head.translation + head.rotation * Vec3::new(0.0, 0.0, -0.32),
            )
            .with_rotation(head.rotation);
            right.pose = Transform::from_matrix(
                (eye_target.compute_affine() * sight.compute_affine().inverse()).into(),
            );
        }
        // A steadying hand goes where the handguard is.
        if let (true, Some(support)) = (supporting, hint.support) {
            let right_pose = input.hand(Side::Right).pose;
            input.hand_mut(Side::Left).pose =
                Transform::from_translation(right_pose.transform_point(support))
                    .with_rotation(right_pose.rotation);
        }
    }
}
