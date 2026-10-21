//! Getting a stone out of an anomaly is a skill, a different one each:
//!
//! - **Lift**: the stone rides the column, out of reach. Throw something
//!   in to wind it up, keep clear, and catch the stone as the pop throws
//!   it (`lifts`).
//! - **Burner**: the stone in the coals is hot, and holding it burns your
//!   hand. Knock it out (shoot it, flick it), then let it cool for twenty
//!   seconds or drop it in water ([`Hot`], here).
//! - **Snare**: its stone sits in it quietly, but a hand reaching in sets
//!   it off. Ground it first with something thrown, and reach in while it
//!   recharges (`snares`).
//! - **Well**: the stone hangs at the heart. Dash in and out (`wells`).
//! - **Mirror**: the shard hangs in the fold. Throw something through and
//!   reach in while it closes (`mirrors`).

use bevy::prelude::*;
use extraction_core::anomaly::{AnomalyKind, HOT_BURN, HOT_SECS, heat_left};
use extraction_core::items::Category;

use super::Anomaly;
use crate::health::Health;
use crate::interact::Held;
use crate::items::{Catalog, Item};
use crate::world::Zone;

/// A stone just let go of, too hot: no hand takes it again for a moment
/// (seconds left). Without it, a hand still closed round it took it
/// straight back, and burned to death holding on.
#[derive(Component, Debug, Clone, Copy)]
pub struct Scalded(pub f32);

/// How long a dropped hot stone won't be taken back, seconds.
pub const SCALD_SECS: f32 = 3.0;

/// The moment passes.
pub fn cool_scald(
    mut commands: Commands,
    mut stones: Query<(Entity, &mut Scalded)>,
    time: Res<Time>,
) {
    for (e, mut s) in &mut stones {
        s.0 -= time.delta_secs();
        if s.0 <= 0.0 {
            commands.entity(e).remove::<Scalded>();
        }
    }
}

/// How long a hand holds a hot stone before it lets go, seconds.
pub const HOT_GRIP: f32 = 1.5;
/// What's said when it does.
pub const TOO_HOT: &str = "Too hot to hold: you drop it. Let it cool, or pocket it.";

/// A stone hot from a burner's coals: seconds since it left them.
#[derive(Component, Debug, Clone, Copy)]
pub struct Hot {
    pub out: f32,
}

impl Hot {
    pub fn heat(&self) -> f32 {
        heat_left(self.out, false)
    }
}

/// A hot stone burned the hand holding it (for sound, haptics, a hiss).
#[derive(Message, Clone, Copy, Debug)]
pub struct Burned {
    pub player: Entity,
    pub at: Vec3,
}

