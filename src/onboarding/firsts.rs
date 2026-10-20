//! The second hour: the first time you meet each thing the yard doesn't
//! cover, out in the Zone, the driver says one short line on the radio
//! about it, once a career. A jam (and how to clear that kind), a machine
//! that heard you (and when it loses you), a drone's lock-on chirp, the
//! air going wrong ahead, the way underground, the sky turning before the
//! Surge, a walker you've lamed, a full pack, a gun that's getting worn,
//! the first wound that bleeds.
//! And the first time you go down, what came back and what didn't.
//!
//! What's been said is kept in the career (`firsts` in `career.ron`), so a
//! line is never said twice; lines are spaced so one never buries another
//! on the wrist, which only shows the newest.

use bevy::prelude::*;
use extraction_core::firearm::{Event as FireEvent, Malfunction};
use std::collections::{BTreeSet, VecDeque};

use crate::anomaly::Anomaly;
use crate::camp::TRAINING_YARD;
use crate::interact::{Held, InteractEvent};
use crate::launch::LaunchOptions;
use crate::raid::{Phase, Raid, SURGE_WARNING};
use crate::rig::LocalPlayer;
use crate::robots::{Awareness, PartDestroyed, PartRole};
use crate::weapons::{Gun, GunEvent};
use crate::world::Zone;

/// A first-time moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum First {
    Jam(JamKind),
    Heard,
    LostYou,
    DroneLock,
    Anomaly,
    Underground,
    Surge,
    Lamed,
    PackFull,
    Worn,
    /// Hit, and bleeding: it doesn't stop on its own.
    Bleeding,
    Death,
    ConvoyTook,
    KitRecovered,
    /// The first note read (its copy goes on the truck's board).
    NotePinned,
}

/// `Malfunction`, with an order (for the saved set).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum JamKind {
    Eject,
    Feed,
    Double,
}

impl From<Malfunction> for JamKind {
    fn from(m: Malfunction) -> Self {
        match m {
            Malfunction::FailureToEject => JamKind::Eject,
            Malfunction::FailureToFeed => JamKind::Feed,
            Malfunction::DoubleFeed => JamKind::Double,
        }
    }
}

pub const ALL: &[First] = &[
    First::Jam(JamKind::Eject),
    First::Jam(JamKind::Feed),
    First::Jam(JamKind::Double),
    First::Heard,
    First::LostYou,
    First::DroneLock,
    First::Anomaly,
    First::Underground,
    First::Surge,
    First::Lamed,
    First::PackFull,
    First::Worn,
    First::Bleeding,
    First::Death,
    First::ConvoyTook,
    First::KitRecovered,
    First::NotePinned,
];

impl First {
    /// Its name in the save.
    pub fn key(self) -> String {
        format!("{self:?}")
    }

    /// What the driver says. Each fits the wrist (three lines of 26), uses
    /// no game words, and says what to do.
    pub fn lines(self) -> &'static [&'static str] {
        match self {
            First::Jam(JamKind::Eject) => {
                &["Truck: \"Stuck case? Rack it hard, it'll spit it out.\""]
            }
            First::Jam(JamKind::Feed) => &["Truck: \"Didn't feed. Slap the mag home, then rack.\""],
            First::Jam(JamKind::Double) => {
                &["Truck: \"Two rounds jammed. Mag out, rack, mag back in.\""]
            }
            First::Heard => &["Truck: \"Something heard you. Move off quiet, then go still.\""],
            First::LostYou => &["Truck: \"Lost you, has it? Good. Stay low a while.\""],
            First::DroneLock => &["Truck: \"That chirp is it taking aim. Get behind something.\""],
            First::Anomaly => &["Truck: \"Air's wrong ahead. Throw something in first.\""],
            First::Underground => {
                &["Truck: \"Dark down there, and not empty. Torch on, go slow.\""]
            }
            First::Surge => &["Truck: \"Sky's turning. That's the Surge coming. Come home.\""],
            First::Lamed => &["Truck: \"You've lamed it. It can't chase you now.\""],
            First::PackFull => &["Truck: \"Pack's full. Leave the worst of it behind.\""],
            First::Worn => {
                &["Truck: \"That gun's tired. It'll jam on you. Bench, when you're back.\""]
            }
            First::Bleeding => &["Truck: \"You're bleeding. Dressing on it now, or you'll drop.\""],
            // `{s}`: things in the crate; `{n}`: things left where you fell.
            First::Death => &[
                "Truck: \"Got you back. Don't ask how. Crate's as you left it: {s} things.\"",
                "Truck: \"{n} things you carried are where you fell.\"",
                "Truck: \"Beat their convoy there and they're yours again.\"",
            ],
            First::ConvoyTook => {
                &["Truck: \"That's through the Gate. Kept, they call it. Be quicker.\""]
            }
            First::NotePinned => {
                &["Truck: \"Reading their notes? I keep copies. Board in the back.\""]
            }
            First::KitRecovered => &["Truck: \"That's how you get it back. Take it home.\""],
        }
    }
}

