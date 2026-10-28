//! What the radio says: the driver (Lev, `docs/LORE.md`) and the
//! machines' chatter. Each moment has a few ways of saying it, taken in
//! turn so the same words don't come twice running, and on the drive home
//! the driver lets a little more of himself slip, one raid at a time.
//!
//! Everything here is said on the wrist, which shows three lines of 26:
//! the tests hold every line to that, with the longest place name in.

/// "the bus depot", "the pass": a place name as it sits in a sentence.
pub fn the(name: &str) -> String {
    let rest = name.strip_prefix("The ").unwrap_or(name);
    let mut c = rest.chars();
    match c.next() {
        Some(f) => format!("the {}{}", f.to_lowercase(), c.as_str()),
        None => "the gate".into(),
    }
}

/// A compass word for a direction on the map (north is -Z).
pub fn bearing_word(d: bevy::math::Vec2) -> &'static str {
    const WORDS: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    // Clockwise from north.
    let a = d.x.atan2(-d.y).rem_euclid(std::f32::consts::TAU);
    WORDS[((a / (std::f32::consts::TAU / 8.0)).round() as usize) % 8]
}

/// Which way and about how far, from `from` to `to` (map x, z): "north-
/// west, 480 m", to the nearest ten metres.
pub fn way(from: [f32; 2], to: [f32; 2]) -> String {
    let d = bevy::math::Vec2::from(to) - bevy::math::Vec2::from(from);
    let m = (d.length() / 10.0).round() * 10.0;
    format!("{}, {m:.0} m", bearing_word(d))
}

/// The `n`th way of saying something, round and round.
pub fn pick<'a>(ways: &[&'a str], n: u32) -> &'a str {
    ways[n as usize % ways.len()]
}

/// Called to a rendezvous: `{rv}` is where, `{b}` which way and how far
/// from you ("north-west, 480 m": the map only says RV, so the name alone
/// tells someone new nothing).
pub const CALLED: &[&str] = &[
    "Truck: \"Copy. {rv}, {b}. Two minutes.\"",
    "Truck: \"On my way to {rv}, {b}. Two minutes.\"",
    "Truck: \"{rv}, {b}. Two minutes. Keep low.\"",
    "Truck: \"Heard you. {rv}, {b}. Two minutes.\"",
];

/// Called again while on the way: `{s}` is seconds left.
pub const ON_THE_WAY: &[&str] = &[
    "Truck: \"Still on my way. {s} seconds.\"",
    "Truck: \"{s} seconds. Road's bad.\"",
    "Truck: \"I'm coming. {s} seconds.\"",
];

/// Arrived at `{rv}`.
pub const ARRIVED: &[&str] = &[
    "Truck: \"At {rv}. I'll wait five minutes, no more.\"",
    "Truck: \"I'm at {rv}. Five minutes, then I go.\"",
    "Truck: \"At {rv}. Engine's running. Five minutes.\"",
];

/// Called while waiting at `{rv}`, `{b}` from you.
pub const WAITING: &[&str] = &[
    "Truck: \"I'm at {rv}, {b}. Get in the back.\"",
    "Truck: \"Still at {rv}, {b}. Hurry.\"",
    "Truck: \"{rv}, {b}. I hear them.\"",
];

pub const GAVE_UP: &[&str] = &[
    "Truck: \"Can't stay. Call me again when you're close.\"",
    "Truck: \"Pulling out. Call when you're close.\"",
];

pub const SHORT_RUN: &[&str] = &[
    "Truck: \"Burning a can for you. Get in the back.\"",
    "Truck: \"Short run. That's a can of fuel. Get in.\"",
];

/// On the first plant job's card: there's a roof within a sprint all the
/// way (`map_lint`), and a drone's chirp is the reason to use one.
pub const SHEDS_ON_THE_WAY: &str = "\nSheds on the way: in, if anything chirps.";

/// The first job's way home, from the gate, on the house.
pub const FIRST_SHORT_RUN: &str = "Truck: \"First run's on me. Get in the back.\"";

pub const NO_FUEL: &str = "Truck: \"No fuel in the stores for the short run. Head for an RV.\"";

