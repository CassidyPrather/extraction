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

/// Ice a frost lays on water in its reach: a flat floor at the water's
/// level, so a frost by a stream or across a reservoir's arm is a crossing
/// (cold to walk: it's in the frost). One disc the frost's size; under dry
/// ground it's buried and nothing knows it's there.
#[derive(Component, Debug, Clone, Copy)]
pub struct Frozen {
    pub ice: Option<Entity>,
    /// Where and how big the frost was when its ice was laid.
    pub at: Vec3,
    pub radius: f32,
}

/// The ice sheet's thickness, m, and how far its top stands over the
/// water (so a foot on it is dry).
pub const ICE_THICK: f32 = 0.15;
pub const ICE_PROUD: f32 = 0.03;
/// Water shallower than this over the ground isn't worth a sheet, m.
const ICE_MIN_DEPTH: f32 = 0.05;
/// How finely the frost's reach is sampled for water, m.
const ICE_STEP: f32 = 0.5;

/// Where the water in a frost's reach lies (centre `c`, radius `r`): its
/// mean level, if any of the disc is water.
pub fn frozen_level(hm: &extraction_core::terrain::Heightmap, c: Vec3, r: f32) -> Option<f32> {
    let n = (r / ICE_STEP).ceil() as i32;
    let (mut sum, mut count) = (0.0f32, 0u32);
    for i in -n..=n {
        for j in -n..=n {
            let d = Vec2::new(i as f32, j as f32) * ICE_STEP;
            if d.length() > r {
                continue;
            }
            let p = [c.x + d.x, c.z + d.y];
            if let Some(w) = hm.water_level(p)
                && w - hm.height(p) > ICE_MIN_DEPTH
            {
                sum += w;
                count += 1;
            }
        }
    }
    (count > 0).then(|| sum / count as f32)
}

/// Lays (and relays, when a frost creeps or grows) the ice on water in
/// each frost's reach.
#[allow(clippy::type_complexity)]
pub fn freeze_water(
    mut commands: Commands,
    frosts: Query<(Entity, &Anomaly, &GlobalTransform, Option<&Frozen>)>,
    zone: Res<crate::world::Zone>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut ice_look: Local<Option<Handle<StandardMaterial>>>,
) {
    use avian3d::prelude::*;
    for (e, a, gt, frozen) in &frosts {
        if a.kind != AnomalyKind::Frost {
            continue;
        }
        let c = gt.translation();
        if frozen.is_some_and(|f| f.at.distance(c) < 0.05 && (f.radius - a.radius).abs() < 0.05) {
            continue;
        }
        if let Some(old) = frozen.and_then(|f| f.ice) {
            commands.entity(old).try_despawn();
        }
        let ice = frozen_level(&zone.hm, c, a.radius).map(|w| {
            let top = w + ICE_PROUD;
            let mut sheet = commands.spawn((
                Name::new("Frost ice"),
                RigidBody::Static,
                Collider::cylinder(a.radius, ICE_THICK),
                Transform::from_translation(Vec3::new(0.0, top - ICE_THICK * 0.5 - c.y, 0.0)),
                Visibility::default(),
            ));
            if let (Some(meshes), Some(materials)) = (meshes.as_mut(), materials.as_mut()) {
                let look = ice_look
                    .get_or_insert_with(|| {
                        materials.add(StandardMaterial {
                            // Pale, rough-skinned ice over dark water.
                            base_color: Color::srgba(0.72, 0.82, 0.92, 0.92),
                            perceptual_roughness: 0.45,
                            reflectance: 0.5,
                            alpha_mode: AlphaMode::Blend,
                            ..default()
                        })
                    })
                    .clone();
                sheet.insert((
                    Mesh3d(meshes.add(Cylinder::new(a.radius, ICE_THICK).mesh().resolution(40))),
                    MeshMaterial3d(look),
                ));
            }
            let id = sheet.id();
            commands.entity(e).add_child(id);
            id
        });
        commands.entity(e).try_insert(Frozen {
            ice,
            at: c,
            radius: a.radius,
        });
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

    /// A frost over the canal freezes it: you stand on the ice, dry, where
    /// without it you'd be swimming.
    #[test]
    fn a_frost_freezes_water_walkable() {
        let opts = crate::LaunchOptions {
            robots: false,
            map: "cinder_works".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let p = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let (at, w) = (25..60)
            .filter_map(|z| {
                let q = [-70.0, z as f32];
                zone.hm
                    .water_level(q)
                    .map(|w| (q, w, w - zone.hm.height(q)))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(q, w, _)| (q, w))
            .expect("the canal");
        let frost = super::super::spawn_anomaly(
            app.world_mut(),
            AnomalyKind::Frost,
            Vec3::new(at[0], zone.hm.height(at), at[1]),
            3.0,
            None,
        );
        app.update();
        app.update();
        let ice = app.world().get::<Frozen>(frost).and_then(|f| f.ice);
        assert!(ice.is_some(), "ice on the canal");
        app.world_mut().get_mut::<Transform>(p).unwrap().translation =
            Vec3::new(at[0] + 1.0, w + 0.4, at[1]);
        for _ in 0..90 {
            app.update();
        }
        let feet = app.world().get::<Transform>(p).unwrap().translation;
        let body = app.world().get::<Body>(p).unwrap();
        assert!(
            !body.swimming && body.wading < 0.01,
            "on the ice, dry (wading {:.2}, swimming {})",
            body.wading,
            body.swimming
        );
        assert!(
            (feet.y - (w + ICE_PROUD)).abs() < 0.1,
            "standing on the ice: feet {:.2}, water {w:.2}",
            feet.y
        );
        // Gone with its frost.
        app.world_mut().entity_mut(frost).despawn();
        app.update();
        assert!(app.world().get_entity(ice.unwrap()).is_err(), "it melts");
    }

    /// Frosts on dry ground lay no ice.
    #[test]
    fn a_frost_on_dry_ground_lays_no_ice() {
        let (mut app, _) = app();
        app.update();
        app.update();
        let f = app
            .world_mut()
            .query::<&Frozen>()
            .iter(app.world())
            .next()
            .copied();
        assert!(f.is_some_and(|f| f.ice.is_none()), "no water, no ice");
    }
}
