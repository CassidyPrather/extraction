//! The broken walker: the first machine a newcomer can fight and live.
//!
//! A plain walker whose gun is dead (its turret part at nothing) and one
//! of whose legs is all but gone, so it drags that foot and goes slow
//! (`anim`'s limp). It notices you as a walker does, turns its eye on you
//! and limps at you; it can't shoot, and up close all it can do is shove
//! you, for a few hp ([`SHOVE_DAMAGE`]). It wrecks as a walker does and
//! leaves a walker's salvage: the first wreck a newcomer loots.
//!
//! The director sets one on a career's second job, on its way, before any
//! armed machine is sent out (`director`). Its mind is the walker's
//! (`walker::walker_brain`, the [`Broken`] branch); its gun is held dead
//! there and in `walker::walker_guns` (a mender's weld doesn't bring it
//! back). It has no systems of its own: its parts are broken as it's
//! made and its shove is a command. `docs/FIRST_HOUR.md`, "The first fight": every armed machine
//! killed every newcomer it met first; this one fights back but can't.

use bevy::prelude::*;

use super::{Job, JobKind, PartRole, RobotPart};
use crate::world::Zone;

/// It limps at you at this share of a walker's walk (`Intent::pace`;
/// its lame leg slows it again: `anim`).
pub const PACE: f32 = 0.8;
/// What's left of its bad leg (a share of a leg's hp): it drags that foot,
/// and another hit there brings it down on three.
pub const LEG_LEFT: f32 = 0.05;
/// The leg that's gone bad.
pub const BAD_LEG: usize = 1;
/// Within this of you it stops and shoves (m)...
pub const SHOVE_RANGE: f32 = 1.7;
/// ...this hard (hp)...
pub const SHOVE_DAMAGE: f32 = 4.0;
/// ...pushing you back this fast (m/s)...
pub const SHOVE_PUSH: f32 = 2.5;
/// ...and again after this long (s).
pub const SHOVE_COOLDOWN: f32 = 3.0;

/// Someone's middle is at least this far from a walker of `size` whose
/// feet don't touch them (m): its feet's reach, and a body's width. It
/// stops there to shove ([`SHOVE_RANGE`] is how far its shove reaches past
/// that).
pub fn clear_of_you(size: f32) -> f32 {
    super::anim::FOOT_RADIUS * size + 0.3
}

/// A broken walker shoved someone (for Lev's line and the first-fight
/// check).
#[derive(Message, Clone, Copy, Debug)]
pub struct Shoved {
    pub robot: Entity,
    pub player: Entity,
    pub at: Vec3,
}

pub struct BrokenPlugin;

impl Plugin for BrokenPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Shoved>();
    }
}

/// Where along the way to the job it stands: this share of the way from
/// the drop to the job's place...
pub const ALONG: f32 = 0.35;
/// ...but no nearer the drop than this, nor further than this (m)...
pub const ALONG_MIN: f32 = 30.0;
pub const ALONG_MAX: f32 = 60.0;
/// ...and this far off to the side of the line, in sight of it.
pub const ASIDE: f32 = 8.0;
/// A job with no place on the map (fetch, photograph) is walked toward the
/// nearest place of interest at least this far from the drop (m).
pub const POI_FROM: f32 = 60.0;
/// The director sends nothing else out while it stands, for at most this
/// long (s)...
pub const QUIET_MAX: f32 = 240.0;
/// ...or this long after it's wrecked.
pub const QUIET_AFTER: f32 = 45.0;

/// Where a career's second job's broken walker stands, if this raid is
/// that job's: on the way from the drop to the job's place, a third of
/// the way along and a little to the side. None on any other job.
pub fn second_job_meet(
    zone: &Zone,
    progress: &extraction_core::progression::Progress,
) -> Option<Vec3> {
    if progress.done != 1 {
        return None;
    }
    let job = progress.job.as_ref()?;
    let drop = Vec2::from(
        zone.marker(&extraction_core::map::MarkerKind::Insertion)?
            .at,
    );
    let place = job
        .mark()
        .filter(|(map, _)| *map == zone.map.name)
        .map(|(_, at)| Vec2::from(at))
        .or_else(|| {
            zone.map
                .markers_of(&extraction_core::map::MarkerKind::Poi)
                .map(|m| Vec2::from(m.at))
                .filter(|p| p.distance(drop) >= POI_FROM)
                .min_by(|a, b| a.distance(drop).total_cmp(&b.distance(drop)))
        })?;
    let way = place - drop;
    let along = (way.length() * ALONG).clamp(ALONG_MIN, ALONG_MAX.min(way.length()));
    let dir = way.normalize_or(Vec2::NEG_Y);
    let at = drop + dir * along + dir.perp() * ASIDE;
    Some(zone.on_ground(at.x, at.y))
}

