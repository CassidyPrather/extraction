//! Haptic shapes: a few pulses in a pattern, for the things a hand should
//! know apart without looking: what it touched or struck (wood, metal,
//! glass, stone, a robot's skin, wire), and the three moments of loading a
//! gun (the bolt home, the magazine seated, a round chambered). The table
//! is in `docs/AUDIO.md`, "Haptics".
//!
//! A shape is played as plain [`Haptic`] pulses at their offsets, so every
//! pulse is on the shared scale (`rig::haptics`) and no pulse goes under
//! what a controller can feel (0.1, 10 ms).

use bevy::prelude::*;

use super::Side;
use super::haptics::Haptic;

/// Pulses at offsets: (seconds after the start, amplitude, seconds long).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape(pub &'static [(f32, f32, f32)]);

/// One dull, longish thud.
pub const WOOD: Shape = Shape(&[(0.0, 0.42, 0.03)]);
/// A sharp hit and a ring that dies away.
pub const METAL: Shape = Shape(&[(0.0, 0.5, 0.012), (0.035, 0.25, 0.012), (0.07, 0.12, 0.012)]);
/// Two quick, light, sharp ticks.
pub const GLASS: Shape = Shape(&[(0.0, 0.3, 0.01), (0.025, 0.3, 0.01)]);
/// Hard and short: it doesn't give at all.
pub const STONE: Shape = Shape(&[(0.0, 0.6, 0.018)]);
/// A soft touch that gives, then holds: a robot's skin.
pub const SKIN: Shape = Shape(&[(0.0, 0.18, 0.025), (0.03, 0.26, 0.035)]);
/// A rattle: wire, chain-link.
pub const WIRE: Shape = Shape(&[
    (0.0, 0.3, 0.01),
    (0.03, 0.22, 0.01),
    (0.06, 0.26, 0.01),
    (0.09, 0.16, 0.01),
]);
/// The bolt slamming home, then settling.
pub const BOLT_HOME: Shape = Shape(&[(0.0, 0.6, 0.015), (0.03, 0.3, 0.025)]);
/// The magazine sliding up the well, then the catch.
pub const MAG_SEAT: Shape = Shape(&[(0.0, 0.2, 0.02), (0.045, 0.55, 0.025)]);
/// A round going in: a double tick.
pub const CHAMBER: Shape = Shape(&[(0.0, 0.3, 0.012), (0.025, 0.2, 0.012)]);
/// The slide or bolt stopping on its catch: one sharp stop, no settle.
pub const BOLT_CATCH: Shape = Shape(&[(0.0, 0.55, 0.01)]);
/// A magazine sliding up the well and stalling short: no catch.
pub const MAG_HALF: Shape = Shape(&[(0.0, 0.2, 0.02), (0.03, 0.12, 0.03)]);

/// Play a shape on a controller, its amplitudes scaled by `scale` (a light
/// touch, a hard slap); pulses that would go under what a hand can feel
/// are dropped.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct HapticShape {
    pub side: Side,
    pub shape: Shape,
    pub scale: f32,
}

pub struct ShapesPlugin;

impl Plugin for ShapesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HapticShape>()
            .init_resource::<Pending>()
            .add_systems(Update, play_shapes);
    }
}

/// Pulses still to come: (when, side, amplitude, seconds).
#[derive(Resource, Default)]
struct Pending(Vec<(f32, Side, f32, f32)>);

fn play_shapes(
    mut shapes: MessageReader<HapticShape>,
    mut pending: ResMut<Pending>,
    mut out: MessageWriter<Haptic>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    for s in shapes.read() {
        for (at, amp, secs) in s.shape.0 {
            let a = (amp * s.scale).min(1.0);
            if a >= 0.1 {
                pending.0.push((now + at, s.side, a, *secs));
            }
        }
    }
    pending.0.retain(|(at, side, amplitude, seconds)| {
        if *at > now {
            return true;
        }
        out.write(Haptic {
            side: *side,
            amplitude: *amplitude,
            seconds: *seconds,
        });
        false
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shape is on the shared scale: nothing under what a hand
    /// feels, a kick's strength only where the scale allows it, and each
    /// shape tells itself apart from the others.
    #[test]
    fn the_shapes_are_on_the_scale_and_distinct() {
        let all = [
            WOOD, METAL, GLASS, STONE, SKIN, WIRE, BOLT_HOME, MAG_SEAT, CHAMBER, BOLT_CATCH,
            MAG_HALF,
        ];
        for s in all {
            for (at, amp, secs) in s.0 {
                assert!(*at >= 0.0 && *at < 0.2, "{s:?} runs short");
                assert!((0.1..=0.6).contains(amp), "{s:?}: {amp}");
                assert!(*secs >= 0.01, "{s:?}: {secs}");
            }
        }
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b, "two shapes alike");
            }
        }
    }

    /// A shape plays out as pulses at its offsets, scaled, dropping what's
    /// too faint to feel.
    #[test]
    fn a_shape_plays_out_over_time() {
        #[derive(Resource, Default)]
        struct Felt(Vec<(f32, f32)>);
        fn feel(mut h: MessageReader<Haptic>, mut f: ResMut<Felt>, t: Res<Time>) {
            f.0.extend(h.read().map(|h| (t.elapsed_secs(), h.amplitude)));
        }
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(10),
            ))
            .add_message::<Haptic>()
            .init_resource::<Felt>()
            .add_plugins(ShapesPlugin)
            .add_systems(Update, feel.after(play_shapes));
        app.update();
        app.world_mut().write_message(HapticShape {
            side: Side::Right,
            shape: METAL,
            scale: 0.5,
        });
        for _ in 0..20 {
            app.update();
        }
        let felt = &app.world().resource::<Felt>().0;
        // 0.5 and 0.25 felt; 0.06 too faint.
        assert_eq!(felt.len(), 2, "{felt:?}");
        assert!((felt[0].1 - 0.25).abs() < 1e-4 && (felt[1].1 - 0.125).abs() < 1e-4);
        assert!(
            felt[1].0 - felt[0].0 >= 0.03,
            "the ring comes after the hit"
        );
    }
}
