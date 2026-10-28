//! The second hour: the first time you meet each thing the yard doesn't
//! cover, out in the Zone, the driver says one short line on the radio
//! about it, once a career. A jam (and how to clear that kind), a machine
//! that heard you (and when it loses you), a drone's lock-on chirp, the
//! air going wrong ahead, the way underground, the sky turning before the
//! Surge, a walker you've lamed, a full pack, a gun that's getting worn,
//! the first wound that bleeds, the detector ticking, a thrown thing
//! showing one up, a fold, a well's pull.
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
    /// The detector ticking fast: an anomaly's edge is near.
    Detector,
    /// A thrown thing showed one up.
    Revealed,
    /// Through a fold (mirrors), somewhere else.
    Blinked,
    /// In a well's pull.
    Well,
    Death,
    ConvoyTook,
    KitRecovered,
    /// The first note read (its copy goes on the truck's board).
    NotePinned,
    /// A walker's gun settling on you: the shot is a breath away.
    Aimed,
    /// A sentry's chirp: it has you in its sweep.
    Sentry,
    /// In a hush: the quiet that presses.
    Hush,
    /// Home with a job on the card: it's paid when the card is pressed.
    Payday,
    /// A machine that was shooting at you lost you, and is coming to where
    /// it last saw you, or back past you once it's given up
    /// (`robots::search::ComingBack`): a moving shape behind low cover is
    /// glimpsed, and each glimpse moves its guess; a still one isn't.
    ComingToLook,
}

/// `Malfunction`, with an order (for the saved set).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum JamKind {
    Eject,
    Feed,
    Double,
    Short,
}

impl From<Malfunction> for JamKind {
    fn from(m: Malfunction) -> Self {
        match m {
            Malfunction::FailureToEject => JamKind::Eject,
            Malfunction::FailureToFeed => JamKind::Feed,
            Malfunction::DoubleFeed => JamKind::Double,
            Malfunction::ShortStroke => JamKind::Short,
        }
    }
}

pub const ALL: &[First] = &[
    First::Jam(JamKind::Eject),
    First::Jam(JamKind::Feed),
    First::Jam(JamKind::Double),
    First::Jam(JamKind::Short),
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
    First::Detector,
    First::Revealed,
    First::Blinked,
    First::Well,
    First::Death,
    First::ConvoyTook,
    First::KitRecovered,
    First::NotePinned,
    First::Aimed,
    First::Sentry,
    First::Hush,
    First::Payday,
    First::ComingToLook,
];

impl First {
    /// Its name in the save.
    pub fn key(self) -> String {
        format!("{self:?}")
    }

    /// Danger, now: what to do comes first in its line, so it's on the
    /// wrist's first page (the reason can wait for the next).
    pub fn urgent(self) -> bool {
        matches!(
            self,
            First::Jam(_)
                | First::Heard
                | First::DroneLock
                | First::Anomaly
                | First::Underground
                | First::Surge
                | First::Lamed
                | First::Bleeding
                | First::Blinked
                | First::Well
                | First::Aimed
                | First::Sentry
                | First::Hush
                | First::ComingToLook
        )
    }