/// The moments already met this career.
#[derive(Resource, Default, Debug, Clone)]
pub struct Firsts(pub BTreeSet<String>);

impl Firsts {
    pub fn met(&self, f: First) -> bool {
        self.0.contains(&f.key())
    }
}

/// A first-time moment was met (and its lines queued), for the bot run and
/// its report.
#[derive(Message, Clone, Copy, Debug)]
pub struct FirstMet(pub First);

/// Everything these said, and when (for the bot run's report and tests).
#[derive(Resource, Default, Debug, Clone)]
pub struct Spoken(pub Vec<String>, pub Vec<f32>);

/// Other systems' moments: write this when one happens (each is said once
/// a career, like the rest). E.g. `MeetFirst(First::NotePinned)` when a
/// note is read.
#[derive(Message, Clone, Copy, Debug)]
pub struct MeetFirst(pub First);

/// Lines waiting their turn on the wrist.
#[derive(Resource, Default)]
struct Queue {
    lines: VecDeque<String>,
    quiet: f32,
}

/// Seconds between two of these lines.
const SPACING: f32 = 5.0;
/// How close to an anomaly the air "goes wrong" (beyond its edge).
const ANOMALY_NEAR: f32 = 4.0;
/// Below this condition a gun jams noticeably more (`firearm::malfunction_chance`).
const WORN: f32 = 0.5;

pub struct FirstsPlugin;

impl Plugin for FirstsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Firsts>()
            .init_resource::<Queue>()
            .init_resource::<Spoken>()
            .add_message::<FirstMet>()
            .add_message::<MeetFirst>()
            .add_systems(
                Update,
                (
                    (
                        jams,
                        robots_heard,
                        drone_locks,
                        lamed,
                        pack_full,
                        places,
                        worn,
                        bleeding,
                        deaths,
                        convoy,
                        asked,
                    ),
                    speak,
                )
                    .chain()
                    .run_if(in_the_zone),
            );
    }
}

/// Not in the yard: it teaches all this itself.
fn in_the_zone(opts: Res<LaunchOptions>) -> bool {
    opts.map != TRAINING_YARD
}

/// Mark `f` met; true if it's new (and its lines are queued).
fn meet(
    f: First,
    firsts: &mut Firsts,
    queue: &mut Queue,
    out: &mut MessageWriter<FirstMet>,
    fill: &dyn Fn(&str) -> String,
) -> bool {
    if !firsts.0.insert(f.key()) {
        return false;
    }
    info!("first time: {f:?}");
    queue.lines.extend(f.lines().iter().map(|l| fill(l)));
    out.write(FirstMet(f));
    true
}

#[derive(bevy::ecs::system::SystemParam)]
struct Meet<'w> {
    firsts: ResMut<'w, Firsts>,
    queue: ResMut<'w, Queue>,
    out: MessageWriter<'w, FirstMet>,
}

impl Meet<'_> {
    fn meet(&mut self, f: First) -> bool {
        meet(f, &mut self.firsts, &mut self.queue, &mut self.out, &|l| {
            l.to_string()
        })
    }

    fn meet_filled(&mut self, f: First, fill: &dyn Fn(&str) -> String) -> bool {
        meet(f, &mut self.firsts, &mut self.queue, &mut self.out, fill)
    }

    fn met(&self, f: First) -> bool {
        self.firsts.met(f)
    }
}