/// A broken walker: its gun dead, a leg dragging.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Broken {
    /// Till it can shove again (s).
    pub cooldown: f32,
}

/// A broken walker at `at`, standing about there till it notices someone
/// (a long watch), leaving by `exit`.
pub fn spawn_broken_walker(commands: &mut Commands, zone: &Zone, at: Vec3, exit: Vec3) -> Entity {
    let e = super::walker::spawn_walker(
        commands,
        zone,
        at,
        Job {
            kind: JobKind::Watch,
            site: at,
            duration: 1800.0,
            exit,
        },
    );
    commands
        .entity(e)
        .insert((Broken::default(), Name::new("Broken walker")));
    // (Its parts are spawned by the commands before this one.)
    commands.queue(move |w: &mut World| break_parts(w, e));
    e
}

/// Its gun at nothing, its bad leg at [`LEG_LEFT`]. (Its gun stays dead
/// whatever's done to the part: `walker::walker_guns` never fires a
/// broken walker's.)
fn break_parts(w: &mut World, robot: Entity) {
    let mut parts = w.query::<&mut RobotPart>();
    for mut p in parts.iter_mut(w) {
        if p.robot != robot {
            continue;
        }
        match p.role {
            PartRole::Weapon => p.hp = 0.0,
            PartRole::Leg(i) if i as usize == BAD_LEG => p.hp = p.max_hp * LEG_LEFT,
            _ => {}
        }
    }
}

