//! The second hour: the first time you meet each thing the yard doesn't
//! cover, out in the Zone, the driver says one short line on the radio
//! about it, once a career. A jam (and how to clear that kind), a machine
//! that heard you (and when it loses you), a drone's lock-on chirp, the
//! air going wrong ahead, the way underground, the sky turning before the
//! Surge, a walker you've lamed, a full pack, a gun that's getting worn,
//! the detector ticking, a thrown thing
//! showing one up, a fold, a well's pull.
//! And the first time you go down, what came back and what didn't.
//!
//! What's been said is kept in the career (`firsts` in `career.ron`), so a
//! line is never said twice; lines are spaced so one never buries another
//! on the wrist, which only shows the newest.

use bevy::prelude::*;
use extraction_core::firearm::{Event as FireEvent, Malfunction};
use std::collections::{BTreeSet, VecDeque};

use super::pickups::Family;
use crate::anomaly::Anomaly;
use crate::camp::TRAINING_YARD;
use crate::interact::{Held, InteractEvent};
use crate::launch::LaunchOptions;
use crate::raid::{Phase, Raid, SURGE_WARNING, Show};
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
    /// A kind so often in hand its callout's gone quiet (#107): how to ask
    /// again.
    Recall,
    /// A walker's gun settling on you: the shot is a breath away.
    Aimed,
    /// A sentry's chirp: it has you in its sweep.
    Sentry,
    /// In a hush: the quiet that presses.
    Hush,
    /// Home with a job on the card: it's paid when the card is pressed.
    Payday,
    /// Home from a raid, off again to the truck's tail with finds still in
    /// the pack: what's on you goes at a fall, the stash keeps.
    StashFinds,
    /// Home with an empty stripper clip and a box of its rounds to hand:
    /// they fill by hand, and a raid's no place to find that out.
    EmptyClips,
    /// A machine that was shooting at you lost you, and is coming to where
    /// it last saw you, or back past you once it's given up
    /// (`robots::search::ComingBack`): a moving shape behind low cover is
    /// glimpsed, and each glimpse moves its guess; a still one isn't.
    ComingToLook,
    /// A broken walker noticed you (`robots::broken`): its gun's dead, its
    /// eye isn't. The first machine you can win against.
    BrokenNoticed,
    /// A broken walker shoved you: it can still hurt, close in.
    Shoved,
    /// A heavy's arm rising at you: slow, and it sees well, so hiding
    /// doesn't lose it; walking away does.
    Heavy,
    /// A hound after you.
    Hound,
    /// A crawler charging its spark gun at you.
    Crawler,
    /// A minelayer in sight, or one of its mines blinking near you: watch
    /// your feet.
    Minelayer,
    /// A grappler's harpoon thrown at you, hit or miss.
    Grappler,
    /// A spotter's eye swinging onto you: it's building a fix to call the
    /// rest.
    Spotter,
    /// A dirigible that sees you, or you near its lowered pod: it sends
    /// for those that shoot.
    Dirigible,
    /// The first of a family picked up out there: what it's for
    /// (`onboarding::pickups`).
    Picked(Family),
    /// A machine after you ran into a burner, a snare, a lift or a well,
    /// in your sight: chasing, they don't look where they go.
    Lured,
    /// A machine lost you through a fold (a mirror), in your sight.
    Dazzled,
    /// A machine in a hush went deaf, in your sight.
    Deafened,
    /// A machine stiffened in a frost, in your sight.
    Stiffened,
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
    First::Lured,
    First::Dazzled,
    First::Deafened,
    First::Stiffened,
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
    First::Detector,
    First::Revealed,
    First::Blinked,
    First::Well,
    First::Death,
    First::ConvoyTook,
    First::KitRecovered,
    First::NotePinned,
    First::Recall,
    First::Aimed,
    First::Sentry,
    First::Hush,
    First::Payday,
    First::StashFinds,
    First::EmptyClips,
    First::ComingToLook,
    First::BrokenNoticed,
    First::Shoved,
    First::Heavy,
    First::Hound,
    First::Crawler,
    First::Minelayer,
    First::Grappler,
    First::Spotter,
    First::Dirigible,
    First::Picked(Family::GunCare),
    First::Picked(Family::CleaningRod),
    First::Picked(Family::Matches),
    First::Picked(Family::Salvage),
    First::Picked(Family::MachineParts),
    First::Picked(Family::Armour),
    First::Picked(Family::Injector),
    First::Picked(Family::Grenade),
    First::Picked(Family::EmpTrap),
    First::Picked(Family::Knife),
    First::Picked(Family::Spanner),
    First::Picked(Family::Bench),
    First::Picked(Family::Wedge),
    First::Picked(Family::Binoculars),
    First::Picked(Family::LeadCase),
    First::Picked(Family::Trinkets),
    First::Picked(Family::Rounds),
    First::Picked(Family::GunParts),
];

impl First {
    /// Its name in the save.
    pub fn key(self) -> String {
        format!("{self:?}")
    }