fn local_holds(held: &Query<&Held>, locals: &Query<(), With<LocalPlayer>>, e: Entity) -> bool {
    held.get(e).is_ok_and(|h| locals.contains(h.player))
}

fn jams(
    mut events: MessageReader<GunEvent>,
    held: Query<&Held>,
    locals: Query<(), With<LocalPlayer>>,
    mut m: Meet,
) {
    for ev in events.read() {
        if let FireEvent::Malfunctioned(kind) = ev.event {
            if local_holds(&held, &locals, ev.gun) {
                m.meet(First::Jam(kind.into()));
            }
        }
    }
}

/// A machine turned to look for you; later, gave up.
fn robots_heard(
    mut looking: MessageReader<crate::robots::search::Investigating>,
    mut noticed: MessageReader<crate::robots::Noticed>,
    mut lost: MessageReader<crate::robots::search::LostYou>,
    minds: Query<&Awareness>,
    locals: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    mut m: Meet,
) {
    let Some((me, at)) = locals.iter().next() else {
        return;
    };
    let about_me = |robot: Entity| minds.get(robot).is_ok_and(|a| a.target == Some(me));
    let robots: Vec<Entity> = looking
        .read()
        .map(|e| e.robot)
        .chain(noticed.read().map(|e| e.robot))
        .collect();
    if robots.into_iter().any(about_me) {
        m.meet(First::Heard);
    }
    for ev in lost.read() {
        // Only once it's been after you, and somewhere near.
        if m.met(First::Heard) && ev.at.distance(at.translation()) < 80.0 {
            m.meet(First::LostYou);
        }
    }
}

fn drone_locks(
    mut events: MessageReader<crate::robots::drone::DroneLock>,
    minds: Query<&Awareness>,
    locals: Query<Entity, With<LocalPlayer>>,
    mut m: Meet,
) {
    let Some(me) = locals.iter().next() else {
        return;
    };
    for ev in events.read() {
        if minds.get(ev.drone).is_ok_and(|a| a.target == Some(me)) {
            m.meet(First::DroneLock);
        }
    }
}

/// A walker lost a leg. (Whoever shot it, it's a lame walker in front of
/// you either way.)
fn lamed(mut events: MessageReader<PartDestroyed>, mut m: Meet) {
    for ev in events.read() {
        if matches!(ev.role, PartRole::Leg(_)) {
            m.meet(First::Lamed);
        }
    }
}

fn pack_full(
    mut events: MessageReader<InteractEvent>,
    held: Query<&Held>,
    locals: Query<(), With<LocalPlayer>>,
    mut m: Meet,
) {
    for ev in events.read() {
        if let InteractEvent::Refused { item } = ev {
            // Released into a full bag (no longer held) or stowed with no
            // room (still held): either way it's ours if no one else has it.
            if held.get(*item).is_err() || local_holds(&held, &locals, *item) {
                m.meet(First::PackFull);
            }
        }
    }
}

/// Where you are: by an anomaly, below ground, under a turning sky.
fn places(
    anomalies: Query<(&Anomaly, &GlobalTransform)>,
    locals: Query<&GlobalTransform, With<LocalPlayer>>,
    zone: Res<Zone>,
    raid: Res<Raid>,
    mut m: Meet,
) {
    let Some(p) = locals.iter().next().map(|g| g.translation()) else {
        return;
    };
    if raid.phase == Phase::AtBase {
        return;
    }
    if !m.met(First::Anomaly) {
        let near = anomalies.iter().any(|(a, gt)| {
            let d = gt.translation() - p;
            Vec2::new(d.x, d.z).length() < a.radius + ANOMALY_NEAR && d.y.abs() < 4.0
        });
        if near {
            m.meet(First::Anomaly);
        }
    }
    if !m.met(First::Underground) && crate::world::underground::floor_under(&zone, p).is_some() {
        m.meet(First::Underground);
    }
    if !m.met(First::Surge) && raid.surge_in() <= SURGE_WARNING {
        m.meet(First::Surge);
    }
}

