//! What a headset shows legibly, and every source of text in the game
//! measured against it. The reasoning is in `docs/LEGIBILITY.md`.
//!
//! Letters are measured by their smallest letters' height as an angle at
//! the eye, from where you'd read them: the x-height if the text has lower
//! case, else the cap height. Two floors:
//!
//! - [`MUST_READ_DEG`]: instructions, labels you act on, numbers you dial.
//! - [`INCIDENTAL_DEG`]: flavour (stampings, the memorial's names): legible
//!   with effort, by leaning in or bringing it to your face.
//!
//! Every [`ScreenText`] panel carries a [`TextSource`] saying what it is;
//! [`TextSource::reading`] says how it's read. Text baked into models
//! (`art/**/*.py`, the stencil `text()` helper) and textures (the field
//! map) is listed in [`ART`] and [`BAKED`]. Tests hold all three to the
//! floors, and `scripts/legibility.sh` renders each panel from its reading
//! pose at the headset's pixel density and reads it back with OCR.

use bevy::prelude::*;

use crate::raid::bench::ScreenText;

/// Pixels across one eye's render target at the auto render scale
/// (`rig::comfort::Comfort::AUTO_EYE_PX`).
pub const AUTO_EYE_PX: f32 = crate::rig::comfort::Comfort::AUTO_EYE_PX;

/// A Bigscreen Beyond 2 eye's frustum, tan(left) + tan(right): about 96
/// degrees across (Bigscreen quotes 108 x 96 degrees; the panels are
/// square).
pub const EYE_TAN_SPAN: f32 = 2.22;

/// Pixels a degree at the middle of the view, at the auto render scale:
/// the render target, not the panel (2560 px, 32 px/deg at best), limits
/// it. A rectilinear target has its fewest pixels a degree in the middle,
/// where you read.
pub fn eye_px_per_deg() -> f32 {
    AUTO_EYE_PX / EYE_TAN_SPAN * std::f32::consts::PI / 180.0
}

/// The floor for text you must read: smallest letters at least this tall
/// (degrees). About 8 pixels of x-height at the auto render scale.
pub const MUST_READ_DEG: f32 = 0.55;
/// The floor for incidental text (degrees): about 5 pixels.
pub const INCIDENTAL_DEG: f32 = 0.35;

/// The panels' font (Bevy's default, Fira Mono): x-height and cap height
/// as shares of the em.
pub const X_HEIGHT: f32 = 0.528;
pub const CAP_HEIGHT: f32 = 0.69;

/// The thinnest stroke worth drawing at the reading distance (degrees):
/// one eye pixel.
pub fn min_stroke_deg() -> f32 {
    1.0 / eye_px_per_deg()
}

/// The height of `text`'s smallest letters for a font `em` tall.
pub fn letter_height(text: &str, em: f32) -> f32 {
    if text.chars().any(char::is_lowercase) {
        em * X_HEIGHT
    } else {
        em * CAP_HEIGHT
    }
}

/// The angle (degrees) `h` metres subtends at `at` metres, seen `tilt`
/// degrees off square.
pub fn angle(h: f32, at: f32, tilt: f32) -> f32 {
    (h * tilt.to_radians().cos() / at).atan().to_degrees()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    MustRead,
    Incidental,
}

impl Need {
    pub fn floor(self) -> f32 {
        match self {
            Need::MustRead => MUST_READ_DEG,
            Need::Incidental => INCIDENTAL_DEG,
        }
    }
}

/// How a text is read: from how far (eye to text, m), how far off square
/// (degrees: the reader is that far toward the panel's bottom edge, as
/// you are below a sign or at the near edge of a note on a table), and in
/// what pose.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub need: Need,
    pub at: f32,
    pub tilt: f32,
    pub pose: &'static str,
}

const fn read(need: Need, at: f32, tilt: f32, pose: &'static str) -> Reading {
    Reading {
        need,
        at,
        tilt,
        pose,
    }
}

