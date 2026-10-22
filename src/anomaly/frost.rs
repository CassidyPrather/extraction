//! Frosts: still cold that steals heat, Rime Fell's own. Step into one and
//! the cold gets into you fast, faster towards its heart: it's the body's
//! one chill, the valley's (`world::cold::Chill`), so a frost and the
//! weather add up, you shiver for minutes after (a roof warms it off
//! quickest), and the stamina it caps is the valley's to give back. In the
//! frost the chill stiffens you too (slower, and past a point too stiff to
//! run), and frozen through it bites. You're told as it starts to take
//! you, well before the bite; out of it the stiffness leaves you in a
//! couple of seconds.
//!
//! The way in is a live hearth (a burner's stone): carried, a frost chills
//! you a quarter as fast and never past a light stiffness, and never
//! bites. A Frost grows a **rimestone**: carried live, a burner's coals
//! don't take you, and a little chill never leaves you.

use bevy::prelude::*;
use extraction_core::anomaly::{
    AnomalyKind, FROST_BITE, FROST_SLOW, FROST_THAW, FROST_WARN, RIMESTONE_CHILL, chill_step,
    frost_rate, frost_stiffness,
};

use super::Anomaly;
use super::artifacts::ArtifactEffects;
use crate::health::Health;
use crate::player::{Body, Player};
use crate::world::cold::Chill;

/// Pace lost past which you're too stiff to run: the warning's.
pub const STIFF: f32 = FROST_SLOW * FROST_WARN;

/// Said as a frost starts to take you, and as it bites.
pub const FROST_WARNS: &str = "The cold is in your joints. Get out of it.";
pub const FROST_BITES: &str = "The frost is biting.";

