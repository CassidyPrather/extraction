//! Raking with the crowbar: the claw, drawn through a loose small thing,
//! carries it along. Its reach is the way to a stone you'd rather not put
//! a hand near: a hearth in the coals (rake it out, then pocket it before
//! it burns through your glove, or leave it to cool), a stone a step
//! inside a hush or a frost.
//!
//! A held thing doesn't collide with loose things (`interact`, so it
//! never knocks your own kit about), so the claw's catch is here: a loose
//! thing within [`RAKE_REACH`] of the claw takes the claw's way across the
//! ground while the claw moves.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::interact::Held;
use crate::interact::tools::crowbar::{CLAW, Crowbar};
use crate::items::Item;

/// How near the claw (m, across) a thing must be to catch...
pub const RAKE_REACH: f32 = 0.25;
/// ...and how near up or down.
pub const RAKE_DEPTH: f32 = 0.35;
/// The claw moves a thing only this heavy (kg) or lighter: a stone, a
/// bolt, a can; not a crate.
pub const RAKE_MASS: f32 = 3.0;
/// Slower than this (m/s) and the claw's just resting on it.
pub const RAKE_MIN: f32 = 0.3;
/// The fastest it drags a thing along (m/s): a rake, not a bat.
pub const RAKE_MAX: f32 = 4.0;

