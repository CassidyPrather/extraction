//! Hushes: dead air. Whatever sound is made in one never leaves it: a shot
//! fired inside, a step, a stone's hum (robots' hearing and the director
//! read [`Hushes`]). It's a place to hide, and a place you hear nothing
//! from: detectors inside it are mute, and the mix goes dull (Audio). Stay
//! past a few seconds and the silence presses on you.
//!
//! A Hush grows a **hushstone**: carried live, it's a small Hush of your
//! own, so your steps and shots are eaten too. And so are the clicks of
//! any detector you carry, and the hum of what you carry is quieter to the
//! Zone only while you stand in a real one.

use bevy::prelude::*;
use extraction_core::anomaly::{AnomalyKind, HUSH_GRACE, HUSHSTONE_RADIUS, hush_press, hushed};

use super::Anomaly;
use super::artifacts::{CarriedBy, Shielded};
use crate::health::Health;
use crate::items::{Catalog, Item};
use crate::player::Player;

/// Where sound dies this tick: (centre, radius). Robots' hearing and the
/// director's ear skip anything made inside one.
#[derive(Resource, Debug, Default, Clone)]
pub struct Hushes {
    pub zones: Vec<([f32; 3], f32)>,
    /// Only the anomalies (not carriers' small ones): where the Zone can't
    /// hear your stones.
    pub standing: Vec<([f32; 3], f32)>,
}

impl Hushes {
    /// A sound made at `at` is eaten.
    pub fn eats(&self, at: Vec3) -> bool {
        hushed(at.to_array(), &self.zones)
    }

    /// `at` stands in a Hush anomaly (not just a carried stone's).
    pub fn in_standing(&self, at: Vec3) -> bool {
        hushed(at.to_array(), &self.standing)
    }

    /// A sound made at `src` is held in a Hush anomaly that `ear` isn't
    /// in: it never reaches them (Audio deadens it, as it deadens the
    /// world to someone inside). A machine walking in one is silent to
    /// you outside it; its dust stirring is how you know ([`stir`]).
    pub fn muffles(&self, src: Vec3, ear: Vec3) -> bool {
        self.standing.iter().any(|(c, r)| {
            let c = Vec3::from_array(*c);
            src.distance(c) < *r && ear.distance(c) >= *r
        })
    }
}

/// How fast things moving in a Hush stir its hanging dust: the sum of
/// their speeds, m/s, that stirs it full...
pub const STIR_FULL: f32 = 2.5;
/// ...how fast it rises to that, and settles after, a second.
pub const STIR_RISE: f32 = 4.0;
pub const STIR_SETTLE: f32 = 0.6;

/// Whatever moves in a Hush stirs its dust (a walker, a drone, you, a
/// thrown thing): the silence's tell, since what moves in it makes no
/// sound. Kept as the anomaly's `charge` (a Hush has none of its own),
/// which its dust reads (`visuals`, `anomaly_particles.wgsl` mode 7).
pub fn stir(
    mut hushes: Query<(&mut Anomaly, &GlobalTransform)>,
    movers: Query<(&GlobalTransform, &avian3d::prelude::LinearVelocity)>,
    walkers: Query<(&GlobalTransform, &crate::player::Body)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut a, g) in &mut hushes {
        if a.kind != AnomalyKind::Hush {
            continue;
        }
        let c = g.translation() + Vec3::Y * 1.0;
        let inside = |p: Vec3| p.distance(c) < a.radius;
        let speed: f32 = movers
            .iter()
            .filter(|(t, _)| inside(t.translation()))
            .map(|(_, v)| v.0.length())
            .chain(
                walkers
                    .iter()
                    .filter(|(t, _)| inside(t.translation() + Vec3::Y * 0.9))
                    .map(|(_, b)| Vec2::new(b.velocity.x, b.velocity.z).length()),
            )
            .sum();
        let want = (speed / STIR_FULL).min(1.0);
        let rate = if want > a.charge {
            STIR_RISE
        } else {
            STIR_SETTLE
        };
        a.charge += (want - a.charge) * (rate * dt).min(1.0);
    }
}

/// Seconds a player has spent in a Hush, this stay.
#[derive(Component, Debug, Default)]
pub struct InHush(pub f32);