    /// Where its lines show besides the wrist: danger now holds the view
    /// for its reading time; the rest are captions to act on.
    pub fn show(self) -> Show {
        if self.urgent() {
            Show::Urgent
        } else {
            Show::View
        }
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
                | First::Blinked
                | First::Well
                | First::Aimed
                | First::Sentry
                | First::Hush
                | First::ComingToLook
                | First::BrokenNoticed
                | First::Shoved
                | First::Heavy
                | First::Hound
                | First::Crawler
                | First::Minelayer
                | First::Grappler
                | First::Spotter
                | First::Dirigible
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
            // Not "to cover": out in the open a roof is most often 60 m off,
            // and a wall or a tree hides no one from above. Stood still a
            // drone has you down in 11-20 s; sprinting across its line it
            // hardly hits (`robots::drone_drill`).
            First::DroneLock => &[
                "Truck: \"Don't stop. Sprint across it. A runner's hard to hit.\"",
                "Truck: \"Only a roof stops it. Walls and trees don't. Keep going.\"",
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
            First::Recall => &["Truck: \"Forgot what that is? Hold it up close and look.\""],
            First::NotePinned => {
                &["Truck: \"Reading their notes? I keep copies. Board in the back.\""]
            }
            First::KitRecovered => &["Truck: \"That's how you get it back. Take it home.\""],
            First::Aimed => &["Truck: \"Break its line, now. It's aiming at you.\""],
            First::Sentry => &["Truck: \"Down, out of its sweep. A sentry has you.\""],
            First::Hush => &["Truck: \"Out of there, now. That dead quiet hurts.\""],
            // As the card stands (`payday`): pressed now, it pays, or
            // not yet, and the card's own reason why. (Home pinned by a
            // drone, the job given up, "that's the job paid" was said, and
            // the card wouldn't pay it.)
            First::Payday => &[PAYDAY_NOT_YET, "{verdict}"],
            First::StashFinds => &[STASH_FINDS],
            First::EmptyClips => &[EMPTY_CLIPS],
            First::ComingToLook => {
                &["Truck: \"Low behind it, and keep still. It only finds what moves.\""]
            }
            First::BrokenNoticed => {
                &["Truck: \"That one's lost its gun. Its eye's still good: shoot that.\""]
            }
            First::Shoved => {
                &["Truck: \"It can still shove. Keep your distance, aim for the eye.\""]
            }
            // (It sees well, and it's slow: low and still is a walker's
            // answer, and two went down to its blows that way.)
            // (And keep going: it hunts on, slow, and 15 m off it caught
            // up again at the job.)
            First::Heavy => {
                &["Truck: \"Big one, and slow. Walk away, and keep going. Don't hide.\""]
            }
            // (It's after movement: run and it runs you down; low and still
            // in its coil and it loses you, `hound::COIL_TIME`.)
            First::Hound => &[
                "Truck: \"Plant your feet and shoot. A hound: don't run, it's after movement.\"",
                "Truck: \"Shoot when it crouches. It's about to jump.\"",
            ],
            // (Stood still for "Something heard you", or pressed on into it,
            // five newcomers were zapped down in a shelter's dorm: back out
            // of its line, or a few rounds and it's down.)
            First::Crawler => {
                &["Truck: \"Back out the way you came. Or put it down: it's charging.\""]
            }
            // (Its mines sit in rings of dark turned earth and blink red
            // within `minelayer::WAKE` of you; seen, it turns tail and drops
            // one behind it. Disarming is a later lesson.)
            First::Minelayer => &[
                "Truck: \"Watch your feet. It sows mines in rings of turned earth.\"",
                "Truck: \"A red blink's one near you. Don't walk in its tracks.\"",
            ],
            // (Its line reaches `grappler::TETHER_RANGE`; it snaps when the
            // grappler's wrecked or stunned, and hauls you to its friends.)
            First::Grappler => {
                &["Truck: \"Keep out of its throw. Speared? Shoot it: the line drops.\""]
            }
            // (A full fix sends a response to where you stood; straight
            // under it, it can't see.)
            First::Spotter => {
                &["Truck: \"Out of its eye, now. It calls the rest. Under it, it's blind.\""]
            }
            // (Unarmed, but it reports what it sees, and grapplers guard its
            // pod.)
            First::Dirigible => {
                &["Truck: \"Under cover. It doesn't shoot, but it sends for those that do.\""]
            }
            First::Picked(f) => f.lines(),
            // (Said as it happens, in your sight: the Zone hurts them too,
            // and that's yours to use, `robots::hazards`.)
            First::Lured => {
                &["Truck: \"Lead them in. Chasing you, they don't look where they run.\""]
            }
            First::Dazzled => {
                &["Truck: \"Lost you in the fold. A mirror blinds them a while. Slip off.\""]
            }
            First::Deafened => {
                &["Truck: \"Take it now. Deaf in that quiet, it won't hear your shots.\""]
            }
            First::Stiffened => {
                &["Truck: \"Get clear while it's in there. Cold's in its joints: it's slow.\""]
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
/// (Its third: each first's line as it was filled, (its template, what's
/// said), for the bots' report, which noted the template, "{s} things",
/// where "Crate's empty" was said.)
pub struct Spoken(
    pub Vec<String>,
    pub Vec<f32>,
    pub Vec<(&'static str, String)>,
);

/// Other systems' moments: write this when one happens (each is said once
/// a career, like the rest). E.g. `MeetFirst(First::NotePinned)` when a
/// note is read.
#[derive(Message, Clone, Copy, Debug)]
pub struct MeetFirst(pub First);

/// Lines waiting their turn on the wrist.
#[derive(Resource, Default)]
struct Queue {
    /// Each line with where it shows besides the wrist (`Raid::tell`).
    lines: VecDeque<(String, Show)>,
    /// Each first's lines as filled, till the speaker passes them on to
    /// `Spoken` (kept here, not in a resource of their own: one more on
    /// every first's system reordered the schedule, and a physics soak
    /// test fell through the world's edge).
    filled: Vec<(&'static str, String)>,
    quiet: f32,
    /// Seconds more that a machine has you, by its first (`HAS_YOU`): no
    /// "go still" said meanwhile.
    has_you: f32,
}

/// A machine that has you, by its first: what it says to do (a roof, run
/// across, walk away) isn't "go still", and the two said together told a
/// newcomer both at once. Heard waits for a machine that's only listening.
const HAS_YOU: [First; 9] = [
    First::Sentry,
    First::DroneLock,
    First::Aimed,
    First::Heavy,
    First::Hound,
    First::Crawler,
    First::Grappler,
    First::Spotter,
    First::Dirigible,
];

/// How long a machine's having you holds "go still" back.
const HAS_YOU_HOLD: f32 = 10.0;

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
                        heavy_raised,
                        machines_on_you,
                        lamed,
                        pack_full,
                        places,
                        worn,
                        anomaly_kit,
                        zone_on_machines,
                        deaths,
                        convoy,
                        asked,
                        payday,
                        picked_up,
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
    show: Option<Show>,
) -> bool {
    if f == First::Heard && queue.has_you > 0.0 {
        return false;
    }
    if !firsts.0.insert(f.key()) {
        return false;
    }
    if HAS_YOU.contains(&f) {
        queue.has_you = HAS_YOU_HOLD;
        // Heard in the same breath, and not said yet: not now, and still
        // to learn.
        let heard: Vec<String> = First::Heard.lines().iter().map(|l| l.to_string()).collect();
        let before = queue.lines.len();
        queue.lines.retain(|(l, _)| !heard.contains(l));
        if queue.lines.len() < before {
            firsts.0.remove(&First::Heard.key());
        }
    }
    for l in f.lines() {
        queue.filled.push((l, fill(l)));
    }
    info!("first time: {f:?}");
    let show = show.unwrap_or(f.show());
    if f.urgent() {
        // Danger now: said at once, ahead of anything waiting (a walker's
        // aim came the same moment as "heard", and five seconds behind it
        // was after the shot).
        for l in f.lines().iter().rev().map(|l| fill(l)) {
            if !l.is_empty() {
                queue.lines.push_front((l, show));
            }
        }
        let since = SPACING - queue.quiet;
        queue.quiet = queue.quiet.min((URGENT_SPACING - since).max(0.0));
    } else {
        // (A line filled to nothing isn't said.)
        queue.lines.extend(
            f.lines()
                .iter()
                .map(|l| fill(l))
                .filter(|l| !l.is_empty())
                .map(|l| (l, show)),
        );
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
        meet(
            f,
            &mut self.firsts,
            &mut self.queue,
            &mut self.out,
            &|l| l.to_string(),
            None,
        )
    }

    /// As `meet`, its lines shown beside `item` (in the view if it's not
    /// in a hand by then).
    fn meet_beside(&mut self, f: First, item: Entity) -> bool {
        meet(
            f,
            &mut self.firsts,
            &mut self.queue,
            &mut self.out,
            &|l| l.to_string(),
            Some(Show::Item(item)),
        )
    }

    fn meet_filled(&mut self, f: First, fill: &dyn Fn(&str) -> String) -> bool {
        meet(
            f,
            &mut self.firsts,
            &mut self.queue,
            &mut self.out,
            fill,
            None,
        )
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
    broken: Query<(), With<crate::robots::broken::Broken>>,
    hounds: Query<(), With<crate::robots::hound::Hound>>,
    mut shoves: MessageReader<crate::robots::broken::Shoved>,
    locals: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    mut m: Meet,
) {
    let Some((me, at)) = locals.iter().next() else {
        return;
    };
    let about_me = |robot: Entity| minds.get(robot).is_ok_and(|a| a.target == Some(me));
    if shoves.read().any(|s| s.player == me) {
        m.meet(First::Shoved);
    }
    let looked: Vec<Entity> = looking.read().map(|e| e.robot).collect();
    // One that was shooting at you, coming to look (or back past you).
    if back.read().any(|b| b.about == me) {
        m.meet(First::ComingToLook);
    }
    let robots: Vec<Entity> = looked
        .into_iter()
        .chain(noticed.read().map(|e| e.robot))
        .collect();
    // A broken walker's is its own line (shoot its eye), not "go still":
    // it's the one to take on.
    let (wrecks, others): (Vec<Entity>, Vec<Entity>) = robots
        .into_iter()
        .filter(|r| about_me(*r))
        .partition(|r| broken.contains(*r));
    if !wrecks.is_empty() {
        m.meet(First::BrokenNoticed);
    }
    // A hound's is its own too: "go still" is right, "move off" isn't
    // (it's after movement).
    let (hounds, others): (Vec<Entity>, Vec<Entity>) =
        others.into_iter().partition(|r| hounds.contains(*r));
    if !hounds.is_empty() {
        m.meet(First::Hound);
    }
    if !others.is_empty() {
        m.meet(First::Heard);
    }
    for ev in lost.read() {
        // Only once it's been after you, and somewhere near.
        if m.met(First::Heard) && ev.at.distance(at.translation()) < 80.0 {
            m.meet(First::LostYou);
        }
    }
}

/// How far off, and how far off your look, a machine the Zone strikes is
/// seen happen (m, degrees).
const ZONE_SEEN: f32 = 45.0;
const ZONE_SEEN_DEG: f32 = 55.0;

/// The Zone doing something to a machine, as you see it happen: the lure
/// (one after you ran into a burner, a snare, a lift or a well), a fold's
/// dazzle, a hush's deafness, a frost's stiffness. The machine shows it
/// itself (`robots::hazards`' tells); the line says it's yours to use.
fn zone_on_machines(
    mut struck: MessageReader<crate::robots::hazards::HazardStruck>,
    minds: Query<&Awareness>,
    heads: Query<(&GlobalTransform, &ChildOf), With<crate::rig::RigHead>>,
    locals: Query<Entity, With<LocalPlayer>>,
    mut m: Meet,
) {
    let Some(me) = locals.iter().next() else {
        struck.clear();
        return;
    };
    let Some(eye) = heads
        .iter()
        .find(|(_, c)| c.parent() == me)
        .map(|(g, _)| *g)
    else {
        struck.clear();
        return;
    };
    for ev in struck.read() {
        if !seen_happen(eye.translation(), eye.forward().as_vec3(), ev.at) {
            continue;
        }
        let after_me = minds.get(ev.robot).is_ok_and(|a| a.target == Some(me));
        if let Some(f) = zone_first(ev.hazard, ev.effect, after_me) {
            m.meet(f);
        }
    }
}

/// Whether something at `at` is seen happen from an eye at `eye` looking
/// along `look`: near enough, and before you.
fn seen_happen(eye: Vec3, look: Vec3, at: Vec3) -> bool {
    let to = at - eye;
    to.length() < ZONE_SEEN && to.normalize_or_zero().dot(look) > ZONE_SEEN_DEG.to_radians().cos()
}

/// What seeing a hazard do this to a machine teaches (`after_me`: it was
/// after you).
fn zone_first(
    hazard: crate::robots::hazards::Hazard,
    effect: crate::robots::hazards::Effect,
    after_me: bool,
) -> Option<First> {
    use crate::robots::hazards::{Effect, Hazard};
    Some(match (hazard, effect) {
        (_, Effect::Unaffected) => return None,
        (Hazard::Burner | Hazard::Snare | Hazard::Lift | Hazard::Well, _) if after_me => {
            First::Lured
        }
        (Hazard::Mirror, Effect::Dazzled) => First::Dazzled,
        (Hazard::Hush, Effect::Deafened | Effect::Dazzled) => First::Deafened,
        (Hazard::Frost, _) => First::Stiffened,
        _ => return None,
    })
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

/// A heavy raising its arm at you, close; a hound coiling at you; a
/// crawler charging; and the unarmed ones and the grappler, each on what
/// tells a player it's there (`logistics_on_you`).
#[allow(clippy::too_many_arguments)]
fn heavy_raised(
    mut events: MessageReader<crate::robots::heavy::HeavyEvent>,
    mut crouches: MessageReader<crate::robots::hound::HoundCrouch>,
    mut charges: MessageReader<crate::robots::indoor::CrawlerCharge>,
    mut harpoons: MessageReader<crate::robots::grappler::Harpoon>,
    mut spotters: MessageReader<crate::robots::spotter::SpotterEvent>,
    minds: Query<&Awareness>,
    watchers: Query<
        (&Awareness, Has<crate::robots::minelayer::Minelayer>),
        Or<(
            With<crate::robots::minelayer::Minelayer>,
            With<crate::robots::dirigible::Dirigible>,
        )>,
    >,
    mines: Query<&Transform, With<crate::robots::minelayer::LaidMine>>,
    pods: Query<&GlobalTransform, With<crate::robots::dirigible::CargoPod>>,
    locals: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    mut m: Meet,
) {
    let Some((me, at)) = locals.iter().next() else {
        return;
    };
    let at = at.translation();
    for f in logistics_on_you(
        me,
        at,
        harpoons.read().map(|h| h.to),
        spotters
            .read()
            .filter(|s| {
                s.what == crate::robots::spotter::SpotterAct::Fixing
                    && minds.get(s.spotter).is_ok_and(|a| a.target == Some(me))
            })
            .count(),
        watchers
            .iter()
            .map(|(a, layer)| (a.target == Some(me) && a.can_see_target, layer)),
        mines.iter().map(|t| t.translation),
        pods.iter().map(|p| p.translation()),
    ) {
        m.meet(f);
    }
    for ev in events.read() {
        if ev.what == crate::robots::heavy::HeavyAct::Raise
            && minds.get(ev.heavy).is_ok_and(|a| a.target == Some(me))
        {
            m.meet(First::Heavy);
        }
    }
    // A hound coiling to spring at you.
    for ev in crouches.read() {
        if minds.get(ev.hound).is_ok_and(|a| a.target == Some(me)) {
            m.meet(First::Hound);
        }
    }
    // A crawler's spark gun charging at you.
    for ev in charges.read() {
        if minds.get(ev.crawler).is_ok_and(|a| a.target == Some(me)) {
            m.meet(First::Crawler);
        }
    }
}

/// How near a lowered cargo pod counts as at it: its grapplers' claim on
/// the ground round it (`grappler::CLAIM`).
const POD_NEAR: f32 = crate::robots::grappler::CLAIM;

/// The minelayer's, the grappler's, the spotter's and the dirigible's
/// firsts, from what's happening to you: a harpoon thrown at you; a
/// spotter's eye fixing on you; a minelayer or a dirigible that sees you
/// (`(sees you, is a minelayer)`); a mine blinking near you (within
/// `minelayer::WAKE`); a dirigible's pod lowered near you.
fn logistics_on_you(
    me: Entity,
    at: Vec3,
    harpooned: impl Iterator<Item = Entity>,
    fixing: usize,
    watching: impl Iterator<Item = (bool, bool)>,
    mines: impl Iterator<Item = Vec3>,
    pods: impl Iterator<Item = Vec3>,
) -> Vec<First> {
    let flat = |p: Vec3| p.xz().distance(at.xz());
    let mut out = vec![];
    let (mut layer, mut ship) = (false, false);
    for (sees, is_layer) in watching {
        if sees {
            if is_layer {
                layer = true;
            } else {
                ship = true;
            }
        }
    }
    if layer
        || mines
            .into_iter()
            .any(|p| flat(p) <= extraction_core::minelayer::WAKE)
    {
        out.push(First::Minelayer);
    }
    if harpooned.into_iter().any(|to| to == me) {
        out.push(First::Grappler);
    }
    if fixing > 0 {
        out.push(First::Spotter);
    }
    if ship || pods.into_iter().any(|p| flat(p) <= POD_NEAR) {
        out.push(First::Dirigible);
    }
    out
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
    // (Said of what's so: none fell, or none's in the crate, is its own
    // line, not "0 things".)
    if fell == 0 && line.contains("{n} things") {
        return "Truck: \"You'd nothing on you to lose.\"".into();
    }
    if in_crate == 0 && line.contains("{s} things") {
        return line.replace(
            "Crate's as you left it: {s} things.",
            "Crate's empty, as you left it.",
        );
    }
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
/// the driver's goes in the kit crate, or there'd be no calling him on the
/// next raid.
pub const SPARE_RADIO: &str = "Truck: \"No radio on you. My spare's in the crate.\"";
/// Home with no gun on you, in the crate or on the ledge (a fall, and the
/// convoy took the lot): the driver's own, a magazine and a box of its
/// rounds, in the kit crate. (Gone out unarmed after a lost kit, the
/// hoarder died two minutes in, four raids running; handed through the
/// hatch, they came after a quick newcomer had left the truck.)
pub const SPARE_GUN: &str = "Truck: \"No gun on you. Here, my spare. Don't lose this one.\"";

/// The same for what a job needs taken out (a thing to leave, the camera):
/// lost at a fall, the job couldn't be done.
pub fn spare_for(name: &str) -> String {
    format!("Truck: \"Lost the {name}? There's another in the crate.\"")
}

/// Off the truck again with finds in the pack (Story's wording). (A
/// hoarder carried the lot out every raid, and lost it at the third.)
pub const STASH_FINDS: &str =
    "Driver: \"Stash what you found. What's on you is gone if you fall.\"";

/// Home with an empty stripper clip and a box of its rounds (Progression's
/// wording: they fill by hand from the box, as the yard's magazines do).
pub const EMPTY_CLIPS: &str = "Driver: \"Empty clips. Strip a box into them before you go.\"";

/// Whether, of what's to hand (each thing's catalog entry and the rounds
/// in it), a stripper clip is empty and a box of its cartridge isn't.
fn empty_clip_to_fill<'a>(
    to_hand: impl Iterator<Item = (&'a extraction_core::items::ItemDef, u32)> + Clone,
) -> bool {
    to_hand.clone().any(|(d, n)| {
        d.magazine.as_ref().is_some_and(|m| {
            m.stripper
                && n == 0
                && to_hand.clone().any(|(a, r)| {
                    a.category == extraction_core::items::Category::Ammo
                        && r > 0
                        && a.cartridge.as_deref() == Some(m.cartridge.as_str())
                })
        })
    })
}

/// What counts as a find to stash: what's brought home for its worth, not
/// the kit taken out.
fn a_find(c: extraction_core::items::Category) -> bool {
    use extraction_core::items::Category::*;
    matches!(c, Material | RobotPart | Artifact | Curio)
}

/// Seconds at the truck before the driver's spares are in the crate (what
/// you have is loaded by then: a career's save, a homecoming's hand-off).
const SPARE_AFTER: f32 = 0.5;

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
    opts: Res<crate::launch::LaunchOptions>,
    mut stash: Query<
        (Entity, &mut crate::interact::body::Backpack),
        With<crate::raid::truck::Stash>,
    >,
    trucks: Query<&GlobalTransform, With<crate::raid::truck::Truck>>,
    locals: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    hatch: Res<crate::raid::lev::Handovers>,
    mut commands: Commands,
    mut m: Meet,
    mut home_for: Local<f32>,
    mut handed: Local<bool>,
    mut from_a_raid: Local<bool>,
) {
    if raid.phase != Phase::AtBase {
        *home_for = 0.0;
        *handed = false;
        *from_a_raid = true;
        return;
    }
    *home_for += time.delta_secs();
    // Home from a raid and off again to the truck's tail, finds still on
    // you: stash them (once a career).
    if *from_a_raid
        && !m.met(First::StashFinds)
        && let (Some((me, feet)), Some(truck)) = (locals.iter().next(), trucks.iter().next())
    {
        let l = truck
            .affine()
            .inverse()
            .transform_point3(feet.translation());
        let at_tail = l.z > 1.3 && l.x.abs() < 1.3;
        let stashed = |e: Entity| stash.iter().any(|(_, b)| b.contents.contains(&e));
        let finds = items.iter().any(|(e, it, by, _)| {
            by.is_some_and(|b| b.0 == me) && a_find(catalog.get(it.kind()).category) && !stashed(e)
        });
        if at_tail && finds {
            m.meet(First::StashFinds);
        }
    }
    // An empty stripper clip on you or in the crate, a box of its rounds
    // with it: fill them now (once a career).
    if !m.met(First::EmptyClips)
        && *home_for >= SPARE_AFTER
        && let Some((me, _)) = locals.iter().next()
    {
        let crate_has = |e: Entity| stash.iter().any(|(_, b)| b.contents.contains(&e));
        let to_hand = items
            .iter()
            .filter(|(e, _, by, _)| by.is_some_and(|b| b.0 == me) || crate_has(*e))
            .map(|(_, it, ..)| (catalog.get(it.kind()), it.rounds));
        if empty_clip_to_fill(to_hand) {
            m.meet(First::EmptyClips);
        }
    }
    // (What's on its way through the hatch already counts, once it's down.)
    if *handed || *home_for < SPARE_AFTER || hatch.busy() {
        return;
    }
    let (Some((me, _)), Some(truck)) = (locals.iter().next(), trucks.iter().next()) else {
        return;
    };
    *handed = true;
    let in_crate = |e: Entity| stash.iter().any(|(_, b)| b.contents.contains(&e));
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
    // Into the kit crate, where the kit's loaded from before a raid (on
    // the ledge, the newcomer was out of the truck before it came).
    let mut give: Vec<(String, f32)> = vec![];
    if !have("radio") {
        give.push(("radio".into(), 0.8));
        m.queue.lines.push_back((SPARE_RADIO.into(), Show::View));
    }
    // No gun anywhere of yours (in the yard, the kit is the yard's own).
    let armed = items.iter().any(|(e, it, by, gt)| {
        catalog.get(it.kind()).firearm.is_some()
            && (by.is_some_and(|b| b.0 == me)
                || in_crate(e)
                || gt.translation().distance(truck.translation()) < 3.0)
    });
    if !armed && opts.map != crate::camp::TRAINING_YARD {
        // (Worn as the kit's are.)
        for (id, condition) in [("pm_pistol", 0.75), ("pm_mag", 0.75), ("ammo_9x18", 1.0)] {
            give.push((id.into(), condition));
        }
        m.queue.lines.push_back((SPARE_GUN.into(), Show::View));
    }
    if let Some(id) = wanted.filter(|id| !have(id)) {
        let name = catalog
            .kind(&id)
            .map(|k| catalog.get(k).name.to_lowercase())
            .unwrap_or_else(|| id.replace('_', " "));
        give.push((id, 1.0));
        m.queue.lines.push_back((spare_for(&name), Show::View));
    }
    let Some((crate_e, mut bag)) = stash.iter_mut().next() else {
        return;
    };
    let n = give.len() as f32;
    for (i, (id, condition)) in give.into_iter().enumerate() {
        let Some(kind) = catalog.kind(&id) else {
            warn!("the driver has no {id} to spare");
            continue;
        };
        let e = crate::items::spawn_item(
            &mut commands,
            &catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::default(),
                condition,
            },
        );
        // His magazine comes loaded, as the driver's gifts do (empty, with a
        // box beside it, the newcomer went out with an empty pistol and
        // was shot down by a drone, firing nothing, raid after raid). The
        // box is to fill it again.
        if let Some(cap) = catalog.get(kind).magazine.as_ref().map(|m| m.capacity) {
            commands.entity(e).queue(move |mut it: EntityWorldMut| {
                if let Some(mut item) = it.get_mut::<crate::items::Item>() {
                    item.rounds = cap;
                    item.loads.fit(cap);
                }
            });
        }
        // Side by side along the crate's shelf.
        let x = bag.half.x * 0.8 * ((i as f32 + 0.5) / n.max(1.0) * 2.0 - 1.0);
        crate::interact::body::stow_in_backpack(
            &mut commands,
            e,
            crate_e,
            &mut bag,
            Vec3::new(x, 0.05, 0.0),
        );
    }
}

/// Home, the card as it stands paying the job if pressed now.
pub const PAYDAY_PAID: &str = "Truck: \"Home. Press the card: that's the job paid.\"";
/// ...not yet (the card's reason follows: in the pack, not the stash; or
/// not done).
pub const PAYDAY_NOT_YET: &str = "Truck: \"Home. Press the card once the job's done: it pays it.\"";

/// Home from the Zone with a job on the card: nothing pays it but
/// pressing the card (the first newcomers took the truck home, and the
/// job sat unpaid into the next session).
fn payday(
    raid: Res<Raid>,
    progression: Option<Res<crate::progression::Progression>>,
    opts: Res<crate::launch::LaunchOptions>,
    check: crate::raid::requests::CardCheck,
    mut was_out: Local<bool>,
    mut m: Meet,
) {
    let home = raid.phase == Phase::AtBase;
    let back = *was_out && home;
    *was_out = matches!(raid.phase, Phase::Extracting { .. });
    let Some(p) = progression.filter(|p| p.0.job.is_some() && p.0.last_raid != Some(raid.raids))
    else {
        return;
    };
    if !back || m.met(First::Payday) {
        return;
    }
    // What the card would say, pressed now: it pays; or not yet, and its
    // own reason (each names the stash, so "in your pack" and "not done"
    // read apart). Nothing to pay this raid at all, or no stash: not now
    // (said on a later homecoming).
    use crate::progression::lines;
    let verdict = check.verdict(&p.0, &raid, &opts);
    if let Err(why) = &verdict
        && (why.is_empty()
            || [
                lines::NO_JOB,
                lines::NOT_AT_BASE,
                lines::ONE_A_RAID,
                lines::IN_THE_YARD,
            ]
            .contains(&why.as_str()))
    {
        return;
    }
    m.meet_filled(First::Payday, &|l| match (&verdict, l) {
        (Ok(_), PAYDAY_NOT_YET) => PAYDAY_PAID.into(),
        (Ok(_), _) => String::new(),
        (Err(why), "{verdict}") => why.clone(),
        (Err(_), l) => l.into(),
    });
}

/// At most this many "what it's for" lines a raid (the rest wait for a
/// later one, picked up again).
pub const PICKED_A_RAID: usize = 3;

/// The first of a family in your hand out there (`pickups`): its line,
/// once a career, when it's calm (nothing waiting to be said, no machine
/// after you), and no more than [`PICKED_A_RAID`] a raid.
#[allow(clippy::too_many_arguments)]
fn picked_up(
    held: Query<(Entity, &crate::items::Item, &Held)>,
    locals: Query<Entity, With<LocalPlayer>>,
    minds: Query<&Awareness>,
    catalog: Res<crate::items::Catalog>,
    raid: Res<Raid>,
    mut waiting: Local<Vec<(Family, Entity)>>,
    mut said: Local<usize>,
    mut m: Meet,
) {
    if matches!(
        raid.phase,
        Phase::AtBase | Phase::Extracting { .. } | Phase::Down { .. }
    ) {
        waiting.clear();
        if raid.phase == Phase::AtBase {
            *said = 0;
        }
        return;
    }
    let Some(me) = locals.iter().next() else {
        return;
    };
    for (e, item, h) in &held {
        if h.player != me {
            continue;
        }
        let def = catalog.get(item.kind());
        if let Some(f) = super::pickups::family(def)
            && !m.met(First::Picked(f))
            && !waiting.iter().any(|(w, _)| *w == f)
        {
            waiting.push((f, e));
        }
    }
    let calm = m.queue.lines.is_empty()
        && m.queue.quiet <= 0.0
        && m.queue.has_you <= 0.0
        // (One that has noticed you, not any that tracks you: a glimpse or a
        // sound sets its target till its search gives up, which out there
        // is most of a raid, and the first jobs' finds went unnamed.)
        && !minds
            .iter()
            .any(|a| a.target == Some(me) && a.suspicion >= crate::robots::NOTICED);
    if calm && *said < PICKED_A_RAID && !waiting.is_empty() {
        let (f, e) = waiting.remove(0);
        if m.meet_beside(First::Picked(f), e) {
            *said += 1;
        }
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
    let filled: Vec<_> = queue.filled.drain(..).collect();
    spoken.2.extend(filled);
    queue.has_you -= time.delta_secs();
    queue.quiet -= time.delta_secs();
    if queue.quiet > 0.0 {
        return;
    }
    if let Some((line, show)) = queue.lines.pop_front() {
        spoken.0.push(line.clone());
        spoken.1.push(time.elapsed_secs());
        raid.tell(line, show);
        queue.quiet = SPACING;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Go still" isn't said while a machine has you, either way round
    /// (a sentry's chirp and its noticing you came in one frame: "Down, out
    /// of its sweep" and "go still" at once). Still to learn, for later.
    #[test]
    fn go_still_isnt_said_while_a_machine_has_you() {
        use bevy::ecs::system::SystemState;
        for order in [[First::Heard, First::Sentry], [First::Sentry, First::Heard]] {
            let mut app = App::new();
            app.init_resource::<Firsts>()
                .init_resource::<Queue>()
                .add_message::<FirstMet>();
            let mut st: SystemState<Meet> = SystemState::new(app.world_mut());
            {
                let mut m = st.get_mut(app.world_mut()).unwrap();
                for f in order {
                    m.meet(f);
                }
            }
            st.apply(app.world_mut());
            let q = app.world().resource::<Queue>();
            let says = |f: First| q.lines.iter().any(|(l, _)| f.lines().contains(&l.as_str()));
            assert!(!says(First::Heard), "{order:?}: {:?}", q.lines);
            assert!(says(First::Sentry), "{order:?}: {:?}", q.lines);
            assert!(
                !app.world().resource::<Firsts>().met(First::Heard),
                "{order:?}"
            );
        }
    }

    /// A first's lines are queued for the view, not the wrist alone (#90,
    /// `render::said`; `speak` tells each as queued): danger now as
    /// urgent, the rest as captions, and a find's line beside the thing in
    /// your hand.
    #[test]
    fn a_firsts_lines_are_told_for_the_view() {
        use bevy::ecs::system::SystemState;
        let mut app = App::new();
        app.init_resource::<Firsts>()
            .init_resource::<Queue>()
            .add_message::<FirstMet>();
        let thing = app.world_mut().spawn_empty().id();
        let mut st: SystemState<Meet> = SystemState::new(app.world_mut());
        {
            let mut m = st.get_mut(app.world_mut()).unwrap();
            assert!(m.meet(First::Surge));
            assert!(m.meet(First::Detector));
            assert!(m.meet_beside(First::Picked(Family::GunCare), thing));
        }
        st.apply(app.world_mut());
        let q = app.world().resource::<Queue>();
        let at = |f: First| {
            let l = f.lines()[0];
            q.lines
                .iter()
                .find(|(t, _)| t == l)
                .map(|(_, s)| *s)
                .unwrap_or_else(|| panic!("{f:?} not queued: {:?}", q.lines))
        };
        assert_eq!(at(First::Surge), Show::Urgent);
        assert_eq!(at(First::Detector), Show::View);
        assert_eq!(at(First::Picked(Family::GunCare)), Show::Item(thing));
    }

    /// A hound that's noticed you is the hound's line ("Don't run"), not
    /// "Move off quiet": moving is what it's after. A walker's is still
    /// "Something heard you".
    #[test]
    fn a_hound_noticing_you_is_told_as_a_hound() {
        for hound in [true, false] {
            let mut app = App::new();
            app.init_resource::<Firsts>()
                .init_resource::<Queue>()
                .add_message::<FirstMet>()
                .add_message::<crate::robots::search::Investigating>()
                .add_message::<crate::robots::Noticed>()
                .add_message::<crate::robots::search::LostYou>()
                .add_message::<crate::robots::search::ComingBack>()
                .add_message::<crate::robots::broken::Shoved>()
                .add_systems(Update, robots_heard);
            let me = app
                .world_mut()
                .spawn((LocalPlayer, GlobalTransform::default()))
                .id();
            let mut robot = app.world_mut().spawn(Awareness {
                target: Some(me),
                ..default()
            });
            if hound {
                robot.insert(crate::robots::hound::Hound);
            }
            let robot = robot.id();
            app.world_mut().write_message(crate::robots::Noticed {
                robot,
                at: Vec3::ZERO,
            });
            app.update();
            let met = app.world().resource::<Firsts>();
            assert_eq!(met.met(First::Hound), hound, "hound {hound}");
            assert_eq!(met.met(First::Heard), !hound, "hound {hound}");
        }
    }

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
                    let page = flat(&wrist_text("SURGE 12M", 9.0, 90.0, Some(said), 0));
                    assert!(
                        page.contains(&act),
                        "{f:?}, {how}: \"{act}\" not on page one:\n{page}"
                    );
                }
            }
        }
    }

    /// What the driver says the first time each machine is on you, by its
    /// `spawn_*` in `robots/`: its own first, or why it has none (as
    /// `robots::intent_roster` does for its voice), so the next machine
    /// can't come in with nothing said about it.
    const FIRST_CONTACT: &[(&str, Result<First, &str>)] = &[
        ("walker", Ok(First::Aimed)),
        ("walker_with", Ok(First::Aimed)),
        ("walker_sized", Ok(First::Aimed)),
        (
            "walker_shaped",
            Err("a walker in another body: the grappler's and the crawler's, named under theirs"),
        ),
        ("broken_walker", Ok(First::BrokenNoticed)),
        ("drone", Ok(First::DroneLock)),
        ("hound", Ok(First::Hound)),
        ("heavy", Ok(First::Heavy)),
        ("sentry", Ok(First::Sentry)),
        ("crawler", Ok(First::Crawler)),
        ("warden", Ok(First::Underground)),
        ("grappler", Ok(First::Grappler)),
        ("minelayer", Ok(First::Minelayer)),
        ("spotter", Ok(First::Spotter)),
        ("dirigible", Ok(First::Dirigible)),
        ("mender", Err("never acts on you: it mends machines")),
        ("plough", Err("never looks for you: it drives its lane")),
        ("pylon", Err("a relay mast: nothing toward you")),
        ("sorter", Err("the Depot's gantry: its own alarm says it")),
        ("debris", Err("a wreck's pieces, not a machine")),
    ];

    /// Every machine has its first-contact line, or a reason it has none;
    /// and each line named is a first that's said.
    #[test]
    fn every_machine_has_its_first_contact_line() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/robots");
        let mut found = vec![];
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let text = std::fs::read_to_string(e.path()).unwrap();
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("pub fn spawn_") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    found.push(name);
                }
            }
        }
        found.sort();
        found.dedup();
        let missing: Vec<&String> = found
            .iter()
            .filter(|f| !FIRST_CONTACT.iter().any(|(r, _)| r == f))
            .collect();
        assert!(
            missing.is_empty(),
            "machines with no first-contact line in FIRST_CONTACT (give it one, or a \
             reason): {missing:?}"
        );
        for (name, first) in FIRST_CONTACT {
            if let Ok(f) = first {
                assert!(ALL.contains(f), "{name}: {f:?} isn't in ALL");
                assert!(f.urgent(), "{name}: {f:?} isn't said first-thing");
            }
        }
        // The hound's, as the driver says it: stand, and shoot.
        let hound = First::Hound.lines().join(" ");
        assert!(
            hound.contains("don't run") && hound.contains("shoot"),
            "{hound}"
        );
    }