pub const PULLING_OUT: &[&str] = &[
    "Truck: \"Hold on to something.\" You're out.",
    "Truck: \"Got you. Hold on.\" You're out.",
    "Truck: \"In? Go.\" You're out.",
];

/// Going home, between the things he's ready to say.
pub const SMALL_TALK: &[&str] = &[
    "Driver: \"Heater's broken. Sit on your hands.\"",
    "Driver: \"Rain again. It always rains in there.\"",
    "Driver: \"Tea in the flask. Leave me some.\"",
    "Driver: \"You're dripping on my floor.\"",
    "Driver: \"Road's worse every year. Nobody mends it.\"",
    "Driver: \"Sleep if you can. I'll wake you at camp.\"",
    "Driver: \"Quiet one. I like the quiet ones.\"",
];

/// What he finally says, by how many times you've come home: the job,
/// then the bus, then the gate, then his mother (`docs/LORE.md`).
pub const COMING_HOME: &[(u32, &str)] = &[
    (1, "Driver: \"You came back. Most do, the first time.\""),
    (
        2,
        "Driver: \"Don't tell me what you saw. Just if you're hurt.\"",
    ),
    (
        3,
        "Driver: \"I drove buses before this. The Marrow route.\"",
    ),
    (
        5,
        "Driver: \"Last time I was past that fence, soldiers had it.\"",
    ),
    (
        7,
        "Driver: \"Fourteenth of October. Bus four. Every seat full.\"",
    ),
    (
        9,
        "Driver: \"I turned the bus round at the gate. They kept me out.\"",
    ),
    (
        11,
        "Driver: \"My mother stayed in Marrow. For the dog, she said.\"",
    ),
    (
        13,
        "Driver: \"If you're ever in the house by the well... no. Never mind.\"",
    ),
    (
        16,
        "Driver: \"I told her three days. They told us to say three days.\"",
    ),
    (
        20,
        "Driver: \"Twenty times home. You're off the count, then.\"",
    ),
];

/// After the recall: the valleys are quiet, and so, more or less, is he.
pub const COMING_HOME_QUIET: &[&str] = &[
    "Driver: \"Quiet in there now. Like before. Nearly.\"",
    "Driver: \"Maybe I'll drive in one day. Not today.\"",
    "Driver: \"No machines. Still don't like the look of it.\"",
];

/// The driver's word on the way home, after the `n`th homecoming.
pub fn coming_home(n: u32, recalled: bool) -> &'static str {
    if recalled {
        return pick(COMING_HOME_QUIET, n);
    }
    COMING_HOME
        .iter()
        .find(|(at, _)| *at == n)
        .map(|(_, l)| *l)
        .unwrap_or_else(|| pick(SMALL_TALK, n))
}

/// On the drive home, when he notices what you've brought (and has none
/// of his own story to tell that night): `raid::lev`.
/// Hana's tape: Anna, his sister, at the piano as a girl.
pub const CARGO_ANNA: &[&str] = &["Driver: \"Where did you get that? That's Anna playing.\""];
pub const CARGO_ARTIFACT: &[&str] = &[
    "Driver: \"Something in your kit's humming. Box it before I drive.\"",
    "Driver: \"You've a stone on you. I can feel it in my teeth.\"",
];
pub const CARGO_EYE: &[&str] = &[
    "Driver: \"One of their eyes? Don't point it at me.\"",
    "Driver: \"That eye still looks at you. Bag it.\"",
];
pub const CARGO_CELL: &[&str] = &[
    "Driver: \"One of their cells. Heavier than it looks, eh?\"",
    "Driver: \"Their cell. Keep it off the floor. It ticks.\"",
];
pub const CARGO_FOOD: &[&str] = &[
    "Driver: \"Tins! We'll eat tonight.\"",
    "Driver: \"That's a week of suppers you've got there.\"",
];
/// This much food and he says so.
pub const FOOD_PLENTY: usize = 5;