/// Hush anomalies (their core, a sphere over the ground) and every live
/// hushstone's carrier.
pub fn gather(
    mut hushes: ResMut<Hushes>,
    anomalies: Query<(&Anomaly, &GlobalTransform)>,
    stones: Query<(&Item, &CarriedBy), Without<Shielded>>,
    players: Query<&GlobalTransform, With<Player>>,
    catalog: Res<Catalog>,
) {
    hushes.standing.clear();
    for (a, g) in &anomalies {
        if a.kind == AnomalyKind::Hush {
            let c = g.translation() + Vec3::Y * 1.0;
            hushes.standing.push((c.to_array(), a.radius));
        }
    }
    hushes.zones = hushes.standing.clone();
    for (item, by) in &stones {
        if catalog.get(item.kind()).id != "art_hushstone" {
            continue;
        }
        if let Ok(g) = players.get(by.0) {
            let c = g.translation() + Vec3::Y * 1.0;
            hushes.zones.push((c.to_array(), HUSHSTONE_RADIUS));
        }
    }
}

/// Said a few seconds before the silence starts to hurt, and as it does:
/// you can't hear it coming, so you're told.
pub const HUSH_WARN: &str = "Your ears ring in the silence. Get out of it.";
pub const HUSH_HURTS: &str = "The silence is pressing on you.";

/// A sound louder than this (heard this far, m) feeds it: a thing landing,
/// a shot; not your breath.
pub const HUSH_FED_LOUD: f32 = 8.0;
/// Each sound a hush eats within this many metres of you...
pub const HUSH_FED_NEAR: f32 = 6.0;
/// ...pushes its silence back this many seconds: make a noise in it (throw
/// something in, fire a shot) and you have longer to reach its stone.
pub const HUSH_FED: f32 = 2.0;