    /// Each of the four is told on what tells a player it's there, and
    /// not on what doesn't: a harpoon at someone else, a mine a step past
    /// its blink, a pod off past its grapplers' claim, a minelayer or a
    /// dirigible looking at nothing.
    #[test]
    fn the_unarmed_ones_and_the_grappler_are_told_on_their_tell() {
        let me = Entity::from_raw_u32(7).unwrap();
        let other = Entity::from_raw_u32(8).unwrap();
        let at = Vec3::new(10.0, 2.0, -4.0);
        let wake = extraction_core::minelayer::WAKE;
        let told = |harpoon: &[Entity],
                    fixing: usize,
                    watching: &[(bool, bool)],
                    mines: &[Vec3],
                    pods: &[Vec3]| {
            logistics_on_you(
                me,
                at,
                harpoon.iter().copied(),
                fixing,
                watching.iter().copied(),
                mines.iter().copied(),
                pods.iter().copied(),
            )
        };
        assert!(told(&[], 0, &[], &[], &[]).is_empty());
        // A grappler: its harpoon at you, hit or miss; not at another.
        assert_eq!(told(&[me], 0, &[], &[], &[]), [First::Grappler]);
        assert!(told(&[other], 0, &[], &[], &[]).is_empty());
        // A spotter fixing on you.
        assert_eq!(told(&[], 1, &[], &[], &[]), [First::Spotter]);
        // A minelayer that sees you; or a mine near enough to blink
        // (height aside: a mine on a bank below is as near).
        assert_eq!(told(&[], 0, &[(true, true)], &[], &[]), [First::Minelayer]);
        assert!(told(&[], 0, &[(false, true)], &[], &[]).is_empty());
        let near = at + Vec3::new(wake - 0.2, -1.5, 0.0);
        let far = at + Vec3::new(wake + 0.5, 0.0, 0.0);
        assert_eq!(told(&[], 0, &[], &[near], &[]), [First::Minelayer]);
        assert!(told(&[], 0, &[], &[far], &[]).is_empty());
        // A dirigible that sees you, or its pod lowered near you.
        assert_eq!(told(&[], 0, &[(true, false)], &[], &[]), [First::Dirigible]);
        assert!(told(&[], 0, &[(false, false)], &[], &[]).is_empty());
        let pod = at + Vec3::new(0.0, 0.0, POD_NEAR - 1.0);
        let pod_far = at + Vec3::new(0.0, 0.0, POD_NEAR + 1.0);
        assert_eq!(told(&[], 0, &[], &[], &[pod]), [First::Dirigible]);
        assert!(told(&[], 0, &[], &[], &[pod_far]).is_empty());
    }

