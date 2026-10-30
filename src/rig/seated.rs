//! Seated play (`Comfort::seated`): the view and the hands are lifted by
//! however far the seated head is below a standing eye, so the world sits
//! where it would standing, every body slot and the crate at the heights
//! they're built for. The crouch button (the right stick pulled down)
//! brings the view down to the floor instead: what a seated player can't
//! do by squatting.
//!
//! Standing in the headset, the crouch toggle brings the view down too
//! ([`STAND_CROUCH_DROP`]): it was only slower, and nobody could tell they
//! were crouched (playtest 28 October). The tracked space sinks, view and
//! hands together, as the seated crouch does.
//!
//! The lift is applied to [`RigInput`] after the drivers write it (they
//! write raw, unlifted poses and zero `RigInput::lifted`); the headset's
//! tracking root rides up by the same amount (`xr`), so what's seen
//! matches.

use bevy::prelude::*;

use super::comfort::Comfort;
use super::{LocalPlayer, RigDriver, RigInput, Side};

/// A standing eye, where the seated view is lifted to.
pub const STAND_EYE: f32 = 1.65;
/// Crouched, seated: the eye this low, so a hand held down beside the
/// chair, at shin height and without leaning, reaches the ground (and a
/// little below it, on a slope).
pub const SEATED_CROUCH_EYE: f32 = 0.7;
/// Crouched standing in the headset: the view this much lower (m).
pub const STAND_CROUCH_DROP: f32 = 0.35;
/// How fast the lift moves when the crouch button goes (m/s): quick, but
/// not a jump.
pub const LIFT_RATE: f32 = 3.0;

/// The seated head's upright height and the lift applied.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Seat {
    /// The highest the (unlifted) head has been lately: sitting up
    /// straight.
    pub high: f32,
    pub lift: f32,
    /// The head's highest in each of the last [`WINDOW`] seconds (0: none
    /// yet).
    pub window: [f32; WINDOW],
    pub clock: f32,
}

/// Seconds the upright seated height is measured over.
pub const WINDOW: usize = 8;

pub struct SeatedPlugin;

impl Plugin for SeatedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, seat_lift.after(super::RigDriverSystems));
    }
}

/// Lifts the local player's head and hands by the seat's lift.
pub fn seat_lift(
    mut commands: Commands,
    comfort: Option<Res<Comfort>>,
    driver: Option<Res<RigDriver>>,
    mut players: Query<
        (
            Entity,
            &mut RigInput,
            Option<&mut Seat>,
            Has<crate::bot::Bot>,
        ),
        With<LocalPlayer>,
    >,
    time: Res<Time>,
) {
    let seated = comfort.is_some_and(|c| c.seated);
    let flat = driver.is_some_and(|d| *d == RigDriver::Flat);
    let headset = driver.is_some_and(|d| *d == RigDriver::Xr);
    let dt = time.delta_secs();
    for (e, mut input, seat, bot) in &mut players {
        let Some(mut seat) = seat else {
            commands.entity(e).insert(Seat::default());
            continue;
        };
        // The flat rig's head is made up (it crouches by key): not lifted.
        let on = seated && (bot || !flat);
        let raw = input.head.translation.y - input.lifted;
        // Measured only while seated, as the highest the head's been over
        // the last few seconds: a lean doesn't lower it, sitting down (after
        // standing a moment) does, soon.
        if on {
            seat.clock += dt;
            if seat.clock >= 1.0 {
                seat.clock = 0.0;
                seat.window.rotate_right(1);
                seat.window[0] = raw;
            }
            seat.window[0] = seat.window[0].max(raw);
            seat.high = seat.window.iter().copied().fold(0.0, f32::max);
        } else {
            *seat = Seat::default();
        }
        let target = if on {
            let eye = if input.crouch {
                SEATED_CROUCH_EYE
            } else {
                STAND_EYE
            };
            (eye - seat.high).clamp(-1.0, 1.2)
        } else if headset && input.crouch {
            -STAND_CROUCH_DROP
        } else {
            0.0
        };
        let step = LIFT_RATE * dt;
        seat.lift += (target - seat.lift).clamp(-step, step);
        let d = seat.lift - input.lifted;
        if d != 0.0 {
            input.head.translation.y += d;
            for side in Side::BOTH {
                input.hand_mut(side).pose.translation.y += d;
            }
            input.lifted = seat.lift;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standing in the headset, crouched: the view comes down (eased), and
    /// back up on standing. It was only slower before, and you couldn't
    /// tell (playtest 28 October).
    #[test]
    fn crouching_standing_in_the_headset_lowers_the_view() {
        let mut vr = crate::vr_harness::Vr::new("marrow_creek");
        let at = vr.feet();
        vr.stand(at, 0.0);
        vr.step(20);
        let p = vr.player;
        let set = |vr: &mut crate::vr_harness::Vr, crouch: bool, frames: usize| {
            for _ in 0..frames {
                vr.app.world_mut().get_mut::<RigInput>(p).unwrap().crouch = crouch;
                vr.step(1);
            }
        };
        set(&mut vr, false, 10);
        let lift = |vr: &crate::vr_harness::Vr| vr.app.world().get::<RigInput>(p).unwrap().lifted;
        assert_eq!(lift(&vr), 0.0, "standing: no drop");
        // Eased, not a jump.
        set(&mut vr, true, 3);
        let early = lift(&vr);
        assert!(early < 0.0 && early > -STAND_CROUCH_DROP, "easing: {early}");
        set(&mut vr, true, 60);
        assert!(
            (lift(&vr) + STAND_CROUCH_DROP).abs() < 1e-3,
            "down: {}",
            lift(&vr)
        );
        set(&mut vr, false, 60);
        assert!(lift(&vr).abs() < 1e-3, "back up: {}", lift(&vr));
    }
}