/// The silence presses on whoever stays in a Hush past its grace; you're
/// warned first.
pub fn press(
    mut commands: Commands,
    hushes: Res<Hushes>,
    mut noises: MessageReader<crate::robots::Noise>,
    mut players: Query<(Entity, &GlobalTransform, &mut Health, Option<&mut InHush>), With<Player>>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    mut raid: ResMut<crate::raid::Raid>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    // Sounds it ate this tick, where (a thing thrown in, a shot, a radio):
    // fed, its silence eases off whoever's near.
    let eaten: Vec<Vec3> = noises
        .read()
        .filter(|n| n.radius > HUSH_FED_LOUD && hushes.in_standing(n.at))
        .map(|n| n.at)
        .collect();
    for (e, g, mut h, inside) in &mut players {
        let chest = g.translation() + Vec3::Y * 1.0;
        let here = hushes.in_standing(chest);
        match (here, inside) {
            (true, None) => {
                commands.entity(e).insert(InHush(0.0));
            }
            (true, Some(mut t)) => {
                let fed = eaten
                    .iter()
                    .filter(|n| n.distance(chest) < HUSH_FED_NEAR)
                    .count();
                t.0 = (t.0 - HUSH_FED * fed as f32).max(0.0);
                let before = t.0;
                t.0 += dt;
                if locals.contains(e) {
                    if before < HUSH_GRACE - 3.0 && t.0 >= HUSH_GRACE - 3.0 {
                        raid.say(HUSH_WARN);
                    }
                    if before < HUSH_GRACE && t.0 >= HUSH_GRACE {
                        raid.say(HUSH_HURTS);
                    }
                }
                let hurt = hush_press(t.0);
                if hurt > 0.0 && !h.down {
                    h.hp -= hurt * dt;
                    h.since_hit = 0.0;
                }
            }
            (false, Some(_)) => {
                commands.entity(e).remove::<InHush>();
            }
            (false, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::body::{Backpack, InBackpack};
    use crate::launch::LaunchOptions;
    use crate::robots::Noise;
    use crate::robots::director::Director;
    use avian3d::prelude::{ColliderDisabled, RigidBodyDisabled};

    fn app() -> (App, Entity, Vec3, f32) {
        let mut app = crate::headless_app(
            LaunchOptions {
                robots: true,
                ..Default::default()
            },
            false,
        );
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
        let (at, r) = app
            .world_mut()
            .query::<(&Anomaly, &GlobalTransform)>()
            .iter(app.world())
            .find(|(a, _)| a.kind == AnomalyKind::Hush)
            .map(|(a, g)| (g.translation(), a.radius))
            .expect("the Hollow's hush");
        // A walker within earshot, for a shot to be reported by (only a
        // machine that hears reports one: `robots::director::heard_by_any`).
        let ear = at + Vec3::new(r + 30.0, 0.0, 0.0);
        app.world_mut().spawn((
            crate::robots::Robot {
                kind: crate::robots::RobotKind::Walker,
                wrecked: false,
            },
            crate::robots::Senses {
                sight: 0.0,
                fov_deg: 90.0,
                hearing: 1.2,
                eye: Vec3::ZERO,
            },
            Transform::from_translation(ear),
            GlobalTransform::from_translation(ear),
        ));
        (app, p, at, r)
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

    fn stand(app: &mut App, p: Entity, at: Vec3) {
        let ground = app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(at.x, at.z);
        app.world_mut().get_mut::<Transform>(p).unwrap().translation = ground;
        for _ in 0..3 {
            app.update();
        }
    }

    /// A loud shot by the player at `at`: does the director send anyone?
    fn reported(app: &mut App, p: Entity, at: Vec3) -> bool {
        let mut d = app.world_mut().resource_mut::<Director>();
        d.responses.clear();
        d.response_cooldown = 0.0;
        app.world_mut().write_message(Noise {
            at,
            radius: 400.0,
            source: Some(p),
        });
        app.update();
        // This shot's response, not another source's (a survey place, a
        // relay's report) queued in the same tick.
        app.world()
            .resource::<Director>()
            .responses
            .iter()
            .any(|(_, r)| r.distance(at) < 3.0)
    }

    #[test]
    fn a_shot_fired_in_a_hush_is_never_heard() {
        let (mut app, p, at, r) = app();
        let inside = at + Vec3::Y * 1.2;
        let outside = at + Vec3::new(r + 6.0, 1.2, 0.0);
        assert!(
            reported(&mut app, p, outside),
            "outside, a shot gets a team sent"
        );
        assert!(!reported(&mut app, p, inside), "inside, nobody heard it");
    }

    /// A loud sound it eats near you (a thing thrown in, a shot) pushes its
    /// silence back; your breath doesn't.
    #[test]
    fn a_sound_it_eats_pushes_its_silence_back() {
        let (mut app, p, at, _) = app();
        stand(&mut app, p, at);
        for _ in 0..90 * 5 {
            app.update();
        }
        let before = app.world().get::<InHush>(p).unwrap().0;
        let chest = at + Vec3::Y * 1.0;
        // A breath: nothing.
        app.world_mut().write_message(Noise {
            at: chest,
            radius: 2.0,
            source: Some(p),
        });
        app.update();
        let breathed = app.world().get::<InHush>(p).unwrap().0;
        assert!(breathed > before, "a breath doesn't feed it");
        // A thing landing by you: two seconds back.
        app.world_mut().write_message(Noise {
            at: chest + Vec3::X,
            radius: 20.0,
            source: None,
        });
        app.update();
        let fed = app.world().get::<InHush>(p).unwrap().0;
        assert!(
            fed < breathed - HUSH_FED + 0.1,
            "a loud one does ({breathed:.2} -> {fed:.2})"
        );
    }

    #[test]
    fn the_silence_presses_on_whoever_stays() {
        let (mut app, p, at, _) = app();
        stand(&mut app, p, at);
        let hp = app.world().get::<Health>(p).unwrap().hp;
        for _ in 0..90 * 6 {
            app.update();
        }
        let grace = app.world().get::<Health>(p).unwrap().hp;
        assert!(
            (hp - grace).abs() < 0.5,
            "a few seconds is safe ({hp} -> {grace})"
        );
        let said = |app: &App, line: &str| {
            app.world()
                .resource::<crate::raid::Raid>()
                .log
                .iter()
                .any(|(t, _)| t == line)
        };
        assert!(said(&app, HUSH_WARN), "warned before it hurts");
        for _ in 0..90 * 6 {
            app.update();
        }
        let pressed = app.world().get::<Health>(p).unwrap().hp;
        assert!(
            grace - pressed > 6.0,
            "then it presses ({grace} -> {pressed})"
        );
        assert!(said(&app, HUSH_HURTS), "and told when it does");
    }

    #[test]
    fn in_a_hush_your_stones_dont_hum_and_your_detector_is_mute() {
        let (mut app, p, at, r) = app();
        carry(&mut app, p, "art_hearth");
        let det = carry(&mut app, p, "detector");
        let hum = |app: &App| {
            app.world()
                .get::<crate::anomaly::Attention>(p)
                .unwrap()
                .carried
        };
        let clicks = |app: &mut App| {
            let mut cursor = app
                .world()
                .resource::<Messages<crate::anomaly::detector::DetectorClick>>()
                .get_cursor_current();
            let mut n = 0;
            for _ in 0..90 * 3 {
                app.update();
                n += cursor
                    .read(
                        app.world()
                            .resource::<Messages<crate::anomaly::detector::DetectorClick>>(),
                    )
                    .filter(|c| c.detector == det)
                    .count();
            }
            n
        };
        stand(&mut app, p, at + Vec3::new(r + 3.0, 0.0, 0.0));
        let out_clicks = clicks(&mut app);
        assert!(
            hum(&app) > 1.0 && out_clicks > 2,
            "outside: hums, clicks ({out_clicks})"
        );
        stand(&mut app, p, at);
        let in_clicks = clicks(&mut app);
        assert_eq!(hum(&app), 0.0, "inside, the Zone can't hear the hearth");
        assert_eq!(in_clicks, 0, "and the detector is mute");
    }

    #[test]
    fn a_hushstone_silences_its_carrier() {
        let (mut app, p, at, r) = app();
        // Well away from the hush itself.
        let away = at + Vec3::new(r + 20.0, 0.0, 0.0);
        stand(&mut app, p, away);
        let me = app.world().get::<GlobalTransform>(p).unwrap().translation() + Vec3::Y * 1.2;
        assert!(reported(&mut app, p, me), "bare, your shot is heard");
        carry(&mut app, p, "art_hushstone");
        for _ in 0..3 {
            app.update();
        }
        assert!(
            !reported(&mut app, p, me),
            "with a hushstone on you, it isn't"
        );
    }

    /// A sound made in a Hush doesn't reach an ear outside it; one inside
    /// hears it (dulled with the rest), and one outside hears the world.
    #[test]
    fn a_hush_keeps_its_sounds_from_ears_outside() {
        let h = Hushes {
            zones: vec![([0.0, 1.0, 0.0], 3.0)],
            standing: vec![([0.0, 1.0, 0.0], 3.0)],
        };
        let inside = Vec3::new(1.0, 1.0, 0.0);
        let outside = Vec3::new(10.0, 1.0, 0.0);
        assert!(h.muffles(inside, outside), "in to out: held");
        assert!(
            !h.muffles(inside, inside + Vec3::X * 0.5),
            "in to in: heard"
        );
        assert!(
            !h.muffles(outside, inside),
            "out to in: the Hush's own deadening, not this"
        );
    }

    /// Something moving in a Hush stirs its dust; still, it settles.
    #[test]
    fn what_moves_in_a_hush_stirs_its_dust() {
        let (mut app, _, at, _) = app();
        let hush = app
            .world_mut()
            .query::<(Entity, &Anomaly)>()
            .iter(app.world())
            .find(|(_, a)| a.kind == AnomalyKind::Hush)
            .map(|(e, _)| e)
            .unwrap();
        let mover = app
            .world_mut()
            .spawn((
                Transform::from_translation(at + Vec3::Y * 1.0),
                GlobalTransform::from_translation(at + Vec3::Y * 1.0),
                avian3d::prelude::LinearVelocity(Vec3::X * 2.0),
            ))
            .id();
        for _ in 0..60 {
            app.update();
        }
        let stirred = app.world().get::<Anomaly>(hush).unwrap().charge;
        assert!(stirred > 0.5, "stirred ({stirred:.2})");
        app.world_mut().entity_mut(mover).despawn();
        for _ in 0..90 * 6 {
            app.update();
        }
        let still = app.world().get::<Anomaly>(hush).unwrap().charge;
        assert!(still < 0.1, "settles ({still:.2})");
    }
}