    /// In the game, each is met through the firsts' own system, from its
    /// robot's own message or component; and each holds "go still" back
    /// but the minelayer's (nothing's after you: watch where you step).
    #[test]
    fn each_is_met_in_the_game_and_holds_go_still_back() {
        use crate::robots::{dirigible, grappler, minelayer, spotter};
        for f in [
            First::Grappler,
            First::Spotter,
            First::Minelayer,
            First::Dirigible,
        ] {
            let mut app = App::new();
            app.init_resource::<Firsts>()
                .init_resource::<Queue>()
                .add_message::<FirstMet>()
                .add_message::<crate::robots::heavy::HeavyEvent>()
                .add_message::<crate::robots::hound::HoundCrouch>()
                .add_message::<crate::robots::indoor::CrawlerCharge>()
                .add_message::<grappler::Harpoon>()
                .add_message::<spotter::SpotterEvent>()
                .add_systems(Update, heavy_raised);
            let me = app
                .world_mut()
                .spawn((LocalPlayer, GlobalTransform::default()))
                .id();
            let seeing = Awareness {
                target: Some(me),
                can_see_target: true,
                ..default()
            };
            match f {
                First::Grappler => {
                    let from = app.world_mut().spawn_empty().id();
                    app.world_mut().write_message(grappler::Harpoon {
                        from,
                        to: me,
                        hit: false,
                    });
                }
                First::Spotter => {
                    let spotter = app.world_mut().spawn(seeing).id();
                    app.world_mut().write_message(spotter::SpotterEvent {
                        spotter,
                        at: Vec3::Y * 30.0,
                        what: spotter::SpotterAct::Fixing,
                    });
                }
                First::Minelayer => {
                    app.world_mut().spawn((
                        minelayer::LaidMine {
                            mine: Default::default(),
                            disarm: Default::default(),
                            phase: 0.0,
                        },
                        Transform::from_xyz(3.0, 0.0, 2.0),
                    ));
                }
                _ => {
                    let carrier = app.world_mut().spawn_empty().id();
                    app.world_mut().spawn((
                        dirigible::CargoPod { carrier },
                        GlobalTransform::from_translation(Vec3::X * 5.0),
                    ));
                }
            }
            app.update();
            let met = app.world().resource::<Firsts>();
            assert!(met.met(f), "{f:?} not met");
            let held = app.world().resource::<Queue>().has_you > 0.0;
            assert_eq!(held, f != First::Minelayer, "{f:?}: go still held {held}");
        }
    }