/// What a [`ScreenText`] panel is. Every panel carries one: a panel
/// without it fails the gauge.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextSource {
    /// The readout on the left glove's cuff.
    Wrist,
    /// The field radio's little screen.
    Radio,
    /// The bench's display, on the canopy over it.
    BenchDisplay,
    /// A note on a table, shelf or floor.
    Note,
    /// A signpost's board.
    Sign,
    /// A terminal's screen.
    Terminal,
    /// The driver's note on the truck's left wall.
    DriversNote,
    /// The list of the taken on the memorial plank.
    MemorialNote,
    /// The camp board's title.
    CampTitle,
    /// The camp board's career sheet.
    CampSheet,
    /// A card you press (camp, comfort, route, yard).
    Card,
    /// The logbook card.
    Logbook,
    /// The driver's request card and the order card under it.
    Request,
    /// A note's name pinned on the cork board.
    PinnedNote,
    /// A valley's name over its column on the cork board.
    BoardColumn,
    /// A cooking station's label strip.
    KitchenLabel,
    /// The depot lock's screen.
    LockScreen,
    /// The last page, on the depot lock's pedestal.
    Epilogue,
    /// A training yard station's board.
    StationBoard,
    /// The yard's list of stations by the tailgate.
    TrainingList,
    /// The yard's first board, hung in the truck over the crate.
    TruckBoard,
    /// A dev mock-up (screenshot poses): not in the game.
    Dev,
}

impl TextSource {
    pub const ALL: [TextSource; 22] = [
        TextSource::Wrist,
        TextSource::Radio,
        TextSource::BenchDisplay,
        TextSource::Note,
        TextSource::Sign,
        TextSource::Terminal,
        TextSource::DriversNote,
        TextSource::MemorialNote,
        TextSource::CampTitle,
        TextSource::CampSheet,
        TextSource::Card,
        TextSource::Logbook,
        TextSource::Request,
        TextSource::PinnedNote,
        TextSource::BoardColumn,
        TextSource::KitchenLabel,
        TextSource::LockScreen,
        TextSource::Epilogue,
        TextSource::StationBoard,
        TextSource::TrainingList,
        TextSource::TruckBoard,
        TextSource::Dev,
    ];

    pub fn reading(self) -> Reading {
        use Need::*;
        match self {
            TextSource::Wrist => read(MustRead, 0.4, 0.0, "wrist raised and turned, like a watch"),
            TextSource::Radio => read(MustRead, 0.35, 0.0, "radio held up to the face"),
            TextSource::BenchDisplay => read(MustRead, 0.9, 15.0, "standing at the bench"),
            TextSource::Note => read(MustRead, 0.65, 20.0, "leaning over the table"),
            TextSource::Sign => read(MustRead, 3.0, 0.0, "a few steps from the post"),
            TextSource::Terminal => read(MustRead, 0.8, 10.0, "standing at the desk"),
            TextSource::DriversNote => read(MustRead, 0.8, 0.0, "standing at the bed's left wall"),
            TextSource::MemorialNote => read(Incidental, 0.8, 0.0, "standing at the front wall"),
            TextSource::CampTitle => read(Incidental, 1.2, 15.0, "standing at the camp table"),
            TextSource::CampSheet => read(MustRead, 1.0, 15.0, "standing at the camp table"),
            TextSource::Card => read(MustRead, 0.75, 0.0, "within reach of its button"),
            TextSource::Logbook => read(MustRead, 0.75, 0.0, "within reach, on the bed's wall"),
            TextSource::Request => read(MustRead, 0.75, 0.0, "within reach, on the bed's wall"),
            TextSource::PinnedNote => read(MustRead, 0.6, 0.0, "leaning in to the cork board"),
            TextSource::BoardColumn => read(MustRead, 0.8, 0.0, "at the cork board"),
            TextSource::KitchenLabel => read(MustRead, 0.8, 20.0, "standing at the cooking table"),
            TextSource::LockScreen => read(MustRead, 0.8, 0.0, "standing at the lock"),
            TextSource::Epilogue => read(MustRead, 0.8, 30.0, "standing at the pedestal"),
            TextSource::StationBoard => read(MustRead, 2.0, 0.0, "at the station, by its post"),
            TextSource::TrainingList => {
                read(MustRead, 2.0, 0.0, "in the truck bed, by the tailgate")
            }
            TextSource::TruckBoard => {
                read(MustRead, 1.2, 10.0, "standing in the bed, at the crate")
            }
            TextSource::Dev => read(Incidental, 0.4, 0.0, "screenshots only"),
        }
    }
}

