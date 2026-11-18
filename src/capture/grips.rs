//! What looks usable is usable: every prop model that draws something to
//! take hold of (a handle, knob, lever, crank, bail, latch, grip or pull,
//! as its art names it) is explained here, and the explanation holds.
//! A grip nobody can use is a lie (`docs/WORLD_MODELS.md`).

/// Why a model's grip is no lie.
#[derive(Clone, Copy, Debug)]
pub enum Why {
    /// It opens: a moving part in the hand (`world::hinge`).
    Opens,
    /// A handle the gauges hold to reach and legibility
    /// (`capture::checks::handles`).
    Gauged,
    /// Rusted fast, sheared or snapped: shows it doesn't work
    /// (`docs/WORLD_MODELS.md`).
    Seized,
    /// Chained and padlocked: a future bolt cutters' target
    /// (`docs/WORLD_MODELS.md`).
    Locked,
    /// The game works it (where).
    Worked(&'static str),
    /// Part of what it is: a carry handle on a thing too big or too fixed
    /// to carry, a tool's grip on a rack (why).
    Form(&'static str),
    /// It says there's none: the word's in a comment saying so (why).
    Not(&'static str),
}

/// Every prop model whose art draws a grip, and why it's no lie.
pub const LEDGER: &[(&str, Why)] = &[
    (
        "bar_rack",
        Why::Form("a rack of bars and rakes on the wall: tools' handles, stood up"),
    ),
    ("brake_drum", Why::Seized),
    (
        "cab_hatch_panel",
        Why::Worked("raid::lev: the truck's cab hatch slides on its pull"),
    ),
    (
        "cap_hook",
        Why::Form("a lamp hung on a peg by its handle, with the caps"),
    ),
    (
        "chair",
        Why::Not("its apron's helper says it: no drawer, so no knob to open nothing"),
    ),
    ("cellar_vent", Why::Gauged),
    ("coal_bunker", Why::Opens),
    ("control_stand", Why::Seized),
    ("counter_cabinet", Why::Opens),
    ("counter_drawers", Why::Opens),
    (
        "depot_crate_long",
        Why::Form("a crate's rope handles: too heavy to carry alone"),
    ),
    (
        "depot_crate_small",
        Why::Form("a crate's rope handles: too heavy to carry alone"),
    ),
    ("door_leaf", Why::Gauged),
    ("door_metal", Why::Gauged),
    (
        "field_set",
        Why::Worked("raid::transceiver: pressed, it calls the truck"),
    ),
    ("filter_unit", Why::Seized),
    (
        "firewood_basket",
        Why::Form("a log basket's cane loops, full of logs by a stove"),
    ),
    (
        "garden_tools",
        Why::Form("tools' grips on their pegs, a shed wall's"),
    ),
    (
        "gramophone",
        Why::Worked("radio::gramophone: wind it by its crank"),
    ),
    ("hand_pump", Why::Seized),
    (
        "harmonium",
        Why::Worked("radio::piano: its keys; its stops are Radio's, set where they are"),
    ),
    (
        "hurricane_lamp",
        Why::Form("a lamp's bail; open: Hands/Items: grabbable loose thing?"),
    ),
    (
        "kept_bicycle",
        Why::Form("a bicycle's grips: a thing kept, not ridden"),
    ),
    ("landing_net", Why::Form("a net's handle, hung on a wall")),
    (
        "lantern",
        Why::Form("a lamp's bail; open: Hands/Items: grabbable loose thing?"),
    ),
    ("lever_frame", Why::Seized),
    ("locker", Why::Opens),
    ("logbook", Why::Seized),
    (
        "lunch_pail",
        Why::Form("a pail's bail; open: Hands/Items: grabbable loose thing?"),
    ),
    ("mill_ore_bin", Why::Seized),
    (
        "skis_wall",
        Why::Form("skis and poles' grips, stood against a wall"),
    ),
    (
        "sledge",
        Why::Form("a sledge's pull rope: a thing kept, not hauled"),
    ),
    ("snow_shovel", Why::Form("a shovel's grip, stood by a door")),
    (
        "snow_shovel_out",
        Why::Form("a shovel's grip, stood in the snow by a door"),
    ),
    (
        "snow_pole",
        Why::Form("a pole stood in the snow, its leather grip and wrist loop"),
    ),
    ("stove", Why::Opens),
    (
        "suitcase_shut",
        Why::Form("a case's handle; open: Hands/Items: grabbable loose thing?"),
    ),
    ("switch_cabinet", Why::Locked),
    (
        "table",
        Why::Not("no drawer in its apron: a knob that opened nothing was a lie"),
    ),
    ("tool_chest", Why::Opens),
    (
        "upright_piano",
        Why::Worked("radio::piano: its keys; its pedals are for feet"),
    ),
    ("wall_telephone", Why::Seized),
    (
        "watering_can",
        Why::Form("a can's handle; open: Hands/Items: grabbable loose thing?"),
    ),
    (
        "wheelbarrow",
        Why::Form("a barrow's handles: a thing kept, not wheeled"),
    ),
    ("warning_door", Why::Locked),
    ("warning_gate", Why::Locked),
    ("wheelie_bin", Why::Opens),
    ("winch", Why::Locked),
    ("window_sash", Why::Opens),
    (
        "workbench",
        Why::Not("its pedestal's drawers' pulls are gone: they opened nothing"),
    ),
];

/// What names a grip in the art.
const WORDS: &[&str] = &[
    "handle", "handles", "knob", "knobs", "lever", "levers", "crank", "cranks", "bail", "bails",
    "latch", "latches", "grip", "grips", "pull", "pulls",
];

/// Every prop model whose builder (or a helper it calls, in its own
/// file) names a grip, in its code or its comments: the model, its file
/// and the words.
pub fn named_grips() -> std::collections::BTreeMap<String, (String, Vec<String>)> {
    use std::collections::{BTreeMap, BTreeSet};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("art/props");
    let mut out = BTreeMap::new();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    let tokens = |s: &str| -> Vec<String> {
        s.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect()
    };
    for path in files
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "py"))
    {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Its top-level functions' bodies.
        let mut funcs: BTreeMap<String, String> = BTreeMap::new();
        let mut current: Option<String> = None;
        for line in src.lines() {
            if let Some(rest) = line.strip_prefix("def ") {
                let name = rest.split('(').next().unwrap_or("").to_string();
                funcs.insert(name.clone(), String::new());
                current = Some(name);
            } else if !line.is_empty() && !line.starts_with([' ', '\t']) {
                current = None;
            }
            if let Some(f) = &current {
                let body = funcs.get_mut(f).expect("inserted");
                body.push_str(line);
                body.push('\n');
            }
        }
        // What each names, with what the helpers it calls name.
        fn words(
            f: &str,
            funcs: &BTreeMap<String, String>,
            tokens: &dyn Fn(&str) -> Vec<String>,
            seen: &mut BTreeSet<String>,
        ) -> BTreeSet<String> {
            let mut out = BTreeSet::new();
            if !seen.insert(f.to_string()) {
                return out;
            }
            let Some(body) = funcs.get(f) else {
                return out;
            };
            for t in tokens(body) {
                let low = t.to_lowercase();
                if WORDS.contains(&low.as_str()) {
                    out.insert(low);
                } else if t != f && funcs.contains_key(&t) {
                    out.extend(words(&t, funcs, tokens, seen));
                }
            }
            out
        }
        // Its models: `prop("name", builder)` and `("name", builder)`.
        for line in src.lines() {
            let mut rest = line;
            while let Some(a) = rest.find('"') {
                let after = &rest[a + 1..];
                let Some(b) = after.find('"') else {
                    break;
                };
                let model = &after[..b];
                let tail = after[b + 1..].trim_start();
                rest = &after[b + 1..];
                let Some(tail) = tail.strip_prefix(',') else {
                    continue;
                };
                let builder: String = tail
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !funcs.contains_key(&builder)
                    || model.is_empty()
                    || !model.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    continue;
                }
                let w = words(&builder, &funcs, &tokens, &mut BTreeSet::new());
                if !w.is_empty() {
                    let file = path.file_name().unwrap().to_string_lossy().to_string();
                    out.insert(model.to_string(), (file, w.into_iter().collect()));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No prop model carries an unexplained grip: each one its art names
    /// is in the ledger, and its reason holds (it opens, the gauges hold
    /// it, it's listed as seized or locked); each moving part's model is
    /// in it as opening; and nothing's in it that no longer has one.
    #[test]
    fn no_prop_carries_an_unexplained_grip() {
        let named = named_grips();
        let doc = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/WORLD_MODELS.md"),
        )
        .unwrap_or_default();
        let gauged: Vec<String> = crate::capture::checks::handles()
            .into_iter()
            .map(|h| h.what)
            .collect();
        let moving: Vec<&str> = crate::world::hinge::moving_nodes()
            .map(|n| n.0)
            .chain(crate::world::hinge::SASH_LEAVES.iter().map(|l| l.0))
            .map(|n| n.split('#').next().unwrap_or(n))
            .collect();
        let mut bad = Vec::new();
        for (model, (file, words)) in &named {
            println!("{model:<20} {file:<16} {words:?}");
            if !LEDGER.iter().any(|(m, _)| m == model) {
                bad.push(format!(
                    "{model} ({file}): a grip ({}) nobody's explained",
                    words.join(", ")
                ));
            }
        }
        for (model, why) in LEDGER {
            let holds = match why {
                Why::Opens => moving.contains(model),
                Why::Gauged => gauged.iter().any(|w| w.contains(&format!("({model})"))),
                Why::Seized | Why::Locked => doc.contains(&format!("`{model}`")),
                Why::Worked(_) | Why::Form(_) | Why::Not(_) => true,
            };
            if !holds {
                bad.push(format!("{model}: {why:?}, but it isn't"));
            }
            if !matches!(why, Why::Opens) && !named.contains_key(*model) {
                bad.push(format!("{model}: in the ledger, but its art names no grip"));
            }
        }
        for model in &moving {
            if !LEDGER
                .iter()
                .any(|(m, why)| m == model && matches!(why, Why::Opens))
            {
                bad.push(format!(
                    "{model}: a moving part not in the ledger as opening"
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "grips that look usable and aren't:\n{}",
            bad.join("\n")
        );
    }
}