    /// The Zone on a machine is told as you see it: before you and near,
    /// and the lure only for one that was after you.
    #[test]
    fn the_zone_on_a_machine_is_told_as_you_see_it() {
        use crate::robots::hazards::{Effect, Hazard};
        let eye = Vec3::new(0.0, 1.7, 0.0);
        assert!(seen_happen(eye, Vec3::NEG_Z, Vec3::new(3.0, 1.0, -20.0)));
        assert!(
            !seen_happen(eye, Vec3::NEG_Z, Vec3::new(0.0, 1.0, 20.0)),
            "behind you"
        );
        assert!(
            !seen_happen(eye, Vec3::NEG_Z, Vec3::new(0.0, 1.0, -60.0)),
            "too far"
        );
        assert_eq!(
            zone_first(Hazard::Snare, Effect::Hurt, true),
            Some(First::Lured)
        );
        assert_eq!(
            zone_first(Hazard::Burner, Effect::Hurt, false),
            None,
            "not after you"
        );
        assert_eq!(
            zone_first(Hazard::Mirror, Effect::Dazzled, false),
            Some(First::Dazzled)
        );
        assert_eq!(
            zone_first(Hazard::Hush, Effect::Deafened, false),
            Some(First::Deafened)
        );
        assert_eq!(
            zone_first(Hazard::Frost, Effect::Slowed, true),
            Some(First::Stiffened)
        );
        assert_eq!(zone_first(Hazard::Mirror, Effect::Unaffected, true), None);
        assert_eq!(zone_first(Hazard::Gas, Effect::Hurt, true), None);
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

    /// "Empty clips" is for an empty stripper clip with a box of its own
    /// rounds to hand: not a full one, not a box of another cartridge, not
    /// an empty box, not a box magazine.
    #[test]
    fn empty_clips_want_a_box_of_their_rounds() {
        let cat =
            extraction_core::items::ItemCatalog::from_ron(extraction_core::items::BUILTIN_CATALOG)
                .unwrap();
        let d = |id: &str| cat.get(cat.expect(id));
        let fires = |have: &[(&str, u32)]| {
            let v: Vec<_> = have.iter().map(|(id, n)| (d(id), *n)).collect();
            empty_clip_to_fill(v.iter().map(|(d, n)| (*d, *n)))
        };
        assert!(fires(&[("sks_clip", 0), ("ammo_762x39", 20)]));
        assert!(fires(&[
            ("mosin_clip", 0),
            ("mosin_clip", 5),
            ("ammo_762r", 3)
        ]));
        assert!(
            !fires(&[("sks_clip", 10), ("ammo_762x39", 20)]),
            "a full clip"
        );
        assert!(
            !fires(&[("sks_clip", 0), ("ammo_545", 30)]),
            "another cartridge"
        );
        assert!(
            !fires(&[("sks_clip", 0), ("ammo_762x39", 0)]),
            "an empty box"
        );
        assert!(!fires(&[("sks_clip", 0)]), "no box");
        assert!(
            !fires(&[("pm_mag", 0), ("ammo_9x18", 16)]),
            "a box magazine"
        );
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

    /// Home from a raid, off to the truck's tail with a find in the pack:
    /// "Stash what you found", once. Not with nothing found on you, nor at
    /// the tail before a raid, nor a second time.
    #[test]
    fn finds_in_the_pack_off_the_tail_get_stash_them_once() {
        let said = |app: &App| {
            app.world()
                .resource::<Spoken>()
                .0
                .iter()
                .filter(|l| *l == STASH_FINDS)
                .count()
        };
        let mut app = app();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let me = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .single(app.world())
            .unwrap();
        let truck = *app
            .world_mut()
            .query_filtered::<&GlobalTransform, With<crate::raid::truck::Truck>>()
            .single(app.world())
            .unwrap();
        let tail = truck.transform_point(Vec3::new(0.0, 0.0, 1.6));
        let stand = |app: &mut App, at: Vec3| {
            app.world_mut()
                .get_mut::<Transform>(me)
                .unwrap()
                .translation = at;
        };
        let raid = |app: &mut App| {
            app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
            app.update();
            app.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
            app.update();
        };
        let wait = |app: &mut App| {
            for _ in 0..(8 * 90) {
                app.update();
            }
        };
        let pack = app
            .world_mut()
            .query_filtered::<(Entity, &crate::interact::body::Backpack), Without<crate::raid::truck::Stash>>()
            .iter(app.world())
            .find(|(_, b)| b.owner == me)
            .map(|(e, _)| e)
            .expect("the newcomer's pack");
        let find_kind = catalog
            .iter()
            .find(|(_, d)| a_find(d.category))
            .map(|(k, _)| k)
            .expect("something that's a find");
        // Home from a raid, at the tail, nothing found on you: nothing.
        raid(&mut app);
        stand(&mut app, tail);
        wait(&mut app);
        assert_eq!(said(&app), 0, "said with nothing found on you");
        // A find in the pack, back to the tail: said.
        stand(
            &mut app,
            truck.transform_point(Vec3::new(0.0, crate::raid::truck::FLOOR, 0.0)),
        );
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let find = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind: find_kind,
                    transform: Transform::IDENTITY,
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        crate::persist::put_in_bag(app.world_mut(), find, pack, Vec3::ZERO);
        // The pack on your back (in the app it starts off you): what's in it
        // is on you.
        app.world_mut()
            .get_mut::<crate::interact::body::Backpack>(pack)
            .unwrap()
            .carried = true;
        app.update();
        stand(&mut app, tail);
        wait(&mut app);
        assert_eq!(said(&app), 1, "not said at the tail with a find on you");
        // Another raid, home, the find still on you, at the tail: not again.
        raid(&mut app);
        stand(&mut app, tail);
        wait(&mut app);
        assert_eq!(said(&app), 1, "said twice");
    }

    /// Home with no gun anywhere (the convoy took the kit, the crate's
    /// empty): the driver's spare, a magazine and rounds are in the kit
    /// crate for the next raid's loadout, and he says so; armed, he doesn't. (Unarmed after a lost kit,
    /// the hoarder went out and died two minutes in, raid after raid.)
    #[test]
    fn no_gun_home_gets_the_spare() {
        let mut app = app();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let guns: Vec<Entity> = app
            .world_mut()
            .query::<(Entity, &crate::items::Item)>()
            .iter(app.world())
            .filter(|(_, i)| catalog.get(i.kind()).firearm.is_some())
            .map(|(e, _)| e)
            .collect();
        for g in guns {
            app.world_mut().despawn(g);
        }
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.update();
        app.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
        for _ in 0..(8 * 90) {
            app.update();
        }
        let spoken = &app.world().resource::<Spoken>().0;
        assert_eq!(
            spoken.iter().filter(|l| *l == SPARE_GUN).count(),
            1,
            "{spoken:?}"
        );
        // In the crate, all three: the next raid's loadout has a gun.
        let in_crate: Vec<String> = {
            let mut q = app
                .world_mut()
                .query_filtered::<&crate::interact::body::Backpack, With<crate::raid::truck::Stash>>();
            let contents: Vec<Entity> = q
                .iter(app.world())
                .flat_map(|b| b.contents.clone())
                .collect();
            contents
                .into_iter()
                .filter_map(|e| app.world().get::<crate::items::Item>(e))
                .map(|i| catalog.get(i.kind()).id.clone())
                .collect()
        };
        for id in ["pm_pistol", "pm_mag", "ammo_9x18"] {
            assert!(
                in_crate.iter().any(|c| c == id),
                "no {id} in the kit crate: {in_crate:?}"
            );
        }
        // And the magazine full: a gun that fires (empty, beside a box, it
        // went out with nothing to shoot).
        let mags: Vec<u32> = app
            .world_mut()
            .query::<&crate::items::Item>()
            .iter(app.world())
            .filter(|i| catalog.get(i.kind()).id == "pm_mag")
            .map(|i| i.rounds)
            .collect();
        assert!(
            mags.iter().any(|r| *r == 8),
            "the spare magazine isn't full: {mags:?}"
        );
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
        // The job's not done (the career's own, or the letter, not in the
        // stash): not yet, and the card's own reason after it (never
        // "that's the job paid").
        let spoken = &app.world().resource::<Spoken>().0;
        assert_eq!(
            spoken.iter().filter(|s| *s == PAYDAY_NOT_YET).count(),
            1,
            "{spoken:?}"
        );
        assert!(!spoken.iter().any(|s| s == PAYDAY_PAID), "{spoken:?}");
        let at = spoken.iter().position(|s| s == PAYDAY_NOT_YET).unwrap();
        let why = spoken.get(at + 1).cloned().unwrap_or_default();
        assert!(why.starts_with("Driver:"), "after it: {why:?} ({spoken:?})");
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
        for id in ["canned_stew", "regen_injector"] {
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

    /// The first of a family in your hand out there: its line, once a
    /// career, three a raid at most (the rest on a later raid's pickup);
    /// not at the truck; not while something else is being said.
    #[test]
    fn a_familys_line_is_said_on_its_first_pickup_and_never_again() {
        use super::super::pickups::Family;
        let mut app = app();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let me = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .single(app.world())
            .unwrap();
        let hand = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::RigHand>>()
            .iter(app.world())
            .next()
            .unwrap();
        // (Long enough for what's queued ahead to be said, five seconds
        // apart: the driver's spares at the start, three lines.)
        let wait = |app: &mut App| {
            for _ in 0..(30 * 90) {
                app.update();
            }
        };
        let said = |app: &App, f: Family| {
            let line = f.lines()[0];
            app.world()
                .resource::<Spoken>()
                .0
                .iter()
                .filter(|l| *l == line)
                .count()
        };
        // In the hand a moment, then let go.
        let hold = |app: &mut App, id: &str| {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let e = {
                let mut c = Commands::new(&mut queue, app.world());
                crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind: catalog.kind(id).unwrap(),
                        transform: Transform::IDENTITY,
                        condition: 1.0,
                    },
                )
            };
            queue.apply(app.world_mut());
            app.world_mut().entity_mut(e).insert((
                Held {
                    player: me,
                    hand,
                    side: crate::rig::Side::Right,
                    point: 0,
                    offset: Transform::IDENTITY,
                },
                avian3d::prelude::RigidBodyDisabled,
            ));
            app.world_mut()
                .get_mut::<crate::interact::Hand>(hand)
                .unwrap()
                .holding = Some(e);
            app.update();
            app.world_mut()
                .get_mut::<crate::interact::Hand>(hand)
                .unwrap()
                .holding = None;
            app.world_mut().entity_mut(e).despawn();
            app.update();
        };
        let phase = |app: &mut App, p: Phase| {
            app.world_mut().resource_mut::<Raid>().phase = p;
            app.update();
        };
        // At the truck: nothing.
        phase(&mut app, Phase::AtBase);
        hold(&mut app, "gun_oil");
        wait(&mut app);
        assert_eq!(said(&app, Family::GunCare), 0, "said at the truck");
        // Out: said once; another of its family, not again.
        phase(&mut app, Phase::Out);
        // A machine that only tracks you (it heard something once) doesn't
        // hold it back; one that has noticed you does.
        let tracks = app
            .world_mut()
            .spawn(crate::robots::Awareness {
                target: Some(me),
                suspicion: 0.2,
                ..Default::default()
            })
            .id();
        // Something else being said: it waits its turn.
        app.world_mut()
            .resource_mut::<Queue>()
            .lines
            .push_back(("Truck: \"Busy.\"".into(), Show::View));
        hold(&mut app, "gun_oil");
        let spoken = app.world().resource::<Spoken>().0.clone();
        assert!(
            !spoken.contains(&Family::GunCare.lines()[0].to_string()),
            "said over another line"
        );
        wait(&mut app);
        assert_eq!(
            said(&app, Family::GunCare),
            1,
            "not said on its first pickup"
        );
        let spoken = app.world().resource::<Spoken>().0.clone();
        let at = |l: &str| spoken.iter().position(|s| s == l).unwrap();
        assert!(at("Truck: \"Busy.\"") < at(Family::GunCare.lines()[0]));
        hold(&mut app, "cleaning_kit");
        wait(&mut app);
        assert_eq!(said(&app, Family::GunCare), 1, "said twice");
        app.world_mut().despawn(tracks);
        let noticed = app
            .world_mut()
            .spawn(crate::robots::Awareness {
                target: Some(me),
                suspicion: crate::robots::NOTICED,
                ..Default::default()
            })
            .id();
        hold(&mut app, "matches");
        wait(&mut app);
        assert_eq!(said(&app, Family::Matches), 0, "said with a machine on you");
        app.world_mut().despawn(noticed);
        wait(&mut app);
        // Three a raid: the fourth waits.
        for id in ["knife", "door_wedge"] {
            hold(&mut app, id);
            wait(&mut app);
        }
        assert_eq!(said(&app, Family::Matches), 1);
        assert_eq!(said(&app, Family::Knife), 1);
        assert_eq!(said(&app, Family::Wedge), 0, "a fourth in one raid");
        // Food and the yard's things: nothing to say.
        for id in ["canned_stew", "power_cell", "pm_mag"] {
            hold(&mut app, id);
            wait(&mut app);
        }
        // Next raid, picked up again: said.
        phase(&mut app, Phase::AtBase);
        phase(&mut app, Phase::Out);
        hold(&mut app, "door_wedge");
        wait(&mut app);
        assert_eq!(said(&app, Family::Wedge), 1, "not said next raid");
        let picked = app
            .world()
            .resource::<Firsts>()
            .0
            .iter()
            .filter(|k| k.starts_with("Picked("))
            .count();
        assert_eq!(picked, 4, "only what was said is kept");
    }
}