/// Heats stones lying in burners, cools them out of the coals (at once in
/// water), and burns whoever holds one hot.
#[allow(clippy::type_complexity)]
pub fn heat(
    mut commands: Commands,
    burners: Query<(&Anomaly, &GlobalTransform)>,
    mut stones: Query<(
        Entity,
        &Item,
        &GlobalTransform,
        Option<&mut Hot>,
        Option<&Held>,
    )>,
    mut players: Query<&mut Health>,
    catalog: Res<Catalog>,
    zone: Res<Zone>,
    time: Res<Time>,
    mut burned: MessageWriter<Burned>,
    mut actions: MessageWriter<crate::interact::HandAction>,
    mut raid: Option<ResMut<crate::raid::Raid>>,
    mut gripped: Local<std::collections::HashMap<Entity, f32>>,
) {
    let dt = time.delta_secs();
    // Only stones hot and in a hand now: one let go (or gone) starts
    // afresh, rather than carrying its grip's time to the next hand.
    gripped.retain(|e, _| {
        stones
            .get(*e)
            .is_ok_and(|(.., hot, held)| hot.is_some() && held.is_some())
    });
    let coals: Vec<(Vec3, f32)> = burners
        .iter()
        .filter(|(a, _)| a.kind == AnomalyKind::Burner)
        .map(|(a, g)| (g.translation(), a.radius))
        .collect();
    for (e, item, gt, hot, held) in &mut stones {
        if catalog.get(item.kind()).category != Category::Artifact {
            continue;
        }
        let p = gt.translation();
        let in_coals = held.is_none()
            && coals.iter().any(|(c, r)| {
                let rel = p - *c;
                Vec2::new(rel.x, rel.z).length() < *r && rel.y < 2.5
            });
        match hot {
            None if in_coals => {
                commands.entity(e).insert(Hot { out: 0.0 });
            }
            None => {}
            Some(mut h) => {
                let quenched = zone.hm.water_level([p.x, p.z]).is_some_and(|w| p.y < w);
                if in_coals {
                    h.out = 0.0;
                } else {
                    h.out += dt;
                }
                if quenched || h.out >= HOT_SECS {
                    commands.entity(e).remove::<Hot>();
                    continue;
                }
                if let Some(held) = held
                    && let Ok(mut hp) = players.get_mut(held.player)
                    && !hp.down
                {
                    // Nobody holds on to something burning: a moment, and
                    // the hand lets go (someone new, told to take the
                    // stone, held it to death otherwise).
                    let t = gripped.entry(e).or_default();
                    *t += dt;
                    if *t > HOT_GRIP {
                        gripped.remove(&e);
                        actions.write(crate::interact::HandAction::Release {
                            player: held.player,
                            side: held.side,
                            throw: 0.0,
                        });
                        commands.entity(e).insert(Scalded(SCALD_SECS));
                        if let Some(raid) = raid.as_mut() {
                            raid.say(TOO_HOT);
                        }
                    }
                    hp.hp -= HOT_BURN * h.heat().max(0.3) * dt;
                    hp.since_hit = 0.0;
                    burned.write(Burned {
                        player: held.player,
                        at: p,
                    });
                }
            }
        }
    }
}

/// Where each kind holds its stone, over its ground position.
pub fn stone_height(kind: AnomalyKind) -> f32 {
    match kind {
        AnomalyKind::Lift => 1.2,
        AnomalyKind::Snare => 0.8,
        AnomalyKind::Mirror => super::mirrors::MIRROR_HEIGHT,
        AnomalyKind::Well => 0.3,
        AnomalyKind::Burner => 0.3,
        AnomalyKind::Hush => 0.3,
    }
}

