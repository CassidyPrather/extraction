//! The logbook: a card on the truck bed's left wall, by the tailgate. After
//! each raid it says how it went — how long you were out, what you wrecked,
//! what came home with you (or what the convoy took) — under the career's
//! running record.

use bevy::prelude::*;
use std::collections::BTreeMap;

use super::truck::{FLOOR, Truck};
use super::{Phase, Raid};
use crate::anomaly::artifacts::CarriedBy;
use crate::items::{Catalog, Item};
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::rig::LocalPlayer;
use crate::robots::{PartDestroyed, PartRole, Robot, RobotKind};

/// The line for a raid lost to bleeding.
pub const BLED_OUT: &str = "Bled out. Carry two dressings.";
/// The line for a raid lost to the Surge.
pub const SURGE_CAUGHT: &str = "Caught by the Surge. Be home before it.";

/// What put you down, when it's knowable: said on the card with one plain
/// line of what would have helped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// Still bleeding, nothing hit or fell for [`BLED_AFTER`] seconds.
    Bled,
    /// A machine's hit in the last [`BLED_AFTER`] seconds, by name.
    Machine(&'static str),
    /// Out in the Surge.
    Surge,
}

impl Cause {
    pub fn line(self) -> String {
        match self {
            Cause::Bled => BLED_OUT.to_owned(),
            Cause::Surge => SURGE_CAUGHT.to_owned(),
            Cause::Machine(name) => {
                let a = if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
                    "an"
                } else {
                    "a"
                };
                format!("Killed by {a} {name}. Break its line sooner.")
            }
        }
    }
}

/// How the last raid went.
#[derive(Clone, Debug, PartialEq)]
pub struct RaidEntry {
    pub number: u32,
    pub home: bool,
    pub minutes: f32,
    /// Robots wrecked, by name.
    pub wrecked: BTreeMap<&'static str, u32>,
    /// Things carried home (or lost, if down).
    pub items: usize,
    /// Artifacts among them, by name.
    pub artifacts: Vec<String>,
    /// Nights slept out, in the safehouse.
    pub nights: u32,
    /// A leg broken out there: splinted or not, at the end.
    pub leg: Option<bool>,
    /// What put you down, when it's knowable (a raid lost).
    pub cause: Option<Cause>,
}

/// Seconds since the last hit or fall for a death to be the bleeding's.
pub const BLED_AFTER: f32 = 2.0;

#[derive(Resource, Default, Debug)]
pub struct Logbook {
    /// Tallied while out.
    wrecked: BTreeMap<&'static str, u32>,
    started: f32,
    pub last: Option<RaidEntry>,
    /// The last raid as the card shows it (saved with the career).
    pub written: Option<String>,
    /// Turned to the career page.
    pub career: bool,
    /// The last time you were hurt (seconds, `Time::elapsed`), and the
    /// machine that did it, if one did.
    last_hurt: Option<(f32, Option<&'static str>)>,
}

#[derive(Component)]
pub struct LogbookCard;

pub struct LogbookPlugin;

impl Plugin for LogbookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Logbook>().add_systems(
            Update,
            (
                spawn_card,
                tally_wrecks,
                note_hurts,
                write_entries,
                turn_page,
                card_text,
            )
                .chain()
                .after(super::depart)
                .after(super::downed),
        );
    }
}

fn spawn_card(mut commands: Commands, trucks: Query<Entity, Added<Truck>>) {
    for truck in &trucks {
        commands.spawn((
            Name::new("Logbook"),
            LogbookCard,
            super::truck::Pressable::Logbook,
            crate::interact::Grabbable {
                points: vec![crate::interact::GrabPoint::free()],
            },
            avian3d::prelude::Collider::cuboid(0.36, 0.3, 0.03),
            avian3d::prelude::CollisionLayers::new(
                crate::world::Layer::Item,
                avian3d::prelude::LayerMask::NONE,
            ),
            crate::render::legibility::TextSource::Logbook,
            crate::interact::read::Readable,
            ScreenText {
                text: String::new(),
                size: Vec2::new(0.36, 0.3),
                style: ScreenStyle::Card,
            },
            Transform::from_xyz(-1.12, FLOOR + 1.66, 0.8)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
            ChildOf(truck),
        ));
    }
}