#[allow(clippy::type_complexity)]
pub fn rake(
    bars: Query<(Entity, &Crowbar, &GlobalTransform), With<Held>>,
    mut loose: Query<
        (&GlobalTransform, &ComputedMass, &mut LinearVelocity),
        (With<Item>, Without<Held>, Without<Crowbar>),
    >,
    time: Res<Time>,
    mut last: Local<std::collections::HashMap<Entity, Vec3>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut now = std::collections::HashMap::new();
    for (bar, crowbar, gt) in &bars {
        let claw = gt.transform_point(CLAW);
        now.insert(bar, claw);
        // A claw seated on a seam is levering, not raking.
        if crowbar.seated.is_some() {
            continue;
        }
        let Some(before) = last.get(&bar) else {
            continue;
        };
        let v = (claw - *before) / dt;
        let across = Vec3::new(v.x, 0.0, v.z);
        if across.length() < RAKE_MIN {
            continue;
        }
        let drag = across.clamp_length_max(RAKE_MAX);
        for (t, mass, mut lv) in &mut loose {
            let rel = t.translation() - claw;
            if Vec2::new(rel.x, rel.z).length() > RAKE_REACH
                || rel.y.abs() > RAKE_DEPTH
                || mass.value() > RAKE_MASS
            {
                continue;
            }
            lv.x = drag.x;
            lv.z = drag.z;
        }
    }
    *last = now;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::{GripRole, Side};
    use crate::vr_harness::Vr;
    use extraction_core::anomaly::AnomalyKind;

    /// A crowbar in the right hand, its claw low over the ground at arm's
    /// length; returns the bar, the claw, and the ground there.
    fn bar_low(vr: &mut Vr, feet: Vec3) -> (Entity, Vec3) {
        let at = feet + Vec3::new(0.3, 1.1, -0.5);
        let bar = vr.spawn("crowbar", Transform::from_translation(at));
        vr.app.world_mut().entity_mut(bar).insert(RigidBody::Static);
        let grip = vr.point(bar, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip, Quat::IDENTITY, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(bar)
            .insert(RigidBody::Dynamic);
        vr.step(20);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(bar), "held");
        (bar, at)
    }

    fn claw(vr: &Vr, bar: Entity) -> Vec3 {
        vr.app
            .world()
            .get::<GlobalTransform>(bar)
            .unwrap()
            .transform_point(CLAW)
    }

    fn pos(vr: &Vr, e: Entity) -> Vec3 {
        vr.app
            .world()
            .get::<GlobalTransform>(e)
            .unwrap()
            .translation()
    }

    /// The claw put down beyond a hearth in a burner's coals and drawn
    /// back toward you rakes it out of the coals in a couple of seconds,
    /// with your hand never in them.
    #[test]
    fn the_crowbar_rakes_a_hearth_out_of_the_coals() {
        let mut vr = Vr::new("training_yard");
        let zone = vr.app.world().resource::<crate::world::Zone>().clone();
        let feet = zone.on_ground(0.0, 30.0);
        vr.stand(feet, 0.0);
        vr.step(4);
        let (bar, _) = bar_low(&mut vr, feet);
        // Lower the claw to the ground, slowly.
        let hand0 = vr.hand_state(Side::Right).world.translation;
        let low = hand0 - Vec3::Y * 0.75;
        for i in 1..=60 {
            vr.move_hand(Side::Right, hand0.lerp(low, i as f32 / 60.0));
            vr.step(1);
        }
        vr.step(10);
        let c = claw(&vr, bar);
        let ground = zone.on_ground(c.x, c.z);
        // A burner just beyond the claw, its stone under it.
        let r = 1.4;
        let centre = Vec3::new(c.x, ground.y, c.z - 0.05);
        let fire = Vec3::new(c.x, ground.y, c.z - r + 0.3);
        crate::anomaly::spawn_anomaly(
            vr.app.world_mut(),
            AnomalyKind::Burner,
            zone.on_ground(fire.x, fire.z),
            r,
            None,
        );
        let stone = vr.spawn(
            "art_hearth",
            Transform::from_translation(centre + Vec3::Y * 0.1),
        );
        vr.step(30);
        let set = pos(&vr, stone);
        assert!(
            (set - claw(&vr, bar)).length() < 0.5,
            "the stone lies by the claw ({set} vs {})",
            claw(&vr, bar)
        );
        assert!(
            vr.app
                .world()
                .get::<crate::anomaly::harvest::Hot>(stone)
                .is_some(),
            "hot in the coals"
        );
        // Draw the bar back toward you, at a walking pace, 1.2 m.
        let from = vr.hand_state(Side::Right).world.translation;
        let steps = 64;
        for i in 1..=steps {
            vr.move_hand(Side::Right, from + Vec3::Z * 1.2 * i as f32 / steps as f32);
            vr.step(1);
        }
        vr.step(20);
        let out = pos(&vr, stone);
        let rel = out - zone.on_ground(fire.x, fire.z);
        println!("raked from {set} to {out}");
        assert!(
            Vec2::new(rel.x, rel.z).length() > r,
            "raked out of the coals ({:.2} m from the fire, radius {r})",
            Vec2::new(rel.x, rel.z).length()
        );
    }

    /// A slow claw resting on a stone, or one seated on a seam, leaves it.
    #[test]
    fn a_resting_claw_leaves_a_stone_be() {
        let mut vr = Vr::new("training_yard");
        let zone = vr.app.world().resource::<crate::world::Zone>().clone();
        let feet = zone.on_ground(0.0, 30.0);
        vr.stand(feet, 0.0);
        vr.step(4);
        let (bar, _) = bar_low(&mut vr, feet);
        let c = claw(&vr, bar);
        let stone = vr.spawn("art_hearth", Transform::from_translation(c));
        vr.app
            .world_mut()
            .entity_mut(stone)
            .insert(RigidBody::Static);
        vr.step(2);
        vr.app
            .world_mut()
            .entity_mut(stone)
            .insert(RigidBody::Dynamic);
        let before = pos(&vr, stone);
        // The hand creeps 5 cm over a second.
        let from = vr.hand_state(Side::Right).world.translation;
        for i in 1..=64 {
            vr.move_hand(Side::Right, from + Vec3::Z * 0.05 * i as f32 / 64.0);
            vr.step(1);
        }
        let moved = pos(&vr, stone) - before;
        assert!(
            Vec2::new(moved.x, moved.z).length() < 0.05,
            "a resting claw doesn't drag ({moved})"
        );
    }
}