/// Home from a valley for the first time: what he knows of it from
/// before, when he drove there (he's never been past the fence since).
pub const FIRST_HOME_FROM: &[(&str, &str)] = &[
    (
        "marrow_creek",
        "Driver: \"Marrow. Mind the well. Mum always said that.\"",
    ),
    (
        "kettle_pass",
        "Driver: \"Kettle. The miners sang on my bus. Every shift.\"",
    ),
    (
        "cinder_works",
        "Driver: \"Cinder. You smell of it. The ash gets in everything.\"",
    ),
    (
        "sallow_reservoir",
        "Driver: \"Sallow. We swam there as boys. They say it's dry now.\"",
    ),
    (
        "rime_fell",
        "Driver: \"Rime. Chains on in June up there. You're shaking.\"",
    ),
];

/// The drive-home line: his story when it's that homecoming; Anna's tape
/// or a stone if you have one; his word on a valley you're home from for
/// the first time (`first_from`); then an eye, a cell or a lot of food;
/// then the rest.
pub fn coming_home_with(
    n: u32,
    recalled: bool,
    c: super::lev::Cargo,
    first_from: Option<&str>,
) -> &'static str {
    if !recalled && let Some((_, l)) = COMING_HOME.iter().find(|(at, _)| *at == n) {
        return l;
    }
    // Anna's tape, and a stone humming in your kit (a warning), before
    // his word on the valley.
    if c.anna {
        return pick(CARGO_ANNA, n);
    }
    if c.artifact {
        return pick(CARGO_ARTIFACT, n);
    }
    if let Some(map) = first_from
        && let Some((_, l)) = FIRST_HOME_FROM.iter().find(|(m, _)| *m == map)
    {
        return l;
    }
    if c.eye {
        pick(CARGO_EYE, n)
    } else if c.cell {
        pick(CARGO_CELL, n)
    } else if c.food >= FOOD_PLENTY {
        pick(CARGO_FOOD, n)
    } else {
        coming_home(n, recalled)
    }
}

/// Machines talking about you, close by.
pub const CHATTER_CLOSE: &[&str] = &[
    "Robot chatter on the radio. They're coming here.",
    "Machine talk on the radio, loud and close: coming here.",
    "The radio stutters in their tones. They're coming here.",
];

/// A machine on you calls its part in the plan (`robots::squad`): one
/// working round to your `{b}`...
pub const SQUAD_FLANK: &[&str] = &[
    "Radio, close: one going round to your {b}.",
    "Chirps on the radio: one working round to the {b}.",
    "The radio clicks: one circling to your {b}.",
];
/// ...or, by a place it can name (`{rv}`)...
pub const SQUAD_FLANK_BY: &[&str] = &[
    "Radio, close: one round your {b}, by {rv}.",
    "Chirps: one round to the {b}, by {rv}.",
];
/// ...one making for the way to `{rv}`, to cut you off...
pub const SQUAD_CUT_OFF: &[&str] = &[
    "Radio: one making for {rv}. They mean to cut you off.",
    "Chirps: one heading for {rv}, ahead of you.",
    "The radio clicks: one off toward {rv}, to cut you off.",
];
/// ...or round to the `{b}`, where it can't name...
pub const SQUAD_CUT_OFF_AWAY: &[&str] = &[
    "Radio: one moving round to the {b}, to cut you off.",
    "Chirps: one off to the {b}, to get ahead of you.",
];
/// ...one holding you...
pub const SQUAD_PIN: &[&str] = &[
    "Radio, close: one holding on you.",
    "The radio clicks: one keeping you pinned.",
];
/// ...the drone lighting the `{b}`...
pub const SQUAD_SPOT: &[&str] = &[
    "Radio: the drone's lighting the {b}.",
    "Chirps: the drone moving to light the {b}.",
];
/// ...or by a place it can name...
pub const SQUAD_SPOT_BY: &[&str] = &[
    "Radio: the drone lights the {b}, by {rv}.",
    "Chirps: the drone over {rv}, lighting it.",
];
/// ...a relay calling it in.
pub const SQUAD_CALL: &[&str] = &[
    "Radio: a relay calling it in. More coming.",
    "The radio stutters: a relay's called them.",
];

/// Machines talking about something off to the `{b}` (a bearing).
pub const CHATTER_FAR: &[&str] = &[
    "Robot chatter on the radio: something's moving, off to the {b}.",
    "Machine talk on the radio, faint. A team heading {b}.",
    "The radio clicks in their tones: something off to the {b}.",
];