/// An anomaly with a stone in it, and loose kit to work it with
/// (scenarios, tests).
pub fn stage(
    world: &mut World,
    kind: AnomalyKind,
    at: Vec3,
    radius: f32,
    artifact: &str,
    kit: &[(String, Vec3)],
) -> Entity {
    let a = super::spawn_anomaly(world, kind, at, radius, None);
    super::surge::grow(world, artifact, at + Vec3::Y * stone_height(kind));
    for (id, p) in kit {
        super::surge::grow(world, id, *p);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::LaunchOptions;
    use crate::rig::Side;
    use crate::vr_harness::Vr;
    use avian3d::prelude::*;

    fn app() -> App {
        let mut app = crate::headless_app(
            LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    fn find(world: &mut World, kind: AnomalyKind) -> (Entity, Vec3, f32) {
        world
            .query::<(Entity, &Anomaly, &GlobalTransform)>()
            .iter(world)
            .find(|(_, a, _)| a.kind == kind && a.bloom.is_none())
            .map(|(e, a, g)| (e, g.translation(), a.radius))
            .unwrap()
    }

    fn spawn(world: &mut World, id: &str, at: Vec3) -> Entity {
        let catalog = world.resource::<Catalog>().clone();
        let kind = catalog.kind(id).unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let e = {
            let mut c = Commands::new(&mut queue, world);
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(at),
                    condition: 1.0,
                },
            )
        };
        queue.apply(world);
        e
    }

    fn pos(app: &App, e: Entity) -> Vec3 {
        app.world().get::<GlobalTransform>(e).unwrap().translation()
    }

    #[test]
    fn a_lifts_stone_rides_out_of_reach_until_it_pops_and_throws_it() {
        let mut app = app();
        let (lift, at, r) = find(app.world_mut(), AnomalyKind::Lift);
        let stone = spawn(app.world_mut(), "art_featherstone", at + Vec3::Y * 1.0);
        for _ in 0..90 * 4 {
            app.update();
        }
        let p = pos(&app, stone);
        let ground = app.world().resource::<Zone>().ground(p.x, p.z);
        assert!(
            p.y - ground > 2.4,
            "out of reach up the column ({:.2} m up)",
            p.y - ground
        );
        assert!(Vec2::new(p.x - at.x, p.z - at.z).length() < r);
        assert!(
            app.world().get::<Anomaly>(lift).unwrap().charge < 0.2,
            "and the stone alone doesn't wind it up"
        );
        // A bolt in: it winds up and pops, and throws the stone out.
        spawn(app.world_mut(), "bolt", at + Vec3::Y * 1.5);
        let mut out = None;
        for _ in 0..90 * 12 {
            app.update();
            let p = pos(&app, stone);
            if Vec2::new(p.x - at.x, p.z - at.z).length() > r + 0.5 {
                out = Some(p);
            }
        }
        let p = out.expect("the pop threw it out of the column");
        let ground = app.world().resource::<Zone>().ground(p.x, p.z);
        assert!(
            p.y - ground < 2.2,
            "where a hand can reach it ({:.2} m up)",
            p.y - ground
        );
    }

    #[test]
    fn a_stone_from_the_coals_burns_until_it_cools() {
        let mut app = app();
        let (_, at, _) = find(app.world_mut(), AnomalyKind::Burner);
        let stone = spawn(app.world_mut(), "art_hearth", at + Vec3::Y * 0.3);
        for _ in 0..30 {
            app.update();
        }
        assert!(app.world().get::<Hot>(stone).is_some(), "hot in the coals");
        // Knocked out onto open ground beside it.
        let out = {
            let zone = app.world().resource::<Zone>();
            zone.on_ground(at.x + 6.0, at.z) + Vec3::Y * 0.3
        };
        app.world_mut()
            .get_mut::<Transform>(stone)
            .unwrap()
            .translation = out;
        for _ in 0..90 * 5 {
            app.update();
        }
        let h = app
            .world()
            .get::<Hot>(stone)
            .expect("still hot five seconds out");
        assert!(h.heat() < 0.9);
        for _ in 0..90 * 17 {
            app.update();
        }
        assert!(app.world().get::<Hot>(stone).is_none(), "cool after twenty");
    }

    /// Held on to, a hot stone is let go of in a moment, with a word why:
    /// it burns, but nobody holds it till it kills them.
    #[test]
    fn a_hot_stone_is_let_go_of() {
        let mut vr = Vr::new("marrow_creek");
        let (_, at, _) = find(vr.app.world_mut(), AnomalyKind::Burner);
        let feet = vr
            .app
            .world()
            .resource::<Zone>()
            .on_ground(at.x + 8.0, at.z);
        vr.stand(feet, 0.0);
        vr.step(10);
        let hand = feet + Vec3::new(0.25, 1.1, -0.35);
        let stone = spawn(vr.app.world_mut(), "art_hearth", hand);
        vr.app
            .world_mut()
            .entity_mut(stone)
            .insert((RigidBody::Static, Hot { out: 0.0 }));
        vr.hand(Side::Right, hand, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(4);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(stone), "took it");
        // Still squeezing.
        vr.step(270);
        assert_ne!(vr.hand_state(Side::Right).holding, Some(stone), "let go");
        let said = &vr.app.world().resource::<crate::raid::Raid>().log;
        assert!(said.iter().any(|(l, _)| l == TOO_HOT), "and why: {said:?}");
        vr.step(630);
        let hp = vr.app.world().get::<Health>(vr.player).unwrap().hp;
        assert!(hp > 70.0, "a burn, not the end of you: {hp:.0}");
    }

    #[test]
    fn a_hot_stone_burns_the_hand_and_water_quenches_it() {
        let mut vr = Vr::new("marrow_creek");
        let (_, at, _) = find(vr.app.world_mut(), AnomalyKind::Burner);
        let feet = vr
            .app
            .world()
            .resource::<Zone>()
            .on_ground(at.x + 8.0, at.z);
        vr.stand(feet, 0.0);
        vr.step(10);
        let hand = feet + Vec3::new(0.25, 1.1, -0.35);
        let stone = spawn(vr.app.world_mut(), "art_hearth", hand);
        vr.app
            .world_mut()
            .entity_mut(stone)
            .insert((RigidBody::Static, Hot { out: 0.0 }));
        vr.hand(Side::Right, hand, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(4);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(stone), "took it");
        let hp = vr.app.world().get::<Health>(vr.player).unwrap().hp;
        vr.step(90);
        let after = vr.app.world().get::<Health>(vr.player).unwrap().hp;
        assert!(hp - after > 5.0, "it burns ({hp:.0} -> {after:.0})");
        // In the creek it hisses out at once.
        let water = {
            let zone = vr.app.world().resource::<Zone>();
            (0..4000)
                .map(|i| {
                    [
                        -250.0 + (i % 64) as f32 * 8.0,
                        -250.0 + (i / 64) as f32 * 8.0,
                    ]
                })
                .find_map(|p| {
                    zone.hm
                        .water_level(p)
                        .map(|w| Vec3::new(p[0], w - 0.2, p[1]))
                })
                .expect("water somewhere")
        };
        vr.grip(Side::Right, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(stone)
            .insert(RigidBody::Static)
            .get_mut::<Transform>()
            .unwrap()
            .translation = water;
        vr.step(5);
        assert!(vr.app.world().get::<Hot>(stone).is_none(), "quenched");
    }

    /// A hand reaching into a snare for its stone sets it off; thrown
    /// metal first grounds it, and the hand gets in while it recharges.
    #[test]
    fn a_hand_reaching_into_a_snare_sets_it_off_unless_it_was_grounded() {
        let mut vr = Vr::new("marrow_creek");
        let (snare, at, r) = find(vr.app.world_mut(), AnomalyKind::Snare);
        let stone = spawn(vr.app.world_mut(), "art_aegis", at + Vec3::Y * 0.8);
        // Stood just outside it, facing in (+X).
        let feet = vr
            .app
            .world()
            .resource::<Zone>()
            .on_ground(at.x - r - 0.7, at.z);
        vr.stand(feet, -std::f32::consts::FRAC_PI_2);
        vr.step(90 * 5);
        assert_eq!(
            vr.app.world().get::<Anomaly>(snare).unwrap().charge,
            1.0,
            "its own stone doesn't set it off"
        );
        let reach_in = |vr: &mut Vr| {
            let hp = vr.app.world().get::<Health>(vr.player).unwrap().hp;
            vr.hand(
                Side::Right,
                at + Vec3::new(-r + 0.4, 0.9, 0.0),
                Quat::IDENTITY,
                0.0,
                0.0,
            );
            vr.step(5);
            let after = vr.app.world().get::<Health>(vr.player).unwrap().hp;
            vr.hand(
                Side::Right,
                feet + Vec3::new(0.0, 1.1, 0.3),
                Quat::IDENTITY,
                0.0,
                0.0,
            );
            vr.step(2);
            hp - after
        };
        assert!(reach_in(&mut vr) > 15.0, "a bare reach in is struck");
        vr.step(90 * 5);
        // Ground it: something metal tossed in, and straight in after it.
        spawn(vr.app.world_mut(), "bolt", at + Vec3::Y * 1.5);
        let mut fired = false;
        for _ in 0..60 {
            vr.step(1);
            if vr.app.world().get::<Anomaly>(snare).unwrap().charge < 0.5 {
                fired = true;
                break;
            }
        }
        assert!(fired, "the bolt drew its charge");
        assert!(reach_in(&mut vr) < 1.0, "and the hand gets in unhurt");
        let _ = stone;
    }
}