/// What a map marker's panel is, from `world::notes::marker_panels`' names.
pub fn marker_source(what: &str) -> Option<TextSource> {
    match what.split(' ').next()? {
        "note" => Some(TextSource::Note),
        "sign" => Some(TextSource::Sign),
        "terminal" => Some(TextSource::Terminal),
        _ => None,
    }
}

/// A panel's smallest letters (degrees) from its reading pose.
pub fn panel_deg(screen: &ScreenText, source: TextSource) -> f32 {
    let r = source.reading();
    let em = crate::render::screen::em_metres(screen);
    angle(letter_height(&screen.text, em), r.at, r.tilt)
}

/// Checks a panel against its floor.
pub fn check_panel(screen: &ScreenText, source: TextSource) -> Result<f32, String> {
    let r = source.reading();
    let deg = panel_deg(screen, source);
    if deg + 1e-4 < r.need.floor() {
        let em = crate::render::screen::em_metres(screen);
        return Err(format!(
            "{source:?} ({:.2}x{:.2} m, {:?}): {:.1} mm letters, {deg:.2} deg at {} m \
             (floor {}): {:?}",
            screen.size.x,
            screen.size.y,
            screen.style,
            letter_height(&screen.text, em) * 1000.0,
            r.at,
            r.need.floor(),
            screen.text.lines().next().unwrap_or("")
        ));
    }
    Ok(deg)
}

/// A model with stencil letters (`text()` in `art/items/_kit.py`: all
/// capitals, `h` is the cap height, strokes 0.15 h but at least 0.5 mm).
pub struct ArtText {
    /// The script, relative to `art/`.
    pub script: &'static str,
    pub reading: Reading,
}

const fn art(script: &'static str, need: Need, at: f32, pose: &'static str) -> ArtText {
    ArtText {
        script,
        reading: read(need, at, 0.0, pose),
    }
}

/// Every model with letters on it. A script that calls `text()` and isn't
/// here fails the gauge.
pub const ART: &[ArtText] = &[
    art(
        "guns/ak74.py",
        Need::Incidental,
        0.35,
        "the gun turned to the face",
    ),
    art(
        "guns/pm_pistol.py",
        Need::Incidental,
        0.35,
        "the gun turned to the face",
    ),
    art(
        "guns/ppsh.py",
        Need::Incidental,
        0.35,
        "the gun turned to the face",
    ),
    art(
        "guns/sks.py",
        Need::Incidental,
        0.35,
        "the gun turned to the face",
    ),
    art(
        "guns/nagant.py",
        Need::Incidental,
        0.35,
        "the gun turned to the face",
    ),
    art(
        "mags/ak_mag.py",
        Need::Incidental,
        0.35,
        "the magazine in the hand",
    ),
    art(
        "mags/ppsh_drum.py",
        Need::Incidental,
        0.35,
        "the drum in the hand",
    ),
    art(
        "items/ammo.py",
        Need::MustRead,
        0.35,
        "the box in the hand: which rounds",
    ),
    art(
        "items/food.py",
        Need::Incidental,
        0.35,
        "the tin in the hand",
    ),
    art(
        "items/grenades.py",
        Need::Incidental,
        0.35,
        "the grenade in the hand",
    ),
    art(
        "items/materials.py",
        Need::Incidental,
        0.35,
        "the tin in the hand",
    ),
    art(
        "items/supplies.py",
        Need::Incidental,
        1.0,
        "the can on the ground",
    ),
    art(
        "items/curios.py",
        Need::MustRead,
        0.35,
        "the key in the hand: which valley",
    ),
    art(
        "vehicles/truck.py",
        Need::Incidental,
        1.5,
        "in the truck bed",
    ),
    art("vehicles/bench.py", Need::Incidental, 0.7, "at the bench"),
    art(
        "vehicles/stash_crate.py",
        Need::Incidental,
        1.0,
        "in the truck bed",
    ),
    art(
        "props/depot.py",
        Need::Incidental,
        5.0,
        "walking through the depot",
    ),
];