/// The thread (`world::story::BEATS`): what the driver says on the radio
/// between raids when a rank opens the next beat. One line every few
/// seconds, in order.
pub const BEAT_RUSTY: &[&str] = &[
    "Driver: \"We had a dog. Rusty. Mum's, really. Daft thing.\"",
    "Driver: \"No dogs on the buses. That's why she stayed.\"",
];
pub const BEAT_HOLMS: &[&str] = &[
    "Driver: \"The Holms ran the post counter. Mum's oldest friends.\"",
    "Driver: \"Mrs Holm went down the shelter, they said. Never came up.\"",
];
pub const BEAT_COLLECTOR: &[&str] = &[
    "Driver: \"I saw the collector come for someone in Marrow once.\"",
    "Driver: \"February. From the fence. I watched it go to the Gate.\"",
    "Driver: \"There's a letter on her table. I know there is.\"",
];
pub const BEAT_LETTER: &[&str] = &[
    "Driver: \"She says they keep everything. For the review. Her too.\"",
    "Driver: \"Three keys stop them, the inspector told her. All three.\"",
    "Driver: \"Her photo's in the stash. Take it out there with you.\"",
];
pub const BEAT_ENDED: &[&str] = &["Driver: \"Thank you. For walking down with me.\""];
pub const BEAT_RECALLED: &[&str] = &[
    "Truck: \"They're walking home. All of them. I can see it from here.\"",
    "Driver: \"Next time you go in, I'm coming. First time.\"",
];

/// A machine close by has stopped looking for you
/// (`robots::search::LostYou`).
pub const LOST_YOU: &[&str] = &[
    "The whirring fades. It's stopped looking.",
    "Close by, a machine goes still, then walks off.",
    "Its eye swings away. It's given up.",
];

/// A machine that lost you is coming back your way
/// (`robots::search::ComingBack`).
pub const COMING_BACK: &[&str] = &[
    "Steps again, coming your way. It's back to look.",
    "Its whirr turns back toward you. Keep still.",
    "It's coming back. Slow steps, nearer.",
];

/// A machine close by calls the others (`robots::search::CalledOut`).
pub const CALLED_OUT: &[&str] = &[
    "Chirps answer each other nearby. They're calling.",
    "A machine close by calls out. Others answer.",
    "Two-tone chirps, close. More of them coming.",
];

/// The Zone remembers you (`robots::memory`). Picked up on your usual
/// way, near `{rv}` (a place)...
pub const MEMORY_CONTACT: &[&str] = &[
    "Machine talk, close. They know you by {rv} now.",
    "The radio clicks in their tones: {rv} again.",
    "Chirps, near. They've had you by {rv} before.",
];
/// ...or nowhere it can name.
pub const MEMORY_CONTACT_AWAY: &[&str] = &[
    "Machine talk, close. They know the way you come.",
    "The radio clicks: your usual way. They were waiting.",
];
/// Stepping out: one of theirs left watching where you wrecked one, by
/// `{rv}`...
pub const MEMORY_WATCHER: &[&str] = &[
    "Driver: \"Mind {rv}. They've left one there.\"",
    "Driver: \"One stands by {rv} now. Since your last.\"",
];
/// ...or somewhere it can't name.
pub const MEMORY_WATCHER_AWAY: &[&str] =
    &["Driver: \"Where you dropped theirs, there's another stood now.\""];
/// The rendezvous you always use, `{rv}`.
pub const MEMORY_RV: &[&str] = &[
    "Driver: \"{rv} again? They've noticed.\"",
    "Driver: \"Not {rv} every time. They count.\"",
];
/// A building you cleared, by `{rv}`, found shut.
pub const MEMORY_SHUT: &[&str] = &["Driver: \"Your place by {rv}? Shut again.\""];
/// Nothing in particular, but you keep to the same ways.
pub const MEMORY_WAYS: &[&str] = &[
    "Driver: \"Same way in every time. Even I could find you.\"",
    "Driver: \"Go in another way. They count, you know.\"",
];