    /// What the driver says. Each fits the radio (three lines of 26), uses
    /// no game words, and says what to do; an urgent one says it first.
    pub fn lines(self) -> &'static [&'static str] {
        match self {
            First::Jam(JamKind::Eject) => {
                &["Truck: \"Rack it hard. Stuck case; it'll spit it out.\""]
            }
            First::Jam(JamKind::Feed) => &["Truck: \"Slap the mag home, then rack. Didn't feed.\""],
            First::Jam(JamKind::Double) => {
                &["Truck: \"Mag out, rack, mag back in. Two rounds jammed.\""]
            }
            First::Jam(JamKind::Short) => {
                &["Truck: \"Work that bolt all the way. Cold oil; pull hard.\""]
            }
            First::Heard => &["Truck: \"Move off quiet, then go still. Something heard you.\""],
            First::LostYou => &["Truck: \"Lost you, has it? Good. Stay low a while.\""],
            First::DroneLock => &[
                "Truck: \"Run across it, to cover. That chirp is it taking aim.\"",
                "Truck: \"Under a roof's best. It sees down through trees.\"",
            ],
            First::Anomaly => &["Truck: \"Throw something in first. Air's wrong ahead.\""],
            First::Underground => {
                &["Truck: \"Torch on, go slow. Dark down there, and not empty.\""]
            }
            First::Surge => &["Truck: \"Come home. Sky's turning: that's the Surge coming.\""],
            First::Lamed => &["Truck: \"Get out of its sight. You've lamed it; it can't follow.\""],
            First::PackFull => &["Truck: \"Pack's full. Leave the worst of it behind.\""],
            First::Worn => {
                &["Truck: \"That gun's tired. It'll jam on you. Bench, when you're back.\""]
            }
            First::Bleeding => &["Truck: \"Dressing on, now: you're bleeding. Before you drop.\""],
            First::Detector => &["Truck: \"Hear it ticking? Faster's nearer. Sweep it slow.\""],
            First::Revealed => {
                &["Truck: \"That's how you find them. Bolts are cheap. Legs aren't.\""]
            }
            First::Blinked => &["Truck: \"Stop. Look before you step. You went through a fold.\""],
            First::Well => &["Truck: \"Back out the way you came. Now. Feel that pull?\""],
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
            First::Aimed => &["Truck: \"Break its line, now. It's aiming at you.\""],
            First::Sentry => &["Truck: \"Down, out of its sweep. A sentry has you.\""],
            First::Hush => &["Truck: \"Out of there, now. That dead quiet hurts.\""],
            First::Payday => &["Truck: \"Home. Now press the card: that's the job paid.\""],
            First::ComingToLook => {
                &["Truck: \"Low behind it, and keep still. It only finds what moves.\""]
            }
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
/// ...unless the second is danger now: then this, at least (time to read
/// the first one's opening words).
const URGENT_SPACING: f32 = 1.5;
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
                        machines_on_you,
                        lamed,
                        pack_full,
                        places,
                        worn,
                        bleeding,
                        anomaly_kit,
                        deaths,
                        convoy,
                        asked,
                        payday,
                    ),
                    speak,
                )
                    .chain()
                    .run_if(in_the_zone),
            )
            .add_systems(Update, spare_radio.run_if(in_the_zone));
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
    if f.urgent() {
        // Danger now: said at once, ahead of anything waiting (a walker's
        // aim came the same moment as "heard", and five seconds behind it
        // was after the shot).
        for l in f.lines().iter().rev() {
            queue.lines.push_front(fill(l));
        }
        let since = SPACING - queue.quiet;
        queue.quiet = queue.quiet.min((URGENT_SPACING - since).max(0.0));
    } else {
        queue.lines.extend(f.lines().iter().map(|l| fill(l)));
    }
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
    mut back: MessageReader<crate::robots::search::ComingBack>,
    minds: Query<&Awareness>,
    locals: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    mut m: Meet,
) {
    let Some((me, at)) = locals.iter().next() else {
        return;
    };
    let about_me = |robot: Entity| minds.get(robot).is_ok_and(|a| a.target == Some(me));
    let looked: Vec<Entity> = looking.read().map(|e| e.robot).collect();
    // One that was shooting at you, coming to look (or back past you).
    if back.read().any(|b| b.about == me) {
        m.meet(First::ComingToLook);
    }
    let robots: Vec<Entity> = looked
        .into_iter()
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

/// A walker's gun settling on you, a sentry's chirp near you, and the
/// first hush you're in.
fn machines_on_you(
    mut aims: MessageReader<crate::robots::walker::WalkerAim>,
    mut sentries: MessageReader<crate::robots::sentry::SentryEvent>,
    minds: Query<&Awareness>,
    locals: Query<(Entity, &GlobalTransform, Has<crate::anomaly::hush::InHush>), With<LocalPlayer>>,
    mut m: Meet,
) {
    let Some((me, at, hushed)) = locals.iter().next() else {
        return;
    };
    for ev in aims.read() {
        if minds.get(ev.walker).is_ok_and(|a| a.target == Some(me)) {
            m.meet(First::Aimed);
        }
    }
    for ev in sentries.read() {
        if ev.what == crate::robots::sentry::SentryAct::Chirp
            && ev.at.distance(at.translation()) < 70.0
        {
            m.meet(First::Sentry);
        }
    }
    if hushed {
        m.meet(First::Hush);
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

/// The detector, a thrown thing, a fold: how you find anomalies, and
/// what one did to you.
fn anomaly_kit(
    mut clicks: MessageReader<crate::anomaly::detector::DetectorClick>,
    mut acts: MessageReader<crate::anomaly::AnomalyEvent>,
    mut blinks: MessageReader<crate::anomaly::mirrors::Blinked>,
    mut wells: MessageReader<crate::anomaly::wells::EnteredWell>,
    locals: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    mut m: Meet,
) {
    let Some((me, at)) = locals.iter().next() else {
        return;
    };
    let near = |p: Vec3| p.distance(at.translation()) < NEAR_ME;
    // Yours: ticking where you are (in a hand or on you).
    if clicks.read().any(|c| c.near > 0.4 && near(c.at)) {
        m.meet(First::Detector);
    }
    if acts
        .read()
        .any(|e| e.what == crate::anomaly::AnomalyAct::Reveal && near(e.at))
    {
        m.meet(First::Revealed);
    }
    if blinks.read().any(|b| b.entity == me) {
        m.meet(First::Blinked);
    }
    if wells.read().any(|w| w.player == me) {
        m.meet(First::Well);
    }
}

/// Near enough to you to be yours: a detector on you, a thing you threw.
const NEAR_ME: f32 = 30.0;

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

/// The spare: home with no radio (it went with the rest when you fell),
/// the driver passes his through the hatch, or there'd be no calling him
/// on the next raid.
pub const SPARE_RADIO: &str = "Truck: \"No radio on you. Take my spare, off the ledge.\"";

/// The same for what a job needs taken out (a thing to leave, the camera):
/// lost at a fall, the job couldn't be done.
pub fn spare_for(name: &str) -> String {
    format!("Truck: \"Lost the {name}? Here's another, off the ledge.\"")
}

/// Seconds at the truck before the driver notices what's missing.
const SPARE_AFTER: f32 = 2.0;

#[allow(clippy::too_many_arguments)]
fn spare_radio(
    raid: Res<Raid>,
    time: Res<Time>,
    items: Query<(
        Entity,
        &crate::items::Item,
        Option<&crate::anomaly::artifacts::CarriedBy>,
        &GlobalTransform,
    )>,
    catalog: Res<crate::items::Catalog>,
    progression: Option<Res<crate::progression::Progression>>,
    stash: Query<&crate::interact::body::Backpack, With<crate::raid::truck::Stash>>,
    trucks: Query<&GlobalTransform, With<crate::raid::truck::Truck>>,
    locals: Query<Entity, With<LocalPlayer>>,
    hatch: Res<crate::raid::lev::Handovers>,
    mut handover: MessageWriter<crate::raid::lev::HandOver>,
    mut queue: ResMut<Queue>,
    mut home_for: Local<f32>,
    mut handed: Local<bool>,
) {
    if raid.phase != Phase::AtBase {
        *home_for = 0.0;
        *handed = false;
        return;
    }
    *home_for += time.delta_secs();
    // (What's on its way through the hatch already counts, once it's down.)
    if *handed || *home_for < SPARE_AFTER || hatch.busy() {
        return;
    }
    let (Some(me), Some(truck)) = (locals.iter().next(), trucks.iter().next()) else {
        return;
    };
    *handed = true;
    let in_crate = |e: Entity| stash.iter().any(|b| b.contents.contains(&e));
    // On you, in the crate, or lying in the truck (on the ledge already).
    let have = |id: &str| {
        items.iter().any(|(e, it, by, gt)| {
            catalog.get(it.kind()).id == id
                && (by.is_some_and(|b| b.0 == me)
                    || in_crate(e)
                    || gt.translation().distance(truck.translation()) < 3.0)
        })
    };
    use extraction_core::progression::Job;
    let job = progression.as_ref().and_then(|p| p.0.job.clone());
    let wanted: Option<String> = match job {
        Some(Job::Plant {
            item,
            planted: false,
            ..
        }) => Some(item),
        Some(Job::Photograph { taken: false, .. }) => Some("camera".into()),
        _ => None,
    };
    if !have("radio") {
        handover.write(crate::raid::lev::HandOver {
            id: "radio".into(),
            condition: 0.8,
        });
        queue.lines.push_back(SPARE_RADIO.into());
    }
    if let Some(id) = wanted.filter(|id| !have(id)) {
        let name = catalog
            .kind(&id)
            .map(|k| catalog.get(k).name.to_lowercase())
            .unwrap_or_else(|| id.replace('_', " "));
        handover.write(crate::raid::lev::HandOver { id, condition: 1.0 });
        queue.lines.push_back(spare_for(&name));
    }
}

/// Home from the Zone with a job on the card: nothing pays it but
/// pressing the card (the first newcomers took the truck home, and the
/// job sat unpaid into the next session).
fn payday(
    raid: Res<Raid>,
    progression: Option<Res<crate::progression::Progression>>,
    mut was_out: Local<bool>,
    mut m: Meet,
) {
    let home = raid.phase == Phase::AtBase;
    let back = *was_out && home;
    *was_out = matches!(raid.phase, Phase::Extracting { .. });
    if back
        && !m.met(First::Payday)
        && progression.is_some_and(|p| p.0.job.is_some() && p.0.last_raid != Some(raid.raids))
    {
        m.meet(First::Payday);
    }
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

    /// An urgent line's first sentence (what to do: they're written
    /// action first) is on the wrist's
    /// first page, captioned or not: nobody waits a page under fire.
    #[test]
    fn urgent_advice_is_on_the_wrists_first_page() {
        use crate::render::comfort::{Said, wrist_text};
        let flat = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
        for f in ALL.iter().filter(|f| f.urgent()) {
            for l in f.lines() {
                let (_, words) = crate::raid::captions::parse(l);
                let end = words.find(['.', '?', '!']).map_or(words.len(), |i| i + 1);
                let act = flat(&words[..end].to_uppercase());
                for (how, said) in [
                    ("plain", Said::Plain(l)),
                    ("captioned", Said::Captioned("LEV, RADIO", &words)),
                ] {
                    let page = flat(&wrist_text("SURGE 12M", 9.0, 90.0, false, Some(said), 0));
                    assert!(
                        page.contains(&act),
                        "{f:?}, {how}: \"{act}\" not on page one:\n{page}"
                    );
                }
            }
        }
    }

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

    /// Home with no radio anywhere (it went with the rest at a fall): the
    /// driver hands his spare over, and says so; with one, he doesn't.
    #[test]
    fn no_radio_home_gets_the_spare() {
        let spoken_spare = |app: &App| {
            app.world()
                .resource::<Spoken>()
                .0
                .iter()
                .filter(|l| *l == SPARE_RADIO)
                .count()
        };
        let mut app = app();
        // A radio in the stash crate: the driver says nothing.
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let radio = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind("radio").unwrap(),
                    transform: Transform::IDENTITY,
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        let stash = app
            .world_mut()
            .query_filtered::<Entity, With<crate::raid::truck::Stash>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .get_mut::<crate::interact::body::Backpack>(stash)
            .unwrap()
            .contents
            .push(radio);
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.update();
        app.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
        for _ in 0..(8 * 90) {
            app.update();
        }
        assert_eq!(spoken_spare(&app), 0);
        // Every radio gone: out and home, and the spare comes.
        let radios: Vec<Entity> = app
            .world_mut()
            .query::<(Entity, &crate::items::Item)>()
            .iter(app.world())
            .filter(|(_, i)| catalog.get(i.kind()).id == "radio")
            .map(|(e, _)| e)
            .collect();
        for r in radios {
            app.world_mut().despawn(r);
        }
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.update();
        app.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
        for _ in 0..(8 * 90) {
            app.update();
        }
        assert_eq!(spoken_spare(&app), 1);
    }

    /// Home from the Zone with the job unpaid: told to press the card, once.
    #[test]
    fn home_with_a_job_is_told_to_press_the_card() {
        let mut app = app();
        if app
            .world()
            .resource::<crate::progression::Progression>()
            .0
            .job
            .is_none()
        {
            app.world_mut()
                .resource_mut::<crate::progression::Progression>()
                .0
                .job = Some(extraction_core::progression::Job::Letter);
        }
        for _ in 0..2 {
            app.world_mut().resource_mut::<Raid>().phase = Phase::Extracting { left: 99.0 };
            app.update();
            app.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
            for _ in 0..(10 * 90) {
                app.update();
            }
        }
        let line = First::Payday.lines()[0];
        let spoken = &app.world().resource::<Spoken>().0;
        assert_eq!(
            spoken.iter().filter(|s| *s == line).count(),
            1,
            "{spoken:?}"
        );
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
        // And spaced: never two in the same second and a half (five
        // seconds, unless the second is danger now).
        let gaps = app
            .world()
            .resource::<Spoken>()
            .1
            .windows(2)
            .all(|w| w[1] - w[0] >= URGENT_SPACING - 0.05);
        assert!(gaps, "lines too close together");
    }

    /// Danger now goes ahead of anything waiting: a walker's aim isn't
    /// said after the line about being heard.
    #[test]
    fn danger_now_is_said_first() {
        let mut app = app();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.world_mut().write_message(MeetFirst(First::Worn));
        app.world_mut().write_message(MeetFirst(First::PackFull));
        app.update();
        app.world_mut().write_message(MeetFirst(First::Aimed));
        for _ in 0..(90 * 3) {
            app.update();
        }
        let spoken = app.world().resource::<Spoken>().0.clone();
        let aimed = fill(First::Aimed.lines()[0], 0, 0);
        let at = spoken.iter().position(|s| *s == aimed);
        assert!(at.is_some_and(|i| i <= 1), "aimed said late: {spoken:?}");
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

    /// The detector ticking near you, a thrown thing showing one up, and a
    /// fold you went through: each met from the anomalies' own messages,
    /// and said once.
    #[test]
    fn the_anomaly_kit_is_explained_once() {
        let mut app = app();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        let (me, at) = app
            .world_mut()
            .query_filtered::<(Entity, &GlobalTransform), With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .map(|(e, g)| (e, g.translation()))
            .expect("a player");
        let thing = app.world_mut().spawn_empty().id();
        for pass in 0..2 {
            let w = app.world_mut();
            // Far off, faint: not yet.
            w.write_message(crate::anomaly::detector::DetectorClick {
                detector: thing,
                at,
                held: true,
                near: 0.1,
            });
            w.write_message(crate::anomaly::AnomalyEvent {
                anomaly: thing,
                kind: extraction_core::anomaly::AnomalyKind::Snare,
                at: at + Vec3::X * 500.0,
                what: crate::anomaly::AnomalyAct::Reveal,
            });
            app.update();
            if pass == 0 {
                assert!(!app.world().resource::<Firsts>().met(First::Detector));
                assert!(!app.world().resource::<Firsts>().met(First::Revealed));
            }
            let w = app.world_mut();
            w.write_message(crate::anomaly::detector::DetectorClick {
                detector: thing,
                at,
                held: true,
                near: 0.8,
            });
            w.write_message(crate::anomaly::AnomalyEvent {
                anomaly: thing,
                kind: extraction_core::anomaly::AnomalyKind::Snare,
                at: at + Vec3::X * 6.0,
                what: crate::anomaly::AnomalyAct::Reveal,
            });
            w.write_message(crate::anomaly::mirrors::Blinked {
                entity: me,
                from: at,
                to: at + Vec3::X * 20.0,
            });
            w.write_message(crate::anomaly::wells::EnteredWell {
                player: me,
                well: thing,
            });
            app.update();
        }
        for _ in 0..90 * 20 {
            app.update();
        }
        let spoken = &app.world().resource::<Spoken>().0;
        for f in [
            First::Detector,
            First::Revealed,
            First::Blinked,
            First::Well,
        ] {
            assert!(app.world().resource::<Firsts>().met(f), "{f:?} not met");
            let n = spoken.iter().filter(|s| *s == f.lines()[0]).count();
            assert_eq!(n, 1, "{f:?} said {n} times");
        }
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