/// The shove, as its brain makes it (a command: `walker_brain`): whoever's
/// within reach of `robot` loses a few hp and is pushed back a step.
pub fn shove(w: &mut World, robot: Entity) {
    let Some(at) = w.get::<GlobalTransform>(robot).map(|g| g.translation()) else {
        return;
    };
    if w.get::<super::Robot>(robot).is_none_or(|r| r.wrecked) {
        return;
    }
    let mut players = w.query_filtered::<(
        Entity,
        &GlobalTransform,
        &mut crate::health::Health,
        &mut crate::player::Body,
    ), With<crate::player::Player>>();
    let mut hit = Vec::new();
    for (p, pt, mut health, mut body) in players.iter_mut(w) {
        let d = pt.translation() - at;
        if d.with_y(0.0).length() > SHOVE_RANGE + 0.3 || health.down {
            continue;
        }
        health.hp -= SHOVE_DAMAGE;
        health.since_hit = 0.0;
        body.velocity += d.with_y(0.0).normalize_or(Vec3::X) * SHOVE_PUSH;
        hit.push(p);
    }
    // (A hurt from where it stands: the first-fight check's warned hit.)
    for player in hit {
        w.write_message(crate::health::Hurt {
            player,
            amount: SHOVE_DAMAGE,
            from: at,
            by: Some(robot),
        });
        w.write_message(Shoved { robot, player, at });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::LaunchOptions;
    use crate::rig::LocalPlayer;
    use bevy::ecs::system::RunSystemOnce;
    use extraction_core::progression::{Job as Career, Progress};

    fn app(opts: LaunchOptions) -> App {
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    fn marrow(robots: bool) -> LaunchOptions {
        LaunchOptions {
            map: "marrow_creek".into(),
            robots,
            ..Default::default()
        }
    }

    /// A scout of the farthest place of interest, and a fetch (no place).
    fn jobs(zone: &Zone) -> [Career; 2] {
        let drop = Vec2::from(
            zone.marker(&extraction_core::map::MarkerKind::Insertion)
                .unwrap()
                .at,
        );
        let far = zone
            .map
            .markers_of(&extraction_core::map::MarkerKind::Poi)
            .max_by(|a, b| {
                Vec2::from(a.at)
                    .distance(drop)
                    .total_cmp(&Vec2::from(b.at).distance(drop))
            })
            .unwrap();
        [
            Career::Scout {
                map: zone.map.name.clone(),
                place: far.name.clone(),
                at: far.at,
                seen: false,
            },
            Career::Fetch {
                item: "bandage".into(),
                n: 1,
            },
        ]
    }

    /// It's a career's second job's, and only that one's: not the first
    /// (a safe there-and-back), not the third; and it stands on the way
    /// out from the drop, nearer it than the job's place.
    #[test]
    fn it_meets_the_second_job_only() {
        let app = app(marrow(false));
        let zone = app.world().resource::<Zone>().clone();
        let drop = Vec2::from(
            zone.marker(&extraction_core::map::MarkerKind::Insertion)
                .unwrap()
                .at,
        );
        for job in jobs(&zone) {
            for done in 0..4 {
                let p = Progress {
                    done,
                    job: Some(job.clone()),
                    ..Default::default()
                };
                let at = second_job_meet(&zone, &p);
                assert_eq!(at.is_some(), done == 1, "{job:?} with {done} done: {at:?}");
                if let Some(at) = at {
                    let d = at.xz().distance(drop);
                    assert!(
                        (ALONG_MIN - 1.0..=ALONG_MAX + ASIDE).contains(&d),
                        "{d} m out"
                    );
                    if let Some((_, place)) = job.mark() {
                        assert!(d < Vec2::from(place).distance(drop), "past the place");
                    }
                }
            }
            let none = Progress {
                done: 1,
                job: None,
                ..Default::default()
            };
            assert!(
                second_job_meet(&zone, &none).is_none(),
                "no job, no meeting"
            );
        }
    }

    /// On the second job the director sets it out first, and sends nothing
    /// else out while it stands (nothing armed comes to the newcomer's
    /// first fight, nor answers its shots); it's nearer the drop than any
    /// armed machine already there. Wrecked, the valley's back to itself
    /// soon after.
    #[test]
    fn on_the_second_job_it_goes_out_before_anything_armed() {
        let mut app = app(marrow(true));
        let zone = app.world().resource::<Zone>().clone();
        let job = jobs(&zone)[0].clone();
        app.insert_resource(crate::progression::Progression(Progress {
            done: 1,
            job: Some(job),
            ..Default::default()
        }));
        // (Its first look was before the career was the second job's.)
        app.world_mut()
            .resource_mut::<super::super::director::Director>()
            .broken_placed = false;
        let drop = Vec2::from(
            zone.marker(&extraction_core::map::MarkerKind::Insertion)
                .unwrap()
                .at,
        );
        // Out on the ground: not a building's or a site's own residents
        // (a crawler in Marrow's bunker), who aren't on anyone's way.
        let robots = |app: &mut App| -> Vec<(Entity, bool, Vec2)> {
            app.world_mut()
                .run_system_once(
                    |q: Query<
                        (Entity, Has<Broken>, &GlobalTransform),
                        (
                            With<super::super::Robot>,
                            Without<super::super::indoor::LocalNav>,
                        ),
                    >| {
                        q.iter()
                            .map(|(e, b, g)| (e, b, g.translation().xz()))
                            .collect::<Vec<_>>()
                    },
                )
                .unwrap()
        };
        let mut at_start = None;
        let mut broken = None;
        for k in 0..90 * 150 {
            app.update();
            let now = robots(&mut app);
            let start = at_start.get_or_insert_with(|| now.iter().map(|r| r.0).collect::<Vec<_>>());
            if broken.is_none() {
                broken = now.iter().find(|r| r.1).map(|r| r.0);
                assert!(
                    broken.is_some() || k < 90,
                    "no broken walker set out on the second job"
                );
            }
            let new: Vec<_> = now
                .iter()
                .filter(|r| !r.1 && !start.contains(&r.0))
                .collect();
            assert!(
                new.is_empty(),
                "sent out at {:.0} s while it stood: {new:?}",
                k as f32 / 90.0
            );
        }
        // Nearer the drop than any armed machine the valley started with.
        let parts: Vec<(Entity, bool)> = app
            .world_mut()
            .run_system_once(|q: Query<&RobotPart>| {
                q.iter()
                    .filter(|p| p.role == PartRole::Weapon)
                    .map(|p| (p.robot, p.alive()))
                    .collect::<Vec<_>>()
            })
            .unwrap();
        let now = robots(&mut app);
        let names: Vec<(Entity, String)> = app
            .world_mut()
            .run_system_once(|q: Query<(Entity, &Name), With<super::super::Robot>>| {
                q.iter()
                    .map(|(e, n)| (e, n.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap();
        let name = |e: &Entity| {
            names
                .iter()
                .find(|n| n.0 == *e)
                .map_or("?".into(), |n| n.1.clone())
        };
        let b = now.iter().find(|r| r.1).expect("it's still there");
        for (e, _, at) in now.iter().filter(|r| !r.1) {
            if parts.iter().any(|(r, alive)| r == e && *alive) {
                assert!(
                    at.distance(drop) > b.2.distance(drop),
                    "an armed {} ({e:?}) at {at:?} stands nearer the drop {drop:?} than it ({:?})",
                    name(e),
                    b.2
                );
            }
        }
        assert!(
            !parts.iter().any(|(r, alive)| *r == b.0 && *alive),
            "its gun's alive"
        );
    }

    #[derive(Resource, Default)]
    struct Seen {
        shots: Vec<Entity>,
        all_shots: u32,
        struck: Vec<(PartRole, f32)>,
        shoves: u32,
    }

    fn count(
        mut shots: MessageReader<crate::weapons::ballistics::Shot>,
        mut hurt: MessageReader<crate::health::Hurt>,
        mut struck: MessageReader<super::super::damage::RobotStruck>,
        mut seen: ResMut<Seen>,
    ) {
        for s in shots.read() {
            seen.all_shots += 1;
            seen.shots.extend(s.shooter);
        }
        seen.shoves += hurt.read().filter(|h| h.amount == SHOVE_DAMAGE).count() as u32;
        let hits: Vec<(PartRole, f32)> = struck.read().map(|h| (h.role, h.dealt)).collect();
        seen.struck.extend(hits);
    }

    /// It never fires, even welded back up (a mender's work): it limps at
    /// you and shoves, a few hp a time.
    #[test]
    fn it_never_fires() {
        let mut app = crate::headless_app(
            LaunchOptions {
                map: extraction_core::map::TEST_FIELD_ID.into(),
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.init_resource::<Seen>().add_systems(PostUpdate, count);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let me = zone.on_ground(40.0, 255.0);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = me;
        app.update();
        let at = zone.on_ground(me.x + 9.0, me.z);
        let robot = app
            .world_mut()
            .run_system_once(move |mut c: Commands, zone: Res<Zone>| {
                spawn_broken_walker(&mut c, &zone, at, at + Vec3::Z * 300.0)
            })
            .unwrap();
        let hp0 = app.world().get::<crate::health::Health>(player).unwrap().hp;
        let mut closest = f32::MAX;
        for k in 0..90 * 40 {
            app.update();
            // Welded back up half way through.
            if k == 90 * 20 {
                app.world_mut()
                    .run_system_once(move |mut q: Query<&mut RobotPart>| {
                        for mut p in &mut q {
                            if p.robot == robot && p.role == PartRole::Weapon {
                                p.hp = p.max_hp;
                            }
                        }
                    })
                    .unwrap();
            }
            let r = app
                .world()
                .get::<GlobalTransform>(robot)
                .unwrap()
                .translation();
            let p = app.world().get::<Transform>(player).unwrap().translation;
            closest = closest.min(r.xz().distance(p.xz()));
        }
        let seen = app.world().resource::<Seen>();
        assert!(!seen.shots.contains(&robot), "it fired");
        assert!(
            closest < SHOVE_RANGE + 0.5,
            "it never came at you: {closest:.1} m at closest"
        );
        assert!(seen.shoves > 0, "it never shoved");
        let lost = hp0 - app.world().get::<crate::health::Health>(player).unwrap().hp;
        assert!(
            (lost - seen.shoves as f32 * SHOVE_DAMAGE).abs() < 0.01,
            "{lost} hp lost to {} shoves",
            seen.shoves
        );
    }

    /// Up to someone crouched where it can reach them (keeping low, as
    /// the driver says), it stops with its body clear of them and shoves
    /// from there: never on top of them, so never pressing them down
    /// through the ground (a crouched newcomer it limped up to sank under
    /// the field and fell: first_fight_broken, one-handed, 1 November).
    #[test]
    fn it_never_stands_on_someone_crouched() {
        let mut vr = crate::vr_harness::Vr::new(extraction_core::map::TEST_FIELD_ID);
        vr.app
            .init_resource::<Seen>()
            .add_systems(PostUpdate, count);
        let zone = vr.app.world().resource::<Zone>().clone();
        let me = zone.on_ground(40.0, 255.0);
        vr.stand(me, 0.0);
        vr.head(Vec3::new(0.0, 0.9, 0.0));
        vr.step(10);
        let at = zone.on_ground(me.x + 9.0, me.z);
        let robot = vr
            .app
            .world_mut()
            .run_system_once(move |mut c: Commands, zone: Res<Zone>| {
                spawn_broken_walker(&mut c, &zone, at, at + Vec3::Z * 300.0)
            })
            .unwrap();
        let (mut closest, mut sank) = (f32::MAX, f32::MIN);
        for _ in 0..90 * 40 {
            vr.head(Vec3::new(0.0, 0.9, 0.0));
            let mut h = vr
                .app
                .world_mut()
                .get_mut::<crate::health::Health>(vr.player)
                .unwrap();
            h.hp = h.max;
            h.down = false;
            vr.step(1);
            let r = vr
                .app
                .world()
                .get::<GlobalTransform>(robot)
                .unwrap()
                .translation();
            let feet = vr.feet();
            closest = closest.min(r.xz().distance(feet.xz()));
            sank = sank.max(zone.ground(feet.x, feet.z) - feet.y);
        }
        let seen = vr.app.world().resource::<Seen>();
        assert!(
            seen.shoves > 0,
            "it never shoved: {closest:.2} m at closest"
        );
        assert!(
            closest >= clear_of_you(1.0) - 0.05,
            "on top of you: {closest:.2} m from its middle (its feet reach {:.2})",
            super::super::anim::FOOT_RADIUS
        );
        assert!(sank < 0.3, "pressed {sank:.2} m under the ground");
    }

    /// A newcomer kills it: the reckless one, on its first raid, meets it
    /// on the way (`first_fight_broken`), draws and fights, and walks on.
    #[test]
    fn a_newcomer_kills_it() {
        let dir = std::env::temp_dir().join(format!("extraction-broken-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("trained"), "yes\n").unwrap();
        let opts = LaunchOptions {
            scenario: Some(crate::onboarding::first_hour::RAID.into()),
            personality: Some("reckless".into()),
            encounter: Some("broken".into()),
            data_dir: Some(dir.clone()),
            save: Some(dir.join("career.ron")),
            ..marrow(false)
        };
        let mut app = crate::headless_app(opts, false);
        app.init_resource::<Seen>().add_systems(PostUpdate, count);
        app.finish();
        app.cleanup();
        app.update();
        let mut verdict = None;
        for _ in 0..90 * 400 {
            app.update();
            let (wrecked, down) = app
                .world_mut()
                .run_system_once(
                    |r: Query<&super::super::Robot, With<Broken>>,
                     h: Query<&crate::health::Health, With<LocalPlayer>>| {
                        (r.iter().any(|r| r.wrecked), h.iter().any(|h| h.down))
                    },
                )
                .unwrap();
            if wrecked || down {
                verdict = Some((wrecked, down));
                break;
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        let log: Vec<String> = app
            .world_mut()
            .query_filtered::<&crate::bot::Bot, With<LocalPlayer>>()
            .iter(app.world())
            .flat_map(|b| b.log.clone())
            .collect();
        let (shots, shoves) = {
            let seen = app.world().resource::<Seen>();
            (
                format!(
                    "{} ({} all), struck {:?}",
                    seen.shots.len(),
                    seen.all_shots,
                    seen.struck
                ),
                seen.shoves,
            )
        };
        let tasks: Vec<String> = app
            .world_mut()
            .query_filtered::<&crate::bot::Bot, With<LocalPlayer>>()
            .iter(app.world())
            .map(|b| format!("failed {:?} front {:?}", b.failed, b.tasks.front()))
            .collect();
        assert_eq!(
            verdict,
            Some((true, false)),
            "(wrecked, newcomer down); {shots} shots, {shoves} shoves; {tasks:?}; its log:\n{}",
            log.join("\n")
        );
    }
}