/// Text baked into a texture.
pub struct BakedText {
    pub what: &'static str,
    /// Smallest letters' height (m) on the thing.
    pub letter: f32,
    /// Texels across those letters.
    pub texels: f32,
    pub reading: Reading,
}

/// Texture-baked text, from `scripts/field_map.py`'s source: it states its
/// letter sizes in millimetres (`*_MM`).
pub fn baked(src: &str) -> Vec<BakedText> {
    let mm = |name: &str| -> f32 {
        src.lines()
            .find_map(|l| l.strip_prefix(&format!("{name} = ")))
            .and_then(|v| v.split('#').next()?.trim().parse().ok())
            .unwrap_or(0.0)
    };
    let side = mm("MAP_MM");
    let texels = mm("MAP_PX");
    let held = read(
        Need::MustRead,
        0.3,
        15.0,
        "the map case held up to the face",
    );
    ["TITLE_MM", "NAME_MM", "LABEL_MM"]
        .into_iter()
        .map(|k| {
            // Mixed case: the x-height (DejaVu Serif: 0.52 em).
            let letter = mm(k) * 0.52 / 1000.0;
            BakedText {
                what: match k {
                    "TITLE_MM" => "field map: title",
                    "NAME_MM" => "field map: place names",
                    _ => "field map: labels (RV, keep away)",
                },
                letter,
                texels: if side > 0.0 {
                    letter * 1000.0 / side * texels
                } else {
                    0.0
                },
                reading: held,
            }
        })
        .collect()
}

