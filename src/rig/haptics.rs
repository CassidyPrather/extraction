//! What your hands feel. Game events become [`Haptic`] pulses on a side;
//! the headset driver plays them on the controllers (flat mode and bots
//! ignore them). Pulses are short and graded: a shot kicks the trigger
//! hand hard and the support hand less; working the action clicks; a
//! magazine seats with a thunk; grabbing something is a tick; being hit
//! shakes both hands.

use bevy::prelude::*;
use extraction_core::firearm::Event as FireEvent;

use super::{LocalPlayer, Side};
use crate::interact::{Held, InteractEvent, Supported};
use crate::weapons::GunEvent;

/// A vibration on one controller.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Haptic {
    pub side: Side,
    /// 0..1.
    pub amplitude: f32,
    pub seconds: f32,
}

pub struct HapticsPlugin;

impl Plugin for HapticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Haptic>()
            .add_systems(Update, (gun_haptics, hand_haptics, hurt_haptics));
    }
}

/// Which hands hold a gun: (the one on the grip, the one supporting it).
fn hands_on(
    gun: Entity,
    held: &Query<(&Held, Option<&Supported>)>,
    locals: &Query<(), With<LocalPlayer>>,
) -> Option<(Side, Option<Side>)> {
    let (h, s) = held.get(gun).ok()?;
    locals
        .contains(h.player)
        .then_some((h.side, s.map(|s| s.side)))
}

fn gun_haptics(
    mut events: MessageReader<GunEvent>,
    held: Query<(&Held, Option<&Supported>)>,
    locals: Query<(), With<LocalPlayer>>,
    mut out: MessageWriter<Haptic>,
) {
    for ev in events.read() {
        let Some((grip, support)) = hands_on(ev.gun, &held, &locals) else {
            continue;
        };
        let (a, t, support_a) = match ev.event {
            FireEvent::Fired { .. } => (1.0, 0.07, 0.6),
            FireEvent::DryFire | FireEvent::Dud => (0.35, 0.02, 0.0),
            FireEvent::MagazineInserted => (0.55, 0.04, 0.3),
            FireEvent::MagazineRemoved => (0.3, 0.03, 0.0),
            FireEvent::BoltClosed | FireEvent::BoltLocked => (0.45, 0.03, 0.25),
            FireEvent::Chambered | FireEvent::Cocked | FireEvent::RoundLoaded => (0.25, 0.02, 0.0),
            FireEvent::ModeChanged(_) => (0.2, 0.015, 0.0),
            FireEvent::Malfunctioned(_) => (0.5, 0.05, 0.3),
            _ => continue,
        };
        out.write(Haptic {
            side: grip,
            amplitude: a,
            seconds: t,
        });
        if let Some(s) = support.filter(|_| support_a > 0.0) {
            out.write(Haptic {
                side: s,
                amplitude: support_a,
                seconds: t,
            });
        }
    }
}

fn hand_haptics(
    mut events: MessageReader<InteractEvent>,
    held: Query<&Held>,
    locals: Query<(), With<LocalPlayer>>,
    mut out: MessageWriter<Haptic>,
) {
    for ev in events.read() {
        match ev {
            InteractEvent::Grabbed { item, side } => {
                let mine = held.get(*item).map_or(true, |h| locals.contains(h.player));
                if mine {
                    out.write(Haptic {
                        side: *side,
                        amplitude: 0.2,
                        seconds: 0.02,
                    });
                }
            }
            InteractEvent::Hover { side, .. } => {
                out.write(Haptic {
                    side: *side,
                    amplitude: 0.08,
                    seconds: 0.012,
                });
            }
            InteractEvent::Pressed { player, .. } if locals.contains(*player) => {
                // Which hand pressed isn't in the event: both feel the click.
                for side in [crate::rig::Side::Left, crate::rig::Side::Right] {
                    out.write(Haptic {
                        side,
                        amplitude: 0.25,
                        seconds: 0.025,
                    });
                }
            }
            InteractEvent::Refused { item } => {
                if let Ok(h) = held.get(*item) {
                    if locals.contains(h.player) {
                        out.write(Haptic {
                            side: h.side,
                            amplitude: 0.4,
                            seconds: 0.05,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn hurt_haptics(
    mut hurts: MessageReader<crate::health::Hurt>,
    locals: Query<(), With<LocalPlayer>>,
    mut out: MessageWriter<Haptic>,
) {
    for h in hurts.read() {
        if !locals.contains(h.player) {
            continue;
        }
        let a = (0.4 + h.amount / 40.0).min(1.0);
        for side in [Side::Left, Side::Right] {
            out.write(Haptic {
                side,
                amplitude: a,
                seconds: 0.12,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Felt(Vec<Haptic>);

    fn feel(mut pulses: MessageReader<Haptic>, mut felt: ResMut<Felt>) {
        felt.0.extend(pulses.read().copied());
    }

    #[test]
    fn a_shot_kicks_the_trigger_hand_hardest() {
        let mut app = crate::headless_app(Default::default(), false);
        app.init_resource::<Felt>().add_systems(Last, feel);
        app.finish();
        app.cleanup();
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let gun = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(gun).insert((
            Held {
                player,
                hand: player,
                side: Side::Right,
                point: 0,
                offset: Transform::IDENTITY,
            },
            Supported {
                hand: player,
                side: Side::Left,
                point: 1,
            },
        ));
        app.world_mut().write_message(GunEvent {
            gun,
            item: "ak74".into(),
            event: FireEvent::Fired { barrel: 0 },
            at: Vec3::ZERO,
        });
        app.update();
        let felt = &app.world().resource::<Felt>().0;
        let right = felt
            .iter()
            .find(|h| h.side == Side::Right)
            .expect("right hand");
        let left = felt
            .iter()
            .find(|h| h.side == Side::Left)
            .expect("left hand");
        assert!(right.amplitude > left.amplitude);
    }
}
