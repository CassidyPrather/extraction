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
/// you are below a sign or at the near edge of a note on a table; less
/// than zero, toward its top edge, as you are over a label on a table),
/// and in what pose.
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
    /// The headset's loading view: the valley's name, or where to.
    Loading,
    /// A dev mock-up (screenshot poses): not in the game.
    Dev,
}

impl TextSource {
    pub const ALL: [TextSource; 23] = [
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
        TextSource::Loading,
        TextSource::Dev,
    ];

    pub fn reading(self) -> Reading {
        use Need::*;
        match self {
            TextSource::Wrist => read(MustRead, 0.4, 0.0, "wrist raised and turned, like a watch"),
            TextSource::Radio => read(MustRead, 0.35, 0.0, "radio held up to the face"),
            TextSource::BenchDisplay => read(MustRead, 0.9, -10.0, "standing at the bench"),
            TextSource::Note => read(MustRead, 0.65, 20.0, "leaning over the table"),
            TextSource::Sign => read(MustRead, 3.0, 0.0, "a few steps from the post"),
            TextSource::Terminal => read(
                MustRead,
                0.7,
                -35.0,
                "within reach of its button, looking down",
            ),
            TextSource::DriversNote => read(MustRead, 0.8, 0.0, "standing at the bed's left wall"),
            TextSource::MemorialNote => read(Incidental, 0.8, 0.0, "standing at the front wall"),
            TextSource::CampTitle => read(Incidental, 1.2, -12.0, "standing at the camp table"),
            TextSource::CampSheet => read(MustRead, 1.0, -12.0, "standing at the camp table"),
            TextSource::Card => read(MustRead, 0.75, 0.0, "within reach of its button"),
            TextSource::Logbook => read(MustRead, 0.75, 0.0, "within reach, on the bed's wall"),
            TextSource::Request => read(MustRead, 0.75, 0.0, "within reach, on the bed's wall"),
            TextSource::PinnedNote => read(MustRead, 0.6, 0.0, "leaning in to the cork board"),
            TextSource::BoardColumn => read(MustRead, 0.8, 0.0, "at the cork board"),
            TextSource::KitchenLabel => read(MustRead, 0.8, -20.0, "standing at the cooking table"),
            TextSource::LockScreen => read(MustRead, 0.8, 0.0, "standing at the lock"),
            TextSource::Epilogue => read(MustRead, 0.8, 30.0, "standing at the pedestal"),
            TextSource::StationBoard => read(MustRead, 2.0, 0.0, "at the station, by its post"),
            TextSource::TrainingList => {
                read(MustRead, 2.0, 0.0, "in the truck bed, by the tailgate")
            }
            TextSource::TruckBoard => {
                read(MustRead, 1.2, 10.0, "standing in the bed, at the crate")
            }
            TextSource::Loading => read(MustRead, 2.0, 0.0, "in the headset, between valleys"),
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
        "guns/mp133.py",
        Need::Incidental,
        0.35,
        "the gun turned to the face",
    ),
    art(
        "guns/rpk74.py",
        Need::Incidental,
        0.35,
        "the gun turned to the face",
    ),
    art(
        "guns/flare_pistol.py",
        Need::Incidental,
        0.35,
        "the gun turned to the face",
    ),
    art(
        "guns/mosin.py",
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
        "mags/rpk_mag.py",
        Need::Incidental,
        0.35,
        "the magazine in the hand",
    ),
    art(
        "items/ammo_22.py",
        Need::MustRead,
        0.35,
        "the box in the hand: which rounds",
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
    // A charcoal burner's name on his bed, a shop's CLOSED card, a
    // chapel's hymn numbers: read across the room (World models).
    art("props/named.py", Need::MustRead, 2.0, "across the room"),
    // A cassette player's nameplate and keys, a tape's A/B, a 78's label
    // (Radio & music): read in the hand.
    art("items/music.py", Need::Incidental, 0.35, "held in the hand"),
    // The piano's and harmonium's names on their name rails: standing at
    // the keys.
    art(
        "props/music.py",
        Need::Incidental,
        0.7,
        "standing at the keys",
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

/// A `text()` height: a number, or `A if ... else B` of two (the smaller).
fn height(arg: &str) -> Option<f32> {
    if let Ok(h) = arg.parse::<f32>() {
        return Some(h);
    }
    let (a, rest) = arg.split_once(" if ")?;
    let (_, b) = rest.split_once(" else ")?;
    Some(
        a.trim()
            .parse::<f32>()
            .ok()?
            .min(b.trim().parse::<f32>().ok()?),
    )
}

/// A computed height's smallest value, stated by a `# smallest: H` comment
/// on the call's line, the one after or the one before (looked for in that
/// order).
fn smallest(src: &str, line: usize) -> Option<f32> {
    let lines: Vec<&str> = src.lines().collect();
    [Some(line), Some(line + 1), line.checked_sub(1)]
        .into_iter()
        .flatten()
        .filter_map(|i| lines.get(i))
        .find_map(|l| {
            l.split_once("# smallest:")
                .and_then(|(_, v)| v.split_whitespace().next()?.parse().ok())
        })
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
                let h = height(args[2].trim()).or_else(|| smallest(src, i));
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

    /// The floors are set for the auto render scale; the SHARPNESS card's
    /// lowest step mustn't render a Beyond 2 (2980 px an eye asked for)
    /// much below it.
    #[test]
    fn no_sharpness_step_goes_under_the_floors_density() {
        use crate::rig::comfort::Comfort;
        let lowest = Comfort::RENDER_SCALES
            .iter()
            .copied()
            .filter(|s| *s > 0.0)
            .fold(1.0f32, f32::min);
        assert!(
            lowest * 2980.0 >= AUTO_EYE_PX * 0.97,
            "lowest step {lowest}: {} px an eye",
            lowest * 2980.0
        );
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
        assert_eq!(height("0.0075 if len(label) <= 4 else 0.006"), Some(0.006));
        let src = "    text(p, s, h1 * 0.8, m,\n         depth=0.0003)  # smallest: 0.0048\n";
        assert_eq!(stencil_calls(src)[0].2, Some(0.0048));
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
                Some(crate::render::comfort::Said::Plain(
                    &"The driver says something long enough to fill the screen. ".repeat(3),
                )),
                0,
            ),
            size: crate::render::comfort::WRIST_SIZE,
            style: ScreenStyle::Lcd,
        };
        check("wrist, full", &wrist, Some(TextSource::Wrist));
        // Captioned: the speaker's line in place of one of the words'.
        let wrist = ScreenText {
            text: crate::render::comfort::wrist_text(
                "SURGE 12M",
                14.2,
                100.0,
                true,
                Some(crate::render::comfort::Said::Captioned(
                    "MACHINES AT 12",
                    &"The driver says something long enough to fill the screen. ".repeat(3),
                )),
                0,
            ),
            size: crate::render::comfort::WRIST_SIZE,
            style: ScreenStyle::Lcd,
        };
        check("wrist, captioned", &wrist, Some(TextSource::Wrist));
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

    /// Paper indoors is lit by the sky's share of light through the walls
    /// (the ambient, dimmed while you're in a room). Going underground and
    /// then in and out of buildings must leave it where it was: the
    /// underground's darkening once took the room's dimmed ambient for the
    /// sky's and wrote it back, so it ran down to nothing within a few
    /// rooms and every note indoors went black for the rest of the raid
    /// (the render gauge found it).
    #[test]
    fn a_trip_underground_and_through_rooms_leaves_the_light_alone() {
        use bevy::render::RenderPlugin;
        use bevy::render::settings::WgpuSettings;
        let dir = std::env::temp_dir().join(format!("extraction-ambient-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(bevy::window::WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        backends: None,
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: crate::asset_root_path().to_string_lossy().into_owned(),
                    ..default()
                })
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<bevy::log::LogPlugin>(),
        );
        app.insert_resource(crate::LaunchOptions {
            flat: true,
            map: "marrow_creek".into(),
            map_explicit: true,
            new_game: true,
            robots: false,
            data_dir: Some(dir.clone()),
            weather: Some("clearing".into()),
            camera: Some((Vec3::new(0.0, 0.0, 0.0), 0.0, 0.0)),
            ..default()
        })
        .insert_resource(Time::<Fixed>::from_hz(90.0))
        .add_plugins(bevy::render::sync_world::SyncWorldPlugin)
        .add_plugins(avian3d::prelude::PhysicsPlugins::default())
        .add_plugins((crate::GamePlugin, crate::ClientPlugin));
        app.finish();
        app.cleanup();
        for _ in 0..60 {
            app.update();
        }
        let zone = app.world().resource::<crate::world::Zone>().clone();
        // A spot down in the village's shelter, and one in a room.
        let site = extraction_core::underground::placed(&zone.map)
            .into_iter()
            .next()
            .expect("an underground site");
        let mut below = None;
        'find: for dx in -20..=20 {
            for dz in -20..=20 {
                let (x, z) = (site.at[0] + dx as f32, site.at[1] + dz as f32);
                for d in [2.5, 3.5, 4.5, 6.0] {
                    let p = Vec3::new(x, zone.ground(x, z) - d, z);
                    if crate::world::underground::floor_under(&zone, p).is_some() {
                        below = Some(p);
                        break 'find;
                    }
                }
            }
        }
        let below = below.expect("a spot in the shelter");
        let rooms: Vec<Vec3> = app
            .world()
            .resource::<crate::robots::doors::Entrances>()
            .shells
            .iter()
            .take(4)
            .map(|s| s.xf.translation + Vec3::Y * 1.2)
            .collect();
        assert!(!rooms.is_empty(), "no buildings");
        let outside = zone.on_ground(site.at[0] + 30.0, site.at[1] + 30.0) + Vec3::Y * 1.7;
        let go = |app: &mut App, at: Vec3, frames: usize| {
            {
                let w = app.world_mut();
                let mut q = w.query_filtered::<&mut Transform, With<crate::render::FreeCamera>>();
                *q.single_mut(w).unwrap() = Transform::from_translation(at);
                // Without a renderer the impostors' capture cameras never
                // finish: they'd be taken for the eye.
                let mut cams =
                    w.query_filtered::<&mut Camera, Without<crate::render::FreeCamera>>();
                for mut c in cams.iter_mut(w) {
                    c.is_active = false;
                }
            }
            for _ in 0..frames {
                app.update();
            }
        };
        // Frames enough for each change to settle (the depth eases in at
        // 0.15 a frame, the eyes' indoors over 0.4 s) and no more: under
        // the suite's load this ran 203 s at twice the frames
        // (`scripts/test_times.py`).
        go(&mut app, outside, 30);
        let day = app.world().resource::<GlobalAmbientLight>().brightness;
        assert!(day > 100.0, "{day}");
        go(&mut app, below, 40);
        go(&mut app, outside, 30);
        for room in rooms.iter().cycle().take(5) {
            go(&mut app, *room, 20);
            go(&mut app, outside, 20);
        }
        go(&mut app, outside, 60);
        let after = app.world().resource::<GlobalAmbientLight>().brightness;
        // Which way round two systems run is the scheduler's choice unless
        // it's pinned, so this run may have got the good order by luck:
        // nothing that touches the ambient where the room's dimming is put
        // on and taken off may run in no set order with another.
        let amb = app
            .world()
            .components()
            .component_id::<GlobalAmbientLight>()
            .expect("the ambient");
        let schedules = app.world().resource::<bevy::ecs::schedule::Schedules>();
        use bevy::ecs::schedule::ScheduleLabel;
        for label in [First.intern(), PostUpdate.intern()] {
            let s = schedules.get(label).unwrap();
            let loose: Vec<_> = s
                .graph()
                .conflicting_systems()
                .0
                .iter()
                .filter(|(_, _, ids)| ids.contains(&amb))
                .collect();
            assert!(
                loose.is_empty(),
                "{label:?}: systems touching the ambient in no set order: {loose:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            after > day * 0.95,
            "the ambient ran down from {day:.0} to {after:.0}"
        );
    }

    /// Nothing in the truck covers a panel or fights it: no housing (a
    /// slat, the cork's face) and no other panel within 5 cm in front of
    /// one where they overlap, and no two parallel surfaces that overlap
    /// closer than 3 mm (they z-fight). Playtest 2026-10-28, F4: the
    /// driver's note shared the comfort cards' plane, the side slat ran
    /// over the cards' edges, the column heads lay 2 mm off the cork.
    /// The slats are `art/vehicles/truck.py`'s; the cork's face is
    /// `story_props.py`'s, 1 cm out.
    #[test]
    fn nothing_covers_a_panel_in_the_truck_or_fights_it() {
        use crate::raid::bench::ScreenText;
        use crate::raid::truck::{FLOOR, Truck};
        let mut app = crate::headless_app(Default::default(), false);
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
        if !app.world().contains_resource::<Assets<Mesh>>() {
            app.init_asset::<Mesh>();
        }
        if !app.world().contains_resource::<Assets<StandardMaterial>>() {
            app.init_asset::<StandardMaterial>();
        }
        app.init_resource::<crate::render::materials::MaterialLibrary>()
            .add_plugins(crate::render::memorial::MemorialBoardPlugin);
        app.finish();
        app.cleanup();
        for _ in 0..6 {
            app.update();
        }
        // (what, truck-space pose of its face (+Z out), its size, written)
        let mut faces: Vec<(String, Transform, Vec2, bool)> = Vec::new();
        let w = app.world_mut();
        let truck = w
            .query_filtered::<&GlobalTransform, With<Truck>>()
            .iter(w)
            .next()
            .expect("the truck")
            .affine()
            .inverse();
        let local = |g: &GlobalTransform| Transform::from_matrix((truck * g.affine()).into());
        for (text, g, name) in w
            .query::<(&ScreenText, &GlobalTransform, Option<&Name>)>()
            .iter(w)
        {
            let at = local(g);
            if at.translation.x.abs() > 1.3 || at.translation.z.abs() > 2.0 {
                continue;
            }
            let what = format!(
                "{} {:?}",
                name.map_or("?".into(), |n| n.to_string()),
                text.text.lines().next().unwrap_or("")
            );
            faces.push((what, at, text.size, true));
        }
        for g in w
            .query_filtered::<&GlobalTransform, With<crate::raid::board::CorkBoard>>()
            .iter(w)
        {
            let at = local(g) * Transform::from_xyz(0.0, 0.0, 0.01);
            faces.push(("the cork".into(), at, crate::raid::board::BOARD, false));
        }
        for sx in [-1.0f32, 1.0] {
            // A side slat's inside face, facing into the bed.
            let at = Transform::from_xyz(sx * 1.1125, FLOOR + 1.35, 0.0)
                .with_rotation(Quat::from_rotation_y(-sx * std::f32::consts::FRAC_PI_2));
            faces.push(("a side slat".into(), at, Vec2::new(3.6, 0.07), false));
        }
        assert!(faces.iter().filter(|f| f.3).count() >= 13);
        let mut bad = Vec::new();
        for (i, a) in faces.iter().enumerate() {
            for (j, b) in faces.iter().enumerate() {
                if i == j {
                    continue;
                }
                let n = a.1.rotation * Vec3::Z;
                if n.dot(b.1.rotation * Vec3::Z).abs() < 0.99 {
                    continue;
                }
                // b's rect, in a's plane: do they overlap (by more than a
                // millimetre)?
                let into = a.1.compute_affine().inverse();
                let (lo, hi) = (0..4).fold((Vec2::MAX, Vec2::MIN), |(lo, hi), k| {
                    let c = Vec3::new(
                        if k & 1 == 0 { -0.5 } else { 0.5 } * b.2.x,
                        if k & 2 == 0 { -0.5 } else { 0.5 } * b.2.y,
                        0.0,
                    );
                    let p = into.transform_point3(b.1.transform_point(c)).truncate();
                    (lo.min(p), hi.max(p))
                });
                let half = a.2 * 0.5;
                let overlap = (hi.x.min(half.x) - lo.x.max(-half.x))
                    .min(hi.y.min(half.y) - lo.y.max(-half.y));
                if overlap <= 0.001 {
                    continue;
                }
                let d = (b.1.translation - a.1.translation).dot(n);
                if i < j && d.abs() < 0.003 {
                    bad.push(format!(
                        "{} and {} are {:.1} mm apart: they fight",
                        a.0,
                        b.0,
                        d.abs() * 1000.0
                    ));
                } else if a.3 && d >= 0.003 && d < 0.05 {
                    bad.push(format!(
                        "{} is covered by {}, {:.1} mm in front of it",
                        a.0,
                        b.0,
                        d * 1000.0
                    ));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Nothing in a staged scene lies on its note: a lantern or a photo on
    /// the paper hides its words (the render gauge found the shelter's
    /// note half under its lantern).
    #[test]
    fn nothing_lies_on_a_scenes_note() {
        use crate::world::scenes::Piece;
        let mut bad = Vec::new();
        let half = crate::world::notes::NOTE * 0.5;
        for scene in crate::world::scenes::all() {
            let Some(note) = &scene.note else {
                continue;
            };
            let rot = Quat::from_rotation_y(-note.rot.to_radians());
            let on_paper = |p: [f32; 3]| {
                let d = rot.inverse()
                    * Vec3::new(p[0] - note.at[0], p[1] - note.at[1], p[2] - note.at[2]);
                // A thing's own footprint: a few centimetres round its point.
                d.y.abs() < 0.15 && d.x.abs() < half.x + 0.04 && d.z.abs() < half.y + 0.04
            };
            for piece in &scene.pieces {
                if let Piece::Lantern { at } = piece {
                    if on_paper(*at) {
                        bad.push(format!("{}: a lantern on the note", scene.name));
                    }
                }
            }
            for thing in &scene.items {
                if on_paper(thing.at) {
                    bad.push(format!("{}: {} on the note", scene.name, thing.id));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// No loot lands on a note: loot rolls round a spot (tens of
    /// centimetres), and notes lie beside loot spots on the same tables; a
    /// torch on the pump log hid half its words. Every roll a spot can
    /// make, kept off the notes (`items::loot::clear_of_notes`), lands
    /// clear of every note's paper.
    #[test]
    fn no_loot_lands_on_a_note() {
        use crate::items::loot::{building_roll, clear_of_notes, marker_roll, notes_lying};
        use extraction_core::map::MarkerKind;
        let mut bad = Vec::new();
        let half = crate::world::notes::NOTE * 0.5 + Vec2::splat(0.06);
        for map in [
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "training_yard",
        ] {
            let opts = crate::LaunchOptions {
                robots: false,
                map: map.into(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..5 {
                app.update();
            }
            let zone = app.world().resource::<crate::world::Zone>().clone();
            let notes = notes_lying(&zone);
            let mut landings: Vec<(Vec3, Vec3)> = Vec::new();
            if let Some(l) = app
                .world()
                .get_resource::<crate::world::buildings::BuildingLoot>()
            {
                for s in &l.0 {
                    landings.extend((0..5).map(|i| (s.at, s.at + building_roll(i))));
                }
            }
            for m in &zone.map.markers {
                if let MarkerKind::Loot { .. } = m.kind {
                    if m.y >= 0.3 {
                        let base = zone.on_ground(m.at[0], m.at[1]) + Vec3::Y * (m.y + 0.1);
                        landings.extend((0..5).map(|i| (base, base + marker_roll(i))));
                    }
                }
            }
            for (spot, at) in landings {
                let landed = clear_of_notes(at, &notes);
                for (note, rot) in &notes {
                    let p = rot.inverse() * (landed - *note);
                    if p.y.abs() < 0.4 && p.x.abs() < half.x && p.z.abs() < half.y {
                        bad.push(format!(
                            "{map}: a roll at {spot:.2} lands on the note at {note:.2}"
                        ));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        // And the keeping-off itself, on a roll laid right on a turned
        // note (loot spots are also moved off notes at the source,
        // `world::buildings`, so the valleys may never give it one).
        let note = (Vec3::new(3.0, 0.8, -2.0), Quat::from_rotation_y(0.7));
        for off in [
            Vec3::ZERO,
            Vec3::new(0.05, 0.02, -0.1),
            Vec3::new(-0.12, 0.0, 0.15),
        ] {
            let at = note.0 + note.1 * off;
            let p = note.1.inverse() * (clear_of_notes(at, &[note]) - note.0);
            assert!(
                p.x.abs() >= half.x || p.z.abs() >= half.y,
                "a roll at {off:?} on the note stayed on it: {p:?}"
            );
        }
    }

    /// Every signpost faces the way you come to it, so its words meet you
    /// rather than its back: beside a road or path (within 25 m), its face
    /// turns to the road 10 m back toward where the truck sets you down,
    /// whence you walk up it; with none near, toward that spot itself. Its board is at eye height by construction (1.45 m
    /// up, `world::notes`).
    #[test]
    fn every_sign_faces_its_approach() {
        use extraction_core::map::{MapDef, MarkerKind};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/maps");
        let mut bad = Vec::new();
        let mut seen = 0;
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "ron") {
                continue;
            }
            let map = MapDef::from_ron(&std::fs::read_to_string(&p).unwrap()).unwrap();
            let ins = map
                .markers_of(&MarkerKind::Insertion)
                .next()
                .map(|m| Vec2::from(m.at));
            for m in &map.markers {
                if !matches!(m.kind, MarkerKind::Sign { .. }) {
                    continue;
                }
                seen += 1;
                let at = Vec2::from(m.at);
                // The nearest point on any road or path.
                let mut near: Option<(f32, Vec2, Vec2)> = None;
                for road in &map.roads {
                    for w in road.points.windows(2) {
                        let (a, b) = (Vec2::from(w[0]), Vec2::from(w[1]));
                        let ab = b - a;
                        let t = ((at - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                        let q = a + ab * t;
                        let d = q.distance(at);
                        if near.is_none_or(|(n, _, _)| d < n) {
                            near = Some((d, q, ab.normalize_or_zero()));
                        }
                    }
                }
                let Some(ins) = ins else {
                    continue;
                };
                let from = match near {
                    // Back along the road, the way you walk up it.
                    Some((d, q, along)) if d < 25.0 => [q + along * 10.0, q - along * 10.0]
                        .into_iter()
                        .min_by(|a, b| a.distance(ins).total_cmp(&b.distance(ins)))
                        .unwrap(),
                    _ => ins,
                };
                // The board faces the marker's +Z, turned by its rot.
                let face = Quat::from_rotation_y(-m.rot.to_radians()) * Vec3::Z;
                let to = (from - at).normalize_or_zero();
                let cos = Vec2::new(face.x, face.z).dot(to);
                if to != Vec2::ZERO && cos < 0.5 {
                    bad.push(format!(
                        "{}: {} faces {:.0} deg away from its approach",
                        map.name,
                        m.name,
                        cos.clamp(-1.0, 1.0).acos().to_degrees()
                    ));
                }
            }
        }
        assert!(seen > 0, "no signs found");
        assert!(bad.is_empty(), "{}", bad.join("\n"));
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