fn robot_name(kind: RobotKind, grappler: bool, collector: bool) -> &'static str {
    match kind {
        RobotKind::Drone => "drone",
        RobotKind::Walker if grappler => "grappler",
        RobotKind::Walker if collector => "collector",
        RobotKind::Walker => "walker",
        RobotKind::Dirigible => "airship",
        RobotKind::Pylon => "pylon",
        RobotKind::Sorter => "sorter",
    }
}

/// Who last hurt you, and when (for what put you down).
fn note_hurts(
    mut hurts: MessageReader<crate::health::Hurt>,
    me: Query<Entity, With<LocalPlayer>>,
    robots: Query<(
        &Robot,
        Has<crate::robots::grappler::Grappler>,
        Has<crate::robots::collector::Collector>,
    )>,
    time: Res<Time>,
    mut book: ResMut<Logbook>,
) {
    for h in hurts.read() {
        if !me.contains(h.player) {
            continue;
        }
        let by =
            h.by.and_then(|b| robots.get(b).ok())
                .map(|(r, g, c)| robot_name(r.kind, g, c));
        book.last_hurt = Some((time.elapsed_secs(), by));
    }
}

fn tally_wrecks(
    mut destroyed: MessageReader<PartDestroyed>,
    robots: Query<(
        &Robot,
        Has<crate::robots::grappler::Grappler>,
        Has<crate::robots::collector::Collector>,
    )>,
    raid: Res<Raid>,
    mut book: ResMut<Logbook>,
) {
    for d in destroyed.read() {
        if d.role != PartRole::Core || raid.phase == Phase::AtBase {
            continue;
        }
        if let Ok((r, g, c)) = robots.get(d.robot) {
            *book.wrecked.entry(robot_name(r.kind, g, c)).or_default() += 1;
        }
    }
}

/// Opens an entry stepping out, closes it getting out or going down.
fn write_entries(
    raid: Res<Raid>,
    mut book: ResMut<Logbook>,
    players: Query<
        (
            Entity,
            Option<&crate::interact::wounds::Fracture>,
            Option<&crate::health::Health>,
        ),
        With<LocalPlayer>,
    >,
    carried: Query<(&Item, &CarriedBy)>,
    catalog: Res<Catalog>,
    house: Option<Res<crate::progression::safehouse::Safehouse>>,
    mut was: Local<Option<std::mem::Discriminant<Phase>>>,
    time: Res<Time>,
) {
    let now = std::mem::discriminant(&raid.phase);
    let before = was.replace(now);
    if before == Some(now) {
        return;
    }
    let back = std::mem::discriminant(&Phase::AtBase);
    if before == Some(back) && raid.phase == Phase::Out {
        book.wrecked.clear();
        book.started = raid.clock;
        book.last_hurt = None;
        return;
    }
    let home = matches!(raid.phase, Phase::Extracting { .. });
    if !home && !matches!(raid.phase, Phase::Down { .. }) {
        return;
    }
    let Some((player, fracture, health)) = players.iter().next() else {
        return;
    };
    let mut items = 0;
    let mut artifacts = Vec::new();
    for (item, by) in &carried {
        if by.0 != player {
            continue;
        }
        items += 1;
        let def = catalog.get(item.kind());
        if def.category == extraction_core::items::Category::Artifact {
            artifacts.push(def.name.clone());
        }
    }
    artifacts.sort();
    let entry = RaidEntry {
        number: raid.raids,
        home,
        minutes: (raid.clock - book.started).max(0.0) / 60.0,
        wrecked: std::mem::take(&mut book.wrecked),
        items,
        artifacts,
        nights: house.map_or(0, |h| h.nights_out),
        leg: fracture.map(|f| f.splinted),
        cause: (!home)
            .then(|| cause(&raid, health, book.last_hurt, time.elapsed_secs()))
            .flatten(),
    };
    book.written = Some(entry_text(&entry));
    book.last = Some(entry);
}

