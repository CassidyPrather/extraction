//! The shot as robots hear it (`extraction_core::report`): its report
//! from the muzzle, by cartridge, barrel, load and what's on the muzzle;
//! and, ahead of it, the crack of a round faster than sound, heard only
//! near its flight and from somewhere along it, so a robot downrange that
//! hears the crack but not the report goes to look in the wrong place.
//!
//! A suppressor wears with every shot (a can in a few dozen, the PBS in a
//! long while), giving back its quiet as it goes, and fouls the gun it's
//! on. Robots' own guns are heard by `robots::senses::gunshot_reach`.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::report::{self, Suppressor};
use std::collections::HashMap;

use super::Gun;
use super::ballistics::Shot;
use crate::interact::{Socket, SocketKind, SocketOwner};
use crate::items::{Catalog, Item};
use crate::robots::Noise;
use crate::world::Layer;
use extraction_core::items::{ItemKind, Mount};

/// A suppressed gun fouls this many times as fast (the gas held back in
/// the bore).
pub const SUPPRESSED_FOULS: f32 = 2.0;
/// Cracks are made this far apart along a round's line (m), from the
/// first this far out...
pub const CRACK_EVERY: f32 = 40.0;
pub const CRACK_FROM: f32 = 20.0;
/// ...this many at most.
pub const CRACKS: u32 = 6;
/// A gun firing fast cracks no more often than this (s).
pub const CRACK_GAP: f32 = 0.25;

pub struct ReportPlugin;

impl Plugin for ReportPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            shot_noise
                .after(super::ballistics::BallisticsSystems)
                .before(crate::robots::senses::perceive),
        );
    }
}

/// The suppressor on `gun`'s muzzle: its entity and what it is.
fn muzzle_can(
    gun: Entity,
    sockets: &Query<(&Socket, &SocketOwner)>,
    kind_of: impl Fn(Entity) -> Option<ItemKind>,
    catalog: &Catalog,
) -> Option<(Entity, Suppressor)> {
    sockets.iter().find_map(|(s, o)| {
        if o.0 != gun || s.kind != SocketKind::Mount(Mount::Muzzle) {
            return None;
        }
        let e = s.occupant?;
        let def = catalog.get(kind_of(e)?);
        def.has_tag("suppressor").then_some((
            e,
            if def.has_tag("homemade") {
                report::CAN
            } else {
                report::PBS
            },
        ))
    })
}

/// Every shot's report and crack, as robots hear them; a suppressor's wear.
#[allow(clippy::too_many_arguments)]
fn shot_noise(
    mut shots: MessageReader<Shot>,
    mut noise: MessageWriter<Noise>,
    mut guns: Query<(&mut Gun, &Item)>,
    mut items: Query<&mut Item, Without<Gun>>,
    sockets: Query<(&Socket, &SocketOwner)>,
    catalog: Res<Catalog>,
    spatial: SpatialQuery,
    hounds: Query<(), With<crate::robots::hound::Hound>>,
    suppressed: Query<(), With<crate::gadgets::Suppressed>>,
    reports: Query<&super::armoury::Report>,
    time: Res<Time>,
    mut last_crack: Local<HashMap<Entity, f32>>,
) {
    let now = time.elapsed_secs();
    last_crack.retain(|e, t| now - *t < CRACK_GAP && guns.contains(*e));
    for s in shots.read() {
        // A hound's bite is a Shot, not a gunshot.
        if hounds.contains(s.gun) {
            continue;
        }
        let k = reports.get(s.gun).map_or(1.0, |r| r.0);
        let Ok((mut gun, item)) = guns.get_mut(s.gun) else {
            // A robot's gun: by its round, as robots always heard it.
            let cut = if suppressed.contains(s.gun) {
                crate::gadgets::SUPPRESSED_REACH
            } else {
                1.0
            };
            noise.write(Noise {
                at: s.origin,
                radius: crate::robots::senses::gunshot_reach(s.speed, s.damage, s.projectiles)
                    * cut
                    * k,
                source: s.shooter,
            });
            continue;
        };
        let load = gun.firearm.last_fired;
        let cartridge = gun.firearm.def.cartridge.clone();
        let id = &catalog.get(item.kind()).id;
        let barrel = report::barrel_mm(id).unwrap_or_else(|| report::full_burn_mm(&cartridge));
        let can = muzzle_can(
            s.gun,
            &sockets,
            |e| items.get(e).ok().map(|i| i.kind()),
            &catalog,
        );
        let cut = match can {
            Some((e, sup)) => {
                let Ok(mut it) = items.get_mut(e) else {
                    continue;
                };
                let cut = sup.cut_at(it.condition);
                it.condition = (it.condition - sup.wear_per_shot()).max(0.0);
                // The gas it holds back fouls the bore.
                let f = &mut gun.firearm;
                f.fouling =
                    (f.fouling + f.def.fouling_per_shot * (SUPPRESSED_FOULS - 1.0)).min(1.0);
                cut
            }
            None => 0.0,
        };
        // The crack first, so a robot that hears the report too takes
        // the report's word for where it came from.
        let speed = load.speed(s.speed);
        let crack = report::crack_reach_m(load.mass_g(&cartridge), speed);
        if crack > 0.0 && !last_crack.contains_key(&s.gun) {
            last_crack.insert(s.gun, now);
            let filter = SpatialQueryFilter::from_mask([Layer::World]);
            let dir = Dir3::new(s.direction).unwrap_or(Dir3::NEG_Z);
            let far = CRACK_FROM + CRACK_EVERY * (CRACKS - 1) as f32;
            let reach = spatial
                .cast_ray(s.origin, dir, far, true, &filter)
                .map_or(far, |h| h.distance);
            let mut d = CRACK_FROM;
            while d <= reach {
                noise.write(Noise {
                    at: s.origin + *dir * d,
                    radius: crack,
                    source: s.shooter,
                });
                d += CRACK_EVERY;
            }
        }
        noise.write(Noise {
            at: s.origin,
            radius: report::shot_reach(&cartridge, barrel, load, cut) * k,
            source: s.shooter,
        });
    }
}

