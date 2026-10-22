//! What leaves the muzzle besides the round (client side): a flash for a
//! frame or three, sized by the gun's kick (a shotgun's wide), a puff of
//! smoke that drifts off on the valley's wind, and, once a barrel is hot
//! from fast fire, a wisp curling off it that a dirty gun keeps up longer.
//! The flash's light is the ballistics' own.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use extraction_core::firearm::Event as FireEvent;
use std::collections::HashMap;

use super::impacts::{Bit, FxAssets, FxPool, SPRAY, Spray, spray_bits};
use super::{Gun, GunEvent};

/// Heat a shot adds, per unit of recoil's square root.
pub const HEAT_PER_SHOT: f32 = 0.1;
/// Heat lost a second.
pub const COOL_RATE: f32 = 0.15;
/// Above this the barrel smokes: about ten quick rounds from an AK; never
/// from deliberate semi-auto.
pub const WISP_HEAT: f32 = 0.6;

/// A gun's barrel heat and when it next puts out a wisp.
#[derive(Default)]
pub(crate) struct Barrels(HashMap<Entity, (f32, f32)>);

pub(crate) fn muzzle_fx(
    mut commands: Commands,
    mut events: MessageReader<GunEvent>,
    guns: Query<(&Gun, &GlobalTransform, Option<&super::gunsmith::Built>)>,
    assets: Option<Res<FxAssets>>,
    pool: Option<ResMut<FxPool>>,
    time: Res<Time>,
    mut barrels: Local<Barrels>,
) {
    let (Some(a), Some(mut pool)) = (assets, pool) else {
        events.clear();
        return;
    };
    let pool = &mut *pool;
    let now = time.elapsed_secs();
    for ev in events.read() {
        let FireEvent::Fired { barrel } = ev.event else {
            continue;
        };
        let Ok((gun, gt, built)) = guns.get(ev.gun) else {
            continue;
        };
        let def = &gun.firearm.def;
        let mut muzzle = gun.layout.muzzle;
        if def.barrels > 1 {
            muzzle.x += if barrel == 0 { -0.0128 } else { 0.0128 };
        }
        let at = gt.transform_point(muzzle);
        let fwd = gt.rotation() * Vec3::NEG_Z;
        let r = def.recoil.max(0.5);
        // The flash: a cone of fire along the bore, a frame or three;
        // a cut barrel's powder still burning as it leaves.
        let len = (0.06 + 0.035 * r) * built.map_or(1.0, |b| b.flash.sqrt());
        let wide = if def.projectiles > 1 { 0.8 } else { 0.45 };
        let size = Vec3::new(len * wide, len * wide, len);
        let flash = commands
            .spawn((
                Bit {
                    vel: Vec3::ZERO,
                    age: 0.0,
                    life: 0.035,
                    size,
                    grow: 0.0,
                    gravity: 0.0,
                    drag: 0.0,
                    align: false,
                    spin: Vec3::ZERO,
                    wind: 0.0,
                },
                Mesh3d(a.cone.clone()),
                MeshMaterial3d(a.flash.clone()),
                Transform::from_translation(at + fwd * len * 0.5)
                    .with_rotation(gt.rotation())
                    .with_scale(size),
                NotShadowCaster,
                NotShadowReceiver,
            ))
            .id();
        if let Some(old) = pool.bits.push(flash) {
            commands.entity(old).try_despawn();
        }
        // Smoke out of the muzzle, slowing, spreading, off on the wind.
        let smoke = Spray {
            n: 2 + (r as usize).min(3),
            mesh: |a| a.ball.clone(),
            mat: |a| a.smoke.clone(),
            size: Vec3::splat(0.05 + 0.01 * r),
            out: 1.2 + 0.3 * r,
            scatter: 0.25,
            life: 1.8,
            grow: 3.5,
            gravity: -0.2,
            drag: 2.5,
            wind: 1.0,
            ..SPRAY
        };
        spray_bits(&mut commands, pool, &a, &smoke, at, fwd, Vec3::ZERO);
        let heat = &mut barrels.0.entry(ev.gun).or_insert((0.0, now)).0;
        *heat = (*heat + HEAT_PER_SHOT * r.sqrt()).min(2.0);
    }
    // Hot barrels smoke; dirty ones more.
    let dt = time.delta_secs();
    barrels.0.retain(|gun_e, (heat, next)| {
        *heat -= COOL_RATE * dt;
        if *heat <= 0.0 {
            return false;
        }
        if *heat < WISP_HEAT || now < *next {
            return true;
        }
        let Ok((gun, gt, _)) = guns.get(*gun_e) else {
            return false;
        };
        let at = gt.transform_point(gun.layout.muzzle);
        let wisp = Spray {
            n: 1,
            mesh: |a| a.ball.clone(),
            mat: |a| a.smoke.clone(),
            size: Vec3::splat(0.02),
            out: 0.15,
            scatter: 0.05,
            life: 1.6,
            grow: 2.5,
            gravity: -0.4,
            drag: 2.0,
            wind: 0.6,
            ..SPRAY
        };
        spray_bits(&mut commands, pool, &a, &wisp, at, Vec3::Y, Vec3::ZERO);
        let dirt = 1.0 + gun.firearm.fouling;
        *next = now + 0.3 / (*heat * dirt);
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A burst heats the barrel past smoking; deliberate fire never does.
    #[test]
    fn fast_fire_smokes_and_slow_fire_doesnt() {
        let heat_after = |shots: usize, gap: f32, recoil: f32| {
            let mut h: f32 = 0.0;
            for _ in 0..shots {
                h = (h + HEAT_PER_SHOT * recoil.sqrt()).min(2.0);
                h = (h - COOL_RATE * gap).max(0.0);
            }
            h
        };
        // An AK on auto: ten rounds a second.
        assert!(heat_after(10, 0.1, 2.2) > WISP_HEAT);
        // A round a second, all day.
        assert!(heat_after(60, 1.0, 2.2) < WISP_HEAT);
        // A pistol emptied quickly smokes a little; at a walk it doesn't.
        assert!(heat_after(8, 0.25, 1.0) < heat_after(8, 0.1, 2.2));
    }
}