/// Stencil `text()` calls in an art script: (line, string, cap height m).
pub fn stencil_calls(src: &str) -> Vec<(usize, String, Option<f32>)> {
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let l = line.trim_start();
        if l.starts_with("def ") || l.starts_with('#') {
            continue;
        }
        for start in ["text(", "text_round("] {
            let mut rest = l;
            while let Some(k) = rest.find(start) {
                let before = rest[..k].chars().last();
                rest = &rest[k + start.len()..];
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                    continue;
                }
                let args: Vec<&str> = rest.splitn(4, ',').collect();
                if args.len() < 3 {
                    continue;
                }
                let h = args[2].trim().parse::<f32>().ok();
                out.push((i + 1, args[1].trim().to_string(), h));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_headset_has_about_fourteen_pixels_a_degree() {
        let ppd = eye_px_per_deg();
        assert!((13.0..16.0).contains(&ppd), "{ppd}");
        // The floors in pixels of x-height.
        assert!(MUST_READ_DEG * ppd >= 7.5);
        assert!(INCIDENTAL_DEG * ppd >= 4.8);
    }

    #[test]
    fn lower_case_is_measured_by_its_x_height() {
        assert_eq!(letter_height("ABC 12", 1.0), CAP_HEIGHT);
        assert_eq!(letter_height("Abc", 1.0), X_HEIGHT);
    }

    #[test]
    fn stencil_calls_are_found() {
        let src = "def text(p, s, h):\n    text(p, \"9x18\", 0.0065, \"Black\")\n    \
                   text_round(p, name, 0.005, m, r)\n    p.text(1)\n";
        let c = stencil_calls(src);
        assert_eq!(c.len(), 2, "{c:?}");
        assert_eq!(c[0].2, Some(0.0065));
        assert_eq!(c[1].1, "name");
    }

    /// Every panel in the game (each valley, the yard, the camp, the
    /// truck), as fitted to its words, clears its floor from where it's
    /// read; every panel says what it is. Notes, signs and terminals are
    /// put up by the client: they're taken from the maps. The wrist and the
    /// radio, the client's too, at their fullest.
    #[test]
    fn every_panel_in_the_game_clears_its_floor() {
        use crate::raid::bench::ScreenStyle;
        let mut bad = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut check = |what: &str, screen: &ScreenText, source: Option<TextSource>| {
            if screen.text.trim().is_empty() {
                return;
            }
            let Some(source) = source else {
                bad.push(format!("{what}: no TextSource"));
                return;
            };
            seen.insert(source);
            if let Err(e) = check_panel(screen, source) {
                bad.push(format!("{what}: {e}"));
            }
        };
        for (map, camp) in [
            ("training_yard", false),
            ("marrow_creek", false),
            ("kettle_pass", false),
            ("cinder_works", false),
            ("marrow_creek", true),
        ] {
            let dir = std::env::temp_dir().join(format!(
                "extraction-legibility-{map}-{camp}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            let opts = crate::LaunchOptions {
                robots: false,
                map: map.into(),
                camp,
                data_dir: Some(dir.clone()),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..60 {
                app.update();
            }
            let zone = app.world().resource::<crate::world::Zone>().clone();
            for (what, screen) in crate::world::notes::marker_panels(&zone.map) {
                check(&format!("{map}: {what}"), &screen, marker_source(&what));
            }
            let mut q = app
                .world_mut()
                .query::<(Option<&Name>, &ScreenText, Option<&TextSource>)>();
            for (name, screen, source) in q.iter(app.world()) {
                let name = name.map_or("(unnamed)".into(), |n| n.to_string());
                check(&format!("{map}: {name}"), screen, source.copied());
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        let wrist = ScreenText {
            text: crate::render::comfort::wrist_text(
                "SURGE 12M",
                14.2,
                100.0,
                true,
                Some(&"The driver says something long enough to fill the screen. ".repeat(3)),
                0,
            ),
            size: crate::render::comfort::WRIST_SIZE,
            style: ScreenStyle::Lcd,
        };
        check("wrist, full", &wrist, Some(TextSource::Wrist));
        let radio = ScreenText {
            text: crate::render::comfort::radio_text(
                &[
                    "Response team sent, heading your way now".into(),
                    "Truck is on its way to the rendezvous".into(),
                ],
                0,
            ),
            size: crate::render::comfort::RADIO_SIZE,
            style: ScreenStyle::Lcd,
        };
        check("radio, full", &radio, Some(TextSource::Radio));
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        assert!(seen.len() >= 15, "sources seen: {seen:?}");
    }

    /// Every model's letters: big enough from where they're read, strokes
    /// at least a pixel, and every script with letters registered.
    #[test]
    fn every_model_with_letters_is_registered_and_legible() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("art");
        let mut bad = Vec::new();
        let mut seen = 0;
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "py") {
                    continue;
                }
                let rel = p
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                if rel == "items/_kit.py" {
                    continue;
                }
                let calls = stencil_calls(&std::fs::read_to_string(&p).unwrap());
                if calls.is_empty() {
                    continue;
                }
                let Some(entry) = ART.iter().find(|a| a.script == rel) else {
                    bad.push(format!("{rel}: has letters but isn't in legibility::ART"));
                    continue;
                };
                let r = entry.reading;
                for (line, s, h) in calls {
                    seen += 1;
                    let Some(h) = h else {
                        bad.push(format!("{rel}:{line}: {s}: height isn't a number"));
                        continue;
                    };
                    let deg = angle(h, r.at, r.tilt);
                    let stroke = angle((h * 0.15).max(0.0005), r.at, r.tilt);
                    if deg < r.need.floor() {
                        bad.push(format!(
                            "{rel}:{line}: {s} {:.1} mm: {deg:.2} deg at {} m (floor {})",
                            h * 1000.0,
                            r.at,
                            r.need.floor()
                        ));
                    }
                    if stroke < min_stroke_deg() {
                        bad.push(format!("{rel}:{line}: {s}: strokes under a pixel"));
                    }
                }
            }
        }
        assert!(seen > 40, "stencil calls seen: {seen}");
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Texture-baked text: big enough, and enough texels a letter that the
    /// texture isn't what blurs it (8 across an x-height).
    #[test]
    fn baked_text_is_legible() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/field_map.py");
        let b = baked(&std::fs::read_to_string(src).unwrap());
        assert_eq!(b.len(), 3);
        let mut bad = Vec::new();
        for t in &b {
            let r = t.reading;
            let deg = angle(t.letter, r.at, r.tilt);
            if deg < r.need.floor() {
                bad.push(format!(
                    "{}: {:.1} mm, {deg:.2} deg at {} m (floor {})",
                    t.what,
                    t.letter * 1000.0,
                    r.at,
                    r.need.floor()
                ));
            }
            if t.texels < 8.0 {
                bad.push(format!("{}: {:.1} texels a letter", t.what, t.texels));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