/// Fills a line's `{rv}`, `{s}` and `{b}`; a place leading a line gets a
/// capital.
pub fn fill(line: &str, rv: &str, s: f32, b: &str) -> String {
    let place = the(rv);
    let mut c = place.chars();
    let cap: String = c
        .next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default();
    let mut out = line.to_string();
    // Leading a sentence, it takes a capital.
    for lead in ["\"", ". "] {
        out = out.replace(&format!("{lead}{{rv}}"), &format!("{lead}{cap}"));
    }
    out.replace("{rv}", &place)
        .replace("{s}", &format!("{s:.0}"))
        .replace("{b}", b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::screen::wrap;

    /// Every name a rendezvous can have, in every map.
    fn rendezvous_names() -> Vec<String> {
        let mut out = vec!["gate".to_string()];
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            let text = std::fs::read_to_string(
                crate::asset_root_path()
                    .join("maps")
                    .join(format!("{map}.ron")),
            )
            .unwrap();
            let def = extraction_core::map::MapDef::from_ron(&text).unwrap();
            out.extend(
                def.markers
                    .iter()
                    .filter(|m| m.kind == extraction_core::map::MarkerKind::Rendezvous)
                    .map(|m| m.name.clone()),
            );
        }
        out
    }

    /// Which way: clockwise from north (-Z), to the nearest ten metres.
    #[test]
    fn the_way_reads_like_a_compass() {
        assert_eq!(way([0.0, 0.0], [0.0, -100.0]), "north, 100 m");
        assert_eq!(way([0.0, 0.0], [141.0, -141.0]), "north-east, 200 m");
        assert_eq!(way([40.0, 270.0], [-232.0, -128.0]), "north-west, 480 m");
        assert_eq!(way([0.0, 0.0], [-50.0, 3.0]), "west, 50 m");
    }

    /// Every line, filled with every place, fits the wrist whole.
    #[test]
    fn every_line_fits_the_wrist() {
        let mut lines: Vec<String> = Vec::new();
        let pools: [&[&str]; 42] = [
            SQUAD_FLANK_BY,
            SQUAD_SPOT_BY,
            SQUAD_FLANK,
            SQUAD_CUT_OFF,
            SQUAD_CUT_OFF_AWAY,
            SQUAD_PIN,
            SQUAD_SPOT,
            SQUAD_CALL,
            MEMORY_CONTACT,
            MEMORY_CONTACT_AWAY,
            MEMORY_WATCHER,
            MEMORY_WATCHER_AWAY,
            MEMORY_RV,
            MEMORY_SHUT,
            MEMORY_WAYS,
            crate::raid::epilogue::FOUND,
            BEAT_ENDED,
            &[
                crate::raid::epilogue::SETTING_OFF,
                crate::raid::epilogue::GOING_DOWN,
                crate::raid::epilogue::STACKS_SHUT,
                crate::raid::epilogue::RELEASED,
            ],
            CARGO_ANNA,
            CARGO_ARTIFACT,
            CARGO_EYE,
            CARGO_CELL,
            CARGO_FOOD,
            BEAT_RUSTY,
            BEAT_HOLMS,
            BEAT_COLLECTOR,
            BEAT_LETTER,
            BEAT_RECALLED,
            LOST_YOU,
            CALLED_OUT,
            CALLED,
            ON_THE_WAY,
            ARRIVED,
            WAITING,
            GAVE_UP,
            SHORT_RUN,
            PULLING_OUT,
            SMALL_TALK,
            COMING_HOME_QUIET,
            CHATTER_CLOSE,
            CHATTER_FAR,
            &[NO_FUEL, FIRST_SHORT_RUN],
        ];
        for pool in pools {
            for line in pool {
                for rv in rendezvous_names() {
                    // The truck gives a way and distance; chatter, a way.
                    let b = if line.contains("{rv}") {
                        "north-west, 1500 m"
                    } else {
                        "north-west"
                    };
                    lines.push(fill(line, &rv, 120.0, b));
                }
            }
        }
        lines.extend(COMING_HOME.iter().map(|(_, l)| l.to_string()));
        lines.extend(FIRST_HOME_FROM.iter().map(|(_, l)| l.to_string()));
        for line in &lines {
            let wrapped = wrap(line, 26, 3);
            assert_eq!(
                wrapped.join(" "),
                line.split_whitespace().collect::<Vec<_>>().join(" "),
                "doesn't fit the wrist: {line:?} -> {wrapped:?}"
            );
            assert!(!line.contains('{'), "unfilled: {line}");
        }
    }

    /// The drive home says one thing, in this order, whatever else is
    /// true that night: his own story beat; Anna's tape; a stone (a
    /// warning); his word on a valley you're home from the first time; an
    /// eye; a cell; a lot of food; then small talk. Every pair is held.
    #[test]
    fn the_drive_home_keeps_its_order() {
        use crate::raid::lev::Cargo;
        // Each kind as the conditions that bring it on, and its line.
        type Kind = (
            &'static str,
            fn(&mut (u32, Cargo, Option<&'static str>)),
            &'static [&'static str],
        );
        const STORY: &[&str] = &[COMING_HOME[2].1];
        const VALLEY: &[&str] = &[FIRST_HOME_FROM[0].1];
        let kinds: [Kind; 7] = [
            ("story", |c| c.0 = COMING_HOME[2].0, STORY),
            ("anna", |c| c.1.anna = true, CARGO_ANNA),
            ("stone", |c| c.1.artifact = true, CARGO_ARTIFACT),
            ("valley", |c| c.2 = Some(FIRST_HOME_FROM[0].0), VALLEY),
            ("eye", |c| c.1.eye = true, CARGO_EYE),
            ("cell", |c| c.1.cell = true, CARGO_CELL),
            ("food", |c| c.1.food = FOOD_PLENTY, CARGO_FOOD),
        ];
        // A homecoming with no story of his own.
        let quiet = COMING_HOME.iter().map(|(n, _)| *n).max().unwrap() + 1;
        for (i, (hi, set_hi, lines)) in kinds.iter().enumerate() {
            for (lo, set_lo, _) in &kinds[i + 1..] {
                let mut c = (quiet, Cargo::default(), None);
                set_lo(&mut c);
                set_hi(&mut c);
                let said = coming_home_with(c.0, false, c.1, c.2);
                assert!(lines.contains(&said), "{hi} over {lo}: said {said:?}");
            }
            // And over small talk.
            let mut c = (quiet, Cargo::default(), None);
            set_hi(&mut c);
            assert!(
                lines.contains(&coming_home_with(c.0, false, c.1, c.2)),
                "{hi}"
            );
        }
    }

    /// Home from a valley the first time, he says his piece about it,
    /// unless it's the night for his own story; every valley has one.
    #[test]
    fn first_home_from_a_valley_he_speaks_of_it() {
        let none = crate::raid::lev::Cargo::default();
        for (map, _) in extraction_core::map::VALLEYS {
            let line = coming_home_with(4, false, none, Some(map));
            assert!(
                FIRST_HOME_FROM.iter().any(|(m, l)| *m == map && *l == line),
                "{map}: {line}"
            );
        }
        // His own story comes first (the third homecoming: the bus).
        assert_eq!(
            coming_home_with(3, false, none, Some("sallow_reservoir")),
            coming_home(3, false)
        );
        // Not the first time: the usual.
        assert_eq!(
            coming_home_with(4, false, none, None),
            coming_home(4, false)
        );
    }

    #[test]
    fn places_read_as_places() {
        assert_eq!(the("The pass"), "the pass");
        assert_eq!(the("Bus depot"), "the bus depot");
        assert_eq!(
            fill(CALLED[2], "The pass", 0.0, "east, 300 m"),
            "Truck: \"The pass, east, 300 m. Two minutes. Keep low.\""
        );
        assert_eq!(
            fill(CALLED[0], "Hunting lodge", 0.0, "north-west, 480 m"),
            "Truck: \"Copy. The hunting lodge, north-west, 480 m. Two minutes.\""
        );
    }

    /// He says each thing once, in order, and small talk in between.
    #[test]
    fn the_driver_opens_up_slowly() {
        let said: Vec<&str> = (1..=24).map(|n| coming_home(n, false)).collect();
        for (n, line) in COMING_HOME {
            assert_eq!(said.iter().filter(|l| *l == line).count(), 1, "{line}");
            assert_eq!(said[*n as usize - 1], *line);
        }
        assert!(said.iter().any(|l| SMALL_TALK.contains(l)));
        assert!(COMING_HOME.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(COMING_HOME_QUIET.contains(&coming_home(3, true)));
    }
}