fn worn(guns: Query<(&Gun, &Held)>, locals: Query<(), With<LocalPlayer>>, mut m: Meet) {
    if m.met(First::Worn) {
        return;
    }
    let worn = guns
        .iter()
        .any(|(g, h)| locals.contains(h.player) && g.firearm.condition < WORN);
    if worn {
        m.meet(First::Worn);
    }
}

/// Hit, and bleeding: the wrist's "BLEED" is easy to miss, and a wound
/// doesn't stop on its own.
fn bleeding(locals: Query<&crate::health::Health, With<LocalPlayer>>, mut m: Meet) {
    if m.met(First::Bleeding) {
        return;
    }
    if locals.iter().any(|h| !h.down && h.bleeding > BLEEDING) {
        m.meet(First::Bleeding);
    }
}

/// Bleeding faster than this (HP a second) is worth a word: a graze
/// that's about to stop isn't.
const BLEEDING: f32 = 0.1;

/// The first time you wake in the truck after going down: what's in the
/// crate (all that came back), and how much lies where you fell.
fn deaths(
    raid: Res<Raid>,
    bags: Query<(
        &crate::interact::body::Backpack,
        Has<crate::raid::truck::Stash>,
        Option<&crate::raid::Remains>,
    )>,
    locals: Query<Entity, With<LocalPlayer>>,
    mut was_down: Local<bool>,
    mut waking: Local<Option<u32>>,
    mut m: Meet,
) {
    let down = matches!(raid.phase, Phase::Down { .. });
    if *was_down && !down && !m.met(First::Death) {
        *waking = Some(0);
    }
    *was_down = down;
    // Count once what was carried has been laid down where you fell (that
    // happens later in the frame you wake, in its own system), or after a
    // moment if there was nothing to lay down.
    let Some(frames) = waking.as_mut() else {
        return;
    };
    *frames += 1;
    let me = locals.iter().next();
    let fell: Vec<usize> = bags
        .iter()
        .filter(|(_, _, r)| r.is_some_and(|r| Some(r.owner) == me))
        .map(|(b, ..)| b.contents.len())
        .collect();
    if fell.is_empty() && *frames < 30 {
        return;
    }
    *waking = None;
    let crate_n: usize = bags
        .iter()
        .filter(|(_, stash, _)| *stash)
        .map(|(b, ..)| b.contents.len())
        .sum();
    let fell_n: usize = fell.iter().sum();
    m.meet_filled(First::Death, &|l| fill(l, crate_n, fell_n));
}

/// A line with its counts in.
pub fn fill(line: &str, in_crate: usize, fell: usize) -> String {
    let things = |n: usize| {
        if n == 1 {
            "1 thing".to_string()
        } else {
            format!("{n} things")
        }
    };
    line.replace("{s} things", &things(in_crate))
        .replace("{n} things", &things(fell))
}

/// What became of your kit, the first time each way.
fn convoy(
    mut events: MessageReader<crate::robots::collector::ConvoyEvent>,
    mut recovered: Local<Option<u32>>,
    memorial: Option<Res<crate::persist::Memorial>>,
    mut m: Meet,
) {
    for ev in events.read() {
        if let crate::robots::collector::ConvoyEvent::Delivered { .. } = ev {
            m.meet(First::ConvoyTook);
        }
    }
    let Some(memorial) = memorial else {
        return;
    };
    let now = memorial.0.recovered;
    if recovered.is_some_and(|r| now > r) {
        m.meet(First::KitRecovered);
    }
    *recovered = Some(now);
}

fn asked(mut asks: MessageReader<MeetFirst>, mut m: Meet) {
    for MeetFirst(f) in asks.read() {
        m.meet(*f);
    }
}