/// What put you down: the Surge, a machine's recent hit, or an open wound
/// with nothing else for a while. Anything else isn't named (gas, a fall,
/// an anomaly: no line rather than a wrong one).
fn cause(
    raid: &Raid,
    health: Option<&crate::health::Health>,
    last_hurt: Option<(f32, Option<&'static str>)>,
    now: f32,
) -> Option<Cause> {
    if raid.surging() {
        return Some(Cause::Surge);
    }
    if let Some((at, Some(name))) = last_hurt {
        if now - at <= BLED_AFTER {
            return Some(Cause::Machine(name));
        }
    }
    health
        .filter(|h| h.bleeding > 0.0 && h.since_hit >= BLED_AFTER)
        .map(|_| Cause::Bled)
}

/// "3 drones, 1 walker".
fn wreck_list(w: &BTreeMap<&'static str, u32>) -> String {
    let parts: Vec<String> = w
        .iter()
        .map(|(name, n)| {
            if *n == 1 {
                format!("1 {name}")
            } else {
                format!("{n} {name}s")
            }
        })
        .collect();
    if parts.is_empty() {
        "nothing".into()
    } else {
        parts.join(", ")
    }
}

fn entry_text(e: &RaidEntry) -> String {
    let mut lines = vec![format!(
        "RAID {}: {}, {:.0} MIN",
        e.number,
        if e.home { "HOME" } else { "LOST" },
        e.minutes.max(1.0)
    )];
    if e.nights > 0 {
        lines.push(if e.nights == 1 {
            "1 night out".to_owned()
        } else {
            format!("{} nights out", e.nights)
        });
    }
    match e.leg {
        Some(true) => lines.push("Broke a leg, splinted".to_owned()),
        Some(false) => lines.push("Broke a leg".to_owned()),
        None => {}
    }
    if let Some(c) = e.cause {
        lines.push(c.line());
    }
    lines.push(format!("Wrecked {}", wreck_list(&e.wrecked)));
    if e.home {
        lines.push(format!("Brought home {} things", e.items));
    } else {
        lines.push(format!("The convoy took {} things", e.items));
    }
    if !e.artifacts.is_empty() {
        lines.push(e.artifacts.join(", "));
    }
    lines.join("\n")
}

/// Pressing the logbook turns it over: the last raid, or the career.
fn turn_page(
    mut events: MessageReader<crate::interact::InteractEvent>,
    cards: Query<(), With<LogbookCard>>,
    mut book: ResMut<Logbook>,
) {
    for ev in events.read() {
        if let crate::interact::InteractEvent::Pressed { item, .. } = ev {
            if cards.contains(*item) {
                book.career = !book.career;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn card_text(
    raid: Res<Raid>,
    book: Res<Logbook>,
    progression: Res<crate::progression::Progression>,
    epilogue: Res<crate::raid::epilogue::Epilogue>,
    memorial: Res<crate::persist::Memorial>,
    stashes: Query<&crate::interact::body::Backpack, With<super::truck::Stash>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut cards: Query<&mut ScreenText, With<LogbookCard>>,
    added: Query<(), Added<LogbookCard>>,
    mut last: Local<String>,
) {
    let text = if book.career {
        let keys_home = stashes
            .iter()
            .flat_map(|s| s.contents.iter())
            .filter_map(|e| items.get(*e).ok())
            .filter(|i| {
                let id = &catalog.get(i.kind()).id;
                crate::capstone::KEYS.iter().any(|(k, _)| k == id)
            })
            .count();
        let page = crate::progression::career_page(
            &progression.0,
            &raid,
            &memorial.0,
            keys_home,
            epilogue.done,
        );
        format!("{page}\n\n(press: last raid)")
    } else {
        let record = format!(
            "{} raids, {} home, {} lost",
            raid.raids, raid.extractions, raid.deaths
        );
        // Lost, and the job still out there: what's waiting (reckless,
        // first_jobs: three raids lost of eight, and the card said only
        // what the convoy took).
        let waiting = match (&book.written, &progression.0.job) {
            (Some(e), Some(job)) if e.contains(": LOST") => {
                let place = crate::progression::job_place(job);
                if place.is_empty() {
                    "\nThe job's still on the card.".to_owned()
                } else {
                    format!("\nStill on the card: {}.", crate::raid::lines::the(&place))
                }
            }
            _ => String::new(),
        };
        match &book.written {
            Some(e) => format!("LOGBOOK\n{e}{waiting}\n\n{record}\n(press: career)"),
            None => format!("LOGBOOK\n\n{record}\n(press: career)"),
        }
    };
    // Fitting the text is not free: only when it changes.
    if *last == text && added.is_empty() {
        return;
    }
    *last = text.clone();
    for mut c in &mut cards {
        let text = crate::render::screen::wrap_to_fit(&text, c.size, c.style);
        if c.text != text {
            c.text = text.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raid_home_is_written_up() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        assert!(
            app.world_mut()
                .query_filtered::<(), With<LogbookCard>>()
                .iter(app.world())
                .count()
                == 1,
            "one logbook in the truck"
        );
        {
            let mut raid = app.world_mut().resource_mut::<Raid>();
            raid.phase = Phase::Out;
            raid.raids += 1;
        }
        app.update();
        // A drone goes down while we're out.
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let drone = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::robots::drone::spawn_drone(
                &mut c,
                Vec3::new(0.0, 80.0, 0.0),
                crate::robots::Job {
                    kind: crate::robots::JobKind::Watch,
                    site: Vec3::ZERO,
                    duration: 999.0,
                    exit: Vec3::ZERO,
                },
            )
        };
        queue.apply(app.world_mut());
        app.update();
        app.world_mut().write_message(PartDestroyed {
            robot: drone,
            part: drone,
            role: PartRole::Core,
            at: Vec3::ZERO,
        });
        app.world_mut().resource_mut::<Raid>().clock += 600.0;
        // And a hard landing breaks a leg; a splint goes on.
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .expect("a player");
        app.world_mut()
            .entity_mut(player)
            .insert(crate::interact::wounds::Fracture {
                leg: crate::rig::Side::Left,
                splinted: true,
            });
        app.update();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Extracting { left: 4.0 };
        app.update();
        let e = app
            .world()
            .resource::<Logbook>()
            .last
            .clone()
            .expect("an entry");
        assert!(e.home);
        assert_eq!(e.wrecked.get("drone"), Some(&1));
        assert!((e.minutes - 10.0).abs() < 0.5, "{}", e.minutes);
        app.update();
        let text = app
            .world_mut()
            .query_filtered::<&ScreenText, With<LogbookCard>>()
            .iter(app.world())
            .next()
            .unwrap()
            .text
            .clone();
        assert!(text.contains("HOME, 10 MIN"), "{text}");
        assert!(text.contains("Wrecked 1 drone"), "{text}");
        assert!(text.contains("Broke a leg, splinted"), "{text}");
        // Home, it's set: the next raid starts on two good legs.
        app.world_mut().resource_mut::<Raid>().phase = Phase::Extracting { left: 0.0 };
        app.update();
        assert_eq!(app.world().resource::<Raid>().phase, Phase::AtBase);
        assert!(
            app.world()
                .get::<crate::interact::wounds::Fracture>(player)
                .is_none(),
            "mended at home"
        );
    }

    /// A raid lost, and the job still out there: the logbook says what's
    /// waiting, not only what the convoy took (reckless, first_jobs: three
    /// raids of eight lost, and nothing on the card to go back for).
    #[test]
    fn a_raid_lost_says_whats_still_on_the_card() {
        let mut app = crate::raid::requests::tests::app();
        app.world_mut()
            .resource_mut::<crate::progression::Progression>()
            .0
            .job = Some(extraction_core::progression::Job::Plant {
            map: "marrow_creek".into(),
            place: "Radio mast".into(),
            item: "relay_box".into(),
            at: [-54.0, -78.8],
            planted: false,
        });
        let text = |app: &mut App| {
            app.update();
            app.update();
            let mut q = app
                .world_mut()
                .query_filtered::<&ScreenText, With<LogbookCard>>();
            q.single(app.world()).unwrap().text.replace('\n', " ")
        };
        app.world_mut().resource_mut::<Logbook>().written =
            Some("RAID 3: LOST, 2 MIN Wrecked nothing The convoy took 16 things".into());
        let lost = text(&mut app);
        assert!(
            lost.contains("Still on the card: the radio mast."),
            "{lost}"
        );
        app.world_mut().resource_mut::<Logbook>().written =
            Some("RAID 3: HOME, 2 MIN Wrecked nothing Brought home 16 things".into());
        let home = text(&mut app);
        assert!(!home.contains("Still on the card"), "{home}");
    }

    /// What put you down is named, with one plain line of what would have
    /// helped: bleeding (nothing hit for a while, the wound open; Marrow's
    /// first fight: the one dressing on the first wound, the second wound
    /// bled out in 15 s), a machine's hit, the Surge. Anything else isn't
    /// named.
    #[test]
    fn a_raid_lost_names_what_put_you_down() {
        #[derive(Clone, Copy)]
        enum How {
            Bled,
            Shot,
            Surge,
            Clean,
        }
        let lost = |how: How| {
            let opts = crate::LaunchOptions {
                robots: false,
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..3 {
                app.update();
            }
            {
                let mut raid = app.world_mut().resource_mut::<Raid>();
                raid.phase = Phase::Out;
                raid.raids += 1;
            }
            app.update();
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .expect("a player");
            if let How::Shot = how {
                let mut queue = bevy::ecs::world::CommandQueue::default();
                let drone = {
                    let mut c = Commands::new(&mut queue, app.world());
                    crate::robots::drone::spawn_drone(
                        &mut c,
                        Vec3::new(0.0, 80.0, 0.0),
                        crate::robots::Job {
                            kind: crate::robots::JobKind::Watch,
                            site: Vec3::ZERO,
                            duration: 999.0,
                            exit: Vec3::ZERO,
                        },
                    )
                };
                queue.apply(app.world_mut());
                app.world_mut().write_message(crate::health::Hurt {
                    player,
                    amount: 14.0,
                    from: Vec3::ZERO,
                    by: Some(drone),
                });
                app.update();
            }
            if let How::Surge = how {
                app.world_mut().resource_mut::<Raid>().clock = crate::raid::SURGE_AFTER + 10.0;
            }
            {
                let mut h = app
                    .world_mut()
                    .get_mut::<crate::health::Health>(player)
                    .unwrap();
                h.hp = 0.0;
                h.down = true;
                h.bleeding = if let How::Clean = how { 0.0 } else { 2.2 };
                h.since_hit = if let How::Bled | How::Clean = how {
                    14.0
                } else {
                    0.5
                };
            }
            app.world_mut().resource_mut::<Raid>().phase = Phase::Down { left: 6.0 };
            app.update();
            app.world().resource::<Logbook>().written.clone().unwrap()
        };
        let bled = lost(How::Bled);
        assert!(bled.contains(BLED_OUT), "{bled}");
        let shot = lost(How::Shot);
        assert!(shot.contains("Killed by a drone."), "{shot}");
        assert!(!shot.contains(BLED_OUT), "a hit took it: {shot}");
        let surge = lost(How::Surge);
        assert!(surge.contains(SURGE_CAUGHT), "{surge}");
        let clean = lost(How::Clean);
        for line in [BLED_OUT, SURGE_CAUGHT, "Killed by"] {
            assert!(!clean.contains(line), "nothing knowable: {clean}");
        }
        assert_eq!(
            Cause::Machine("airship").line(),
            "Killed by an airship. Break its line sooner."
        );
    }
}