/// A frost chills whoever's in it, stiffens them, and frozen through
/// bites; out of one the stiffness eases off.
#[allow(clippy::type_complexity)]
pub fn chill(
    anomalies: Query<(&Anomaly, &GlobalTransform), Without<super::surge::Forming>>,
    mut players: Query<
        (
            Entity,
            &GlobalTransform,
            &mut Body,
            &mut Health,
            &mut Chill,
            Option<&ArtifactEffects>,
        ),
        With<Player>,
    >,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    mut raid: ResMut<crate::raid::Raid>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, g, mut body, mut h, mut c, fx) in &mut players {
        let at = g.translation();
        let rate = anomalies
            .iter()
            .filter(|(a, _)| a.kind == AnomalyKind::Frost)
            .filter(|(_, f)| (at.y - f.translation().y) < 2.5)
            .map(|(a, f)| frost_rate(at.xz().distance(f.translation().xz()), a.radius))
            .fold(0.0, f32::max);
        let hearth = fx.is_some_and(|fx| fx.hearth);
        let rime = fx.is_some_and(|fx| fx.rimestone);
        let before = c.level;
        c.level = chill_step(before, rate, hearth, rime, dt);
        if rate > 0.0 {
            body.chilled = frost_stiffness(c.level, hearth);
            if locals.contains(e) {
                if before < FROST_WARN && c.level >= FROST_WARN {
                    raid.say(FROST_WARNS);
                }
                if before < 1.0 && c.level >= 1.0 {
                    raid.say(FROST_BITES);
                }
            }
            if c.level >= 1.0 && !hearth && !h.down {
                h.hp -= FROST_BITE * dt;
                h.since_hit = 0.0;
            }
        } else {
            let rest = if rime {
                frost_stiffness(RIMESTONE_CHILL, false)
            } else {
                0.0
            };
            body.chilled = (body.chilled - FROST_THAW * dt).max(rest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::body::{Backpack, InBackpack};
    use avian3d::prelude::{ColliderDisabled, RigidBodyDisabled};

    /// A player standing at a frost's heart (one put down for the test).
    fn app() -> (App, Entity) {
        let mut app = crate::headless_app(Default::default(), false);
        app.finish();
        app.cleanup();
        app.update();
        let p = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
        let me = app.world().get::<GlobalTransform>(p).unwrap().translation();
        let site = app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(me.x, me.z);
        super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Frost, site, 3.0, None);
        (app, p)
    }

    fn carry(app: &mut App, p: Entity, id: &str) -> Entity {
        let pack = app
            .world_mut()
            .query::<(Entity, &Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == p && b.carried)
            .map(|(e, _)| e)
            .unwrap();
        let e = crate::anomaly::surge::grow(app.world_mut(), id, Vec3::ZERO).unwrap();
        app.world_mut()
            .get_mut::<Backpack>(pack)
            .unwrap()
            .contents
            .push(e);
        app.world_mut().entity_mut(e).insert((
            InBackpack { bag: pack },
            ChildOf(pack),
            RigidBodyDisabled,
            ColliderDisabled,
        ));
        e
    }

    fn said(app: &App, line: &str) -> bool {
        app.world()
            .resource::<crate::raid::Raid>()
            .log
            .iter()
            .any(|(t, _)| t == line)
    }

    fn hp(app: &App, p: Entity) -> f32 {
        app.world().get::<Health>(p).unwrap().hp
    }

    #[test]
    fn a_frost_warns_stiffens_then_bites() {
        let (mut app, p) = app();
        let full = hp(&app, p);
        // A moment in: stiffening, not yet told, not yet bitten.
        for _ in 0..20 {
            app.update();
        }
        let stiff = app.world().get::<Body>(p).unwrap().chilled;
        assert!(stiff > 0.05, "stiffening ({stiff:.2})");
        let cold = app.world().get::<Chill>(p).unwrap().level;
        assert!(cold > 0.1, "the cold is in you ({cold:.2})");
        assert!(!said(&app, FROST_WARNS));
        assert_eq!(hp(&app, p), full, "no bite yet");
        // Told before the bite.
        for _ in 0..40 {
            app.update();
        }
        assert!(said(&app, FROST_WARNS), "told as it takes you");
        assert_eq!(hp(&app, p), full, "and still no bite");
        for _ in 0..90 * 3 {
            app.update();
        }
        assert!(said(&app, FROST_BITES));
        assert!(hp(&app, p) < full - 5.0, "bitten ({:.0})", hp(&app, p));
        assert!(app.world().get::<Body>(p).unwrap().chilled > 0.65);
        // Out of it: loose again in seconds, but still frozen through,
        // for the valley to warm off.
        let me = app.world().get::<GlobalTransform>(p).unwrap().translation();
        let out = app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(me.x + 6.0, me.z);
        app.world_mut().get_mut::<Transform>(p).unwrap().translation = out;
        for _ in 0..90 * 4 {
            app.update();
        }
        assert_eq!(app.world().get::<Body>(p).unwrap().chilled, 0.0);
        assert!(app.world().get::<Chill>(p).unwrap().level > 0.9);
        let after = hp(&app, p);
        for _ in 0..90 {
            app.update();
        }
        assert!(hp(&app, p) >= after, "and no longer bitten");
    }

    #[test]
    fn a_hearth_keeps_the_frost_off() {
        let (mut app, p) = app();
        carry(&mut app, p, "art_hearth");
        let full = hp(&app, p);
        for _ in 0..90 * 20 {
            app.update();
        }
        assert_eq!(hp(&app, p), full, "never bitten");
        let stiff = app.world().get::<Body>(p).unwrap().chilled;
        assert!(stiff < STIFF, "never too stiff to run ({stiff:.2})");
        assert!(!said(&app, FROST_WARNS));
    }

    #[test]
    fn a_rimestone_walks_you_through_coals_but_keeps_you_stiff() {
        let mut app = crate::headless_app(Default::default(), false);
        app.finish();
        app.cleanup();
        app.update();
        let p = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
        let me = app.world().get::<GlobalTransform>(p).unwrap().translation();
        let site = app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(me.x, me.z);
        carry(&mut app, p, "art_rimestone");
        for _ in 0..90 * 2 {
            app.update();
        }
        let stiff = app.world().get::<Body>(p).unwrap().chilled;
        assert!(stiff > 0.05 && stiff < STIFF, "a little stiff ({stiff:.2})");
        super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Burner, site, 2.0, None);
        let full = hp(&app, p);
        for _ in 0..90 * 3 {
            app.update();
        }
        assert_eq!(hp(&app, p), full, "the coals don't take");
    }
}