/// One line at a time, a few seconds apart.
fn speak(
    mut queue: ResMut<Queue>,
    mut raid: ResMut<Raid>,
    mut spoken: ResMut<Spoken>,
    time: Res<Time>,
) {
    queue.quiet -= time.delta_secs();
    if queue.quiet > 0.0 {
        return;
    }
    if let Some(line) = queue.lines.pop_front() {
        spoken.0.push(line.clone());
        spoken.1.push(time.elapsed_secs());
        raid.say(line);
        queue.quiet = SPACING;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every line fits the wrist whole, with no game words in it.
    #[test]
    fn every_first_line_fits_and_speaks_plainly() {
        const BANNED: [&str; 6] = ["loot", "quest", "spawn", "respawn", "hp", "xp"];
        for f in ALL {
            assert!(!f.lines().is_empty(), "{f:?} says nothing");
            for l in f.lines() {
                let l = &fill(l, 999, 999);
                let n = crate::render::screen::wrap(l, 26, usize::MAX).len();
                assert!(n <= 3, "{f:?}: {n} lines: {l}");
                for w in l
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| !w.is_empty())
                {
                    assert!(!BANNED.contains(&w), "{f:?} says '{w}': {l}");
                }
            }
        }
        let keys: BTreeSet<String> = ALL.iter().map(|f| f.key()).collect();
        assert_eq!(keys.len(), ALL.len(), "every moment has its own key");
    }

    fn app() -> App {
        let opts = crate::LaunchOptions {
            robots: false,
            map: "marrow_creek".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..5 {
            app.update();
        }
        app
    }

    /// The moments that come from a message, met twice over through the
    /// game's own messages (and the Surge and a death through the raid's
    /// state): each one's lines are said once, a few seconds apart.
    #[test]
    fn every_line_fires_at_most_once() {
        let mut app = app();
        let item = app.world_mut().spawn_empty().id();
        let robot = app.world_mut().spawn_empty().id();
        let part = app.world_mut().spawn_empty().id();
        for _ in 0..2 {
            let w = app.world_mut();
            w.write_message(InteractEvent::Refused { item });
            w.write_message(PartDestroyed {
                robot,
                part,
                role: PartRole::Leg(0),
                at: Vec3::ZERO,
            });
            w.write_message(crate::robots::collector::ConvoyEvent::Delivered { robot });
            app.update();
            // Out, the sky turning; down; home.
            {
                let mut raid = app.world_mut().resource_mut::<Raid>();
                raid.phase = Phase::Out;
                raid.clock = crate::raid::SURGE_AFTER - SURGE_WARNING + 1.0;
            }
            app.update();
            app.world_mut().resource_mut::<Raid>().phase = Phase::Down { left: 99.0 };
            app.update();
            app.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
            app.update();
        }
        // A minute and a half: every queued line said.
        for _ in 0..(90 * 90) {
            app.update();
        }
        let spoken = app.world().resource::<Spoken>().0.clone();
        for f in [
            First::PackFull,
            First::Lamed,
            First::ConvoyTook,
            First::Surge,
            First::Death,
        ] {
            for l in f.lines() {
                let l = fill(l, 0, 0);
                let n = spoken.iter().filter(|s| **s == l).count();
                assert_eq!(n, 1, "{f:?} said {n} times: {l}");
            }
        }
        let firsts = app.world().resource::<Firsts>();
        assert!(firsts.met(First::Death) && firsts.met(First::Surge));
        // And spaced: never two in the same few seconds.
        let gaps = app
            .world()
            .resource::<Spoken>()
            .1
            .windows(2)
            .all(|w| w[1] - w[0] >= SPACING - 0.05);
        assert!(gaps, "lines too close together");
    }

    /// The first wound that bleeds is named once, not on every hit after.
    #[test]
    fn the_first_bleed_is_said_once() {
        let mut app = app();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        let me = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .expect("a player");
        let line = First::Bleeding.lines()[0];
        for _ in 0..2 {
            app.world_mut()
                .get_mut::<crate::health::Health>(me)
                .unwrap()
                .bleeding = 0.5;
            for _ in 0..90 {
                app.update();
            }
            // Dressed.
            app.world_mut()
                .get_mut::<crate::health::Health>(me)
                .unwrap()
                .bleeding = 0.0;
            for _ in 0..90 * 6 {
                app.update();
            }
        }
        let spoken = &app.world().resource::<Spoken>().0;
        let n = spoken.iter().filter(|s| *s == line).count();
        assert_eq!(n, 1, "said {n} times");
        assert!(app.world().resource::<Firsts>().met(First::Bleeding));
    }

    /// Met once, kept in the career: a later session doesn't say it again.
    #[test]
    fn firsts_are_kept_in_the_career() {
        let dir = std::env::temp_dir().join(format!("extraction-firsts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("career.ron");
        let open = |path: &std::path::Path| {
            let opts = crate::LaunchOptions {
                robots: false,
                save: Some(path.to_owned()),
                map: "marrow_creek".into(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..3 {
                app.update();
            }
            app
        };
        let mut app = open(&path);
        let item = app.world_mut().spawn_empty().id();
        app.world_mut()
            .write_message(InteractEvent::Refused { item });
        app.update();
        assert!(app.world().resource::<Firsts>().met(First::PackFull));
        app.world_mut().write_message(crate::persist::SaveNow);
        app.update();
        drop(app);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("PackFull"), "saved: {text}");
        let mut app = open(&path);
        assert!(
            app.world().resource::<Firsts>().met(First::PackFull),
            "loaded"
        );
        let item = app.world_mut().spawn_empty().id();
        app.world_mut()
            .write_message(InteractEvent::Refused { item });
        for _ in 0..(90 * 8) {
            app.update();
        }
        let spoken = &app.world().resource::<Spoken>().0;
        assert!(spoken.is_empty(), "said again after loading: {spoken:?}");
        drop(app);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Down for real (health out, the raid's own way home): waking in the
    /// truck, the driver says how much the crate holds and how much lies
    /// where you fell, with the counts right.
    #[test]
    fn the_first_death_says_what_came_back() {
        let dir = std::env::temp_dir().join(format!("extraction-death-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let opts = crate::LaunchOptions {
            robots: false,
            save: Some(dir.join("career.ron")),
            map: "marrow_creek".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..30 {
            app.update();
        }
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        // Out in the Zone, two things in the pack.
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        let pack = app
            .world_mut()
            .query::<(Entity, &crate::interact::body::Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == player && b.carried)
            .map(|(e, _)| e)
            .expect("a pack");
        for id in ["canned_stew", "field_dressing"] {
            let catalog = app.world().resource::<crate::items::Catalog>().clone();
            let kind = catalog.kind(id).unwrap();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let e = {
                let mut commands = Commands::new(&mut queue, app.world());
                crate::items::spawn_item(
                    &mut commands,
                    &catalog,
                    crate::items::SpawnItem {
                        kind,
                        transform: Transform::default(),
                        condition: 1.0,
                    },
                )
            };
            queue.apply(app.world_mut());
            app.world_mut()
                .get_mut::<crate::interact::body::Backpack>(pack)
                .unwrap()
                .contents
                .push(e);
        }
        let in_crate = app
            .world_mut()
            .query_filtered::<&crate::interact::body::Backpack, With<crate::raid::truck::Stash>>()
            .iter(app.world())
            .map(|b| b.contents.len())
            .sum::<usize>();
        // Well out from the truck.
        {
            let zone = app.world().resource::<Zone>().clone();
            let truck = crate::raid::truck::base_pose(&zone).translation;
            let at = zone.on_ground(truck.x + 60.0, truck.z + 20.0) + Vec3::Y * 0.1;
            let mut t = app.world_mut().get_mut::<Transform>(player).unwrap();
            t.translation = at;
        }
        for _ in 0..30 {
            app.update();
        }
        app.world_mut()
            .get_mut::<crate::health::Health>(player)
            .unwrap()
            .hp = 0.0;
        // Down, the grey, the truck; then the driver's lines.
        for _ in 0..(90 * 30) {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Raid>().phase,
            Phase::AtBase,
            "awake in the truck, not straight back out in the Zone"
        );
        let spoken = app.world().resource::<Spoken>().0.clone();
        let want = fill(First::Death.lines()[0], in_crate, 2);
        assert!(spoken.contains(&want), "{want:?} not in {spoken:#?}");
        let want = fill(First::Death.lines()[1], in_crate, 2);
        assert!(spoken.contains(&want), "{want:?} not in {spoken:#?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