/// How far `gun`'s next shot will be heard (m), as [`shot_noise`] reckons
/// it: for a gauge, and for anything that wants to say so.
pub fn reach_of(gun: &Gun, id: &str, suppressor: Option<(Suppressor, f32)>, report_k: f32) -> f32 {
    let cartridge = &gun.firearm.def.cartridge;
    let barrel = report::barrel_mm(id).unwrap_or_else(|| report::full_burn_mm(cartridge));
    let cut = suppressor.map_or(0.0, |(s, c)| s.cut_at(c));
    report::shot_reach(cartridge, barrel, gun.firearm.last_fired, cut) * report_k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Side;
    use crate::weapons::feel::vr_tests::{Range, gun_mut, hold};
    use extraction_core::ammo::{Load, Loads};
    use extraction_core::firearm::{FireMode, Magazine};

    /// A magazine of `n` rounds of `load` in the gun, one chambered.
    fn load_with(r: &mut Range, family: &str, n: u32, load: Load) {
        let mut g = gun_mut(r);
        let f = &mut g.firearm;
        let mut loads = Loads::default();
        loads.fill(0, load, n);
        if f.def.magazine_family.is_some() {
            f.insert_magazine(Magazine {
                family: family.into(),
                capacity: n,
                rounds: n,
                loads,
            })
            .unwrap();
        } else {
            // A fixed magazine: rounds straight in.
            f.internal_rounds = n.min(f.def.internal_capacity);
            f.internal_loads = loads;
        }
        f.set_bolt_travel(1.0);
        f.release_bolt();
        while f.fire_mode() != FireMode::Semi {
            f.cycle_mode();
        }
        f.condition = 1.0;
        f.fouling = 0.0;
    }

    /// `can` (an item id) on `r`'s muzzle.
    fn fit(r: &mut Range, can: &str) -> Entity {
        let at =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .translation();
        let e = r.vr.spawn(can, Transform::from_translation(at));
        let world = r.vr.app.world_mut();
        let se = world
            .query::<(Entity, &Socket, &SocketOwner)>()
            .iter(world)
            .find(|(_, s, o)| o.0 == r.gun && s.kind == SocketKind::Mount(Mount::Muzzle))
            .map(|(e, _, _)| e)
            .expect("a muzzle");
        let mut q = bevy::ecs::system::SystemState::<(Commands, Query<&mut Socket>)>::new(world);
        let (mut c, mut sockets) = q.get_mut(world).unwrap();
        let mut s = sockets.get_mut(se).unwrap();
        crate::interact::seat_in_socket(&mut c, e, se, &mut s);
        q.apply(world);
        r.vr.step(3);
        e
    }

    /// The gun turned up `rad` in the hand, clear of the yard's berms.
    fn aim_up(r: &mut Range, rad: f32) {
        let h = r.vr.hand_state(Side::Right).world;
        let right = h.rotation * Vec3::X;
        r.vr.hand(
            Side::Right,
            h.translation,
            Quat::from_axis_angle(right, rad) * h.rotation,
            1.0,
            0.0,
        );
        r.vr.step(20);
    }

    /// One pull of the trigger: the report's reach (m) and how many cracks
    /// went down the line.
    fn one_shot(r: &mut Range) -> (f32, usize) {
        let muzzle =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .translation();
        let mut cursor =
            r.vr.app
                .world()
                .resource::<Messages<Noise>>()
                .get_cursor_current();
        let mut heard = Vec::new();
        r.vr.trigger(Side::Right, 1.0);
        for _ in 0..8 {
            r.vr.step(1);
            heard.extend(
                cursor
                    .read(r.vr.app.world().resource::<Messages<Noise>>())
                    .filter(|n| n.source.is_some())
                    .cloned(),
            );
        }
        r.vr.trigger(Side::Right, 0.0);
        r.vr.step(10);
        let report = heard
            .iter()
            .filter(|n| n.at.distance(muzzle) < 2.0)
            .map(|n| n.radius)
            .fold(0.0, f32::max);
        let cracks = heard
            .iter()
            .filter(|n| n.at.distance(muzzle) >= 10.0)
            .count();
        (report, cracks)
    }

    /// An AK fired from the hand: ball, bare, is heard 220 m off and
    /// cracks down the range; under the PBS it's quieter than a bare
    /// pistol but still cracks; subsonic under the PBS, it's quieter
    /// still and doesn't crack at all.
    #[test]
    fn a_suppressor_and_subsonic_rounds_quiet_the_shot() {
        let shot = |load: Load, can: Option<&str>| {
            let mut r = hold("ak74", false, false);
            if let Some(c) = can {
                fit(&mut r, c);
            }
            load_with(&mut r, "ak545", 5, load);
            aim_up(&mut r, 0.17);
            one_shot(&mut r)
        };
        let (bare, bare_cracks) = shot(Load::Standard, None);
        let (pbs, pbs_cracks) = shot(Load::Standard, Some("pbs_suppressor"));
        let (quiet, quiet_cracks) = shot(Load::Subsonic, Some("pbs_suppressor"));
        println!(
            "AK report: bare {bare:.0} m ({bare_cracks} cracks), PBS {pbs:.0} m ({pbs_cracks}), subsonic PBS {quiet:.0} m ({quiet_cracks})"
        );
        assert!((bare - report::REF_REACH).abs() < 5.0, "{bare}");
        assert!(bare_cracks >= 3, "{bare_cracks}");
        let pistol = report::shot_reach("9x18", 93.0, Load::Standard, 0.0);
        assert!(pbs < pistol, "{pbs} vs a pistol's {pistol}");
        assert!(pbs_cracks >= 3, "a suppressor doesn't stop the crack");
        assert!(quiet < pbs, "{quiet}");
        assert_eq!(quiet_cracks, 0, "a subsonic round doesn't crack");
    }

    /// A can wears out: thirty rounds through it and the shot is most of
    /// the way back to bare, and the gun's fouled faster than bare.
    #[test]
    fn a_can_suppressor_wears_out_and_fouls_the_gun() {
        let mut r = hold("ak74", false, false);
        let can = fit(&mut r, "can_suppressor");
        load_with(&mut r, "ak545", 30, Load::Standard);
        r.vr.step(3);
        let (first, _) = one_shot(&mut r);
        let mut last = first;
        for _ in 0..34 {
            last = one_shot(&mut r).0;
            if gun_mut(&mut r).firearm.live_rounds() == 0 {
                break;
            }
        }
        let worn = r.vr.app.world().get::<Item>(can).unwrap().condition;
        let fouled = gun_mut(&mut r).firearm.fouling;
        let per = gun_mut(&mut r).firearm.def.fouling_per_shot;
        println!(
            "can: first {first:.0} m, last {last:.0} m, condition {worn:.2}, fouling {fouled:.3}"
        );
        assert!(first < 0.5 * report::REF_REACH, "{first}");
        assert!(last > first * 1.3, "{last} vs {first}");
        assert!(worn < 0.4, "{worn}");
        assert!(fouled > 25.0 * per * 1.5, "{fouled}");
    }

    /// An SVD fired from the hand at nothing, a drone hanging 200 m down
    /// the line and 8 m off it. Bare, it hears the report and looks back
    /// toward the shooter; under the PBS the report doesn't reach it, only
    /// the crack going by, and it looks for the shooter out by itself.
    #[test]
    fn the_crack_sends_a_robot_downrange_the_wrong_way() {
        let heard_at = |can: bool| -> (Vec3, Vec3) {
            let mut r = hold("svd", false, false);
            if can {
                fit(&mut r, "pbs_suppressor");
            }
            load_with(&mut r, "svd", 5, Load::Standard);
            aim_up(&mut r, 0.17);
            let gt = *r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            let (muzzle, fwd) = (gt.translation(), gt.forward().as_vec3());
            let side = fwd.cross(Vec3::Y).normalize();
            let at = muzzle + fwd * 200.0 + side * 8.0;
            let drone = {
                let mut commands = r.vr.app.world_mut().commands();
                let job = crate::robots::Job {
                    kind: crate::robots::JobKind::Survey,
                    site: at,
                    duration: 1.0e6,
                    exit: at,
                };
                crate::robots::drone::spawn_drone(&mut commands, at, job)
            };
            r.vr.app.world_mut().flush();
            r.vr.app
                .world_mut()
                .entity_mut(drone)
                .insert((RigidBody::Static, Transform::from_translation(at)));
            r.vr.step(5);
            one_shot(&mut r);
            let aware =
                r.vr.app
                    .world()
                    .get::<crate::robots::Awareness>(drone)
                    .unwrap();
            (aware.last_known.expect("it heard something"), muzzle)
        };
        let (bare, muzzle) = heard_at(false);
        println!(
            "bare: it looks {:.0} m from the shooter",
            bare.distance(muzzle)
        );
        assert!(bare.distance(muzzle) < 80.0, "the report gives you away");
        let (quiet, muzzle) = heard_at(true);
        println!(
            "suppressed: it looks {:.0} m from the shooter",
            quiet.distance(muzzle)
        );
        assert!(quiet.distance(muzzle) > 150.0, "the crack misleads it");
    }
}
