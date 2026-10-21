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

/// The `n`th way of saying something, round and round.
pub fn pick<'a>(ways: &[&'a str], n: u32) -> &'a str {
    ways[n as usize % ways.len()]
}

/// Called to a rendezvous: `{rv}` is where.
pub const CALLED: &[&str] = &[
    "Truck: \"Copy. Heading for {rv}. Give me two minutes.\"",
    "Truck: \"On my way to {rv}. Two minutes.\"",
    "Truck: \"{rv}. Two minutes. Keep your head down.\"",
    "Truck: \"Heard you. {rv}, two minutes.\"",
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

/// Called while waiting at `{rv}`.
pub const WAITING: &[&str] = &[
    "Truck: \"I'm at {rv}. Get in the back.\"",
    "Truck: \"Still at {rv}. Don't make me wait.\"",
    "Truck: \"{rv}. I can hear them. Hurry.\"",
];

pub const GAVE_UP: &[&str] = &[
    "Truck: \"Can't stay. Call me again when you're close.\"",
    "Truck: \"Pulling out. Call when you're close.\"",
];

pub const SHORT_RUN: &[&str] = &[
    "Truck: \"Burning a can for you. Get in the back.\"",
    "Truck: \"Short run. That's a can of fuel. Get in.\"",
];

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

/// The drive-home line: his story when it's that homecoming, then what
/// you've brought (a stone, an eye, a cell, a lot of food), then the rest.
pub fn coming_home_with(n: u32, recalled: bool, c: super::lev::Cargo) -> &'static str {
    if !recalled && let Some((_, l)) = COMING_HOME.iter().find(|(at, _)| *at == n) {
        return l;
    }
    if c.artifact {
        pick(CARGO_ARTIFACT, n)
    } else if c.eye {
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

/// A machine close by calls the others (`robots::search::CalledOut`).
pub const CALLED_OUT: &[&str] = &[
    "Chirps answer each other nearby. They're calling.",
    "A machine close by calls out. Others answer.",
    "Two-tone chirps, close. More of them coming.",
];

/// Fills a line's `{rv}`, `{s}` and `{b}`; a place leading a line gets a
/// capital.
pub fn fill(line: &str, rv: &str, s: f32, b: &str) -> String {
    let place = the(rv);
    let lead = format!("\"{{rv}}");
    let mut out = line.to_string();
    if out.contains(&lead) {
        let mut c = place.chars();
        let cap: String = c
            .next()
            .map(|f| f.to_uppercase().chain(c).collect())
            .unwrap_or_default();
        out = out.replace(&lead, &format!("\"{cap}"));
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

    /// Every line, filled with every place, fits the wrist whole.
    #[test]
    fn every_line_fits_the_wrist() {
        let mut lines: Vec<String> = Vec::new();
        let pools: [&[&str]; 26] = [
            crate::raid::epilogue::FOUND,
            BEAT_ENDED,
            &[
                crate::raid::epilogue::SETTING_OFF,
                crate::raid::epilogue::GOING_DOWN,
                crate::raid::epilogue::STACKS_SHUT,
                crate::raid::epilogue::RELEASED,
            ],
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
            &[NO_FUEL],
        ];
        for pool in pools {
            for line in pool {
                for rv in rendezvous_names() {
                    lines.push(fill(line, &rv, 120.0, "north-west"));
                }
            }
        }
        lines.extend(COMING_HOME.iter().map(|(_, l)| l.to_string()));
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

    #[test]
    fn places_read_as_places() {
        assert_eq!(the("The pass"), "the pass");
        assert_eq!(the("Bus depot"), "the bus depot");
        assert_eq!(
            fill(CALLED[2], "The pass", 0.0, ""),
            "Truck: \"The pass. Two minutes. Keep your head down.\""
        );
        assert_eq!(
            fill(CALLED[0], "Hunting lodge", 0.0, ""),
            "Truck: \"Copy. Heading for the hunting lodge. Give me two minutes.\""
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
