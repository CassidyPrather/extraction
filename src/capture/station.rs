//! The playtest tour: one new or changed thing at a time, no travel.
//!
//! `extraction --playtest assets/playtest/tour.txt` (or PLAYTEST TOUR on
//! the camp board) plays the tour's first station; each station is the
//! game launched with that station's own arguments (a valley, an hour, a
//! weather, where you stand, what's staged), so a new station is a line
//! in the list, not code. A small board by where you stand says which
//! station of how many, with NEXT and three comfort cards. NEXT asks the
//! station's question, 1 to 5 on five cards, then for what bugged you,
//! out loud for the recording; NEXT again opens the next station.
//!
//! Nothing is saved: the tour is a sandbox, as the arcade is
//! (`persist::career_path`). What it notes goes to
//! `<data dir>/playtest/tour.jsonl`, which REPORT takes: each station's
//! start (wall clock), its rating, its end. Each station's start also
//! plays a chirp and flashes the view white for two frames, so
//! scripts/playtest_ingest.py lines a recording up with the logs exactly.
//!
//! The list, one station a line:
//!
//! ```text
//! name | minutes | question | game arguments
//! ammo_boxes | 3 | How readable are the new ammo boxes? | --map training_yard --spawn 0,0,40,0 --gallery ...
//! ```

use avian3d::prelude::*;
use bevy::prelude::*;
use std::path::Path;

use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::launch::LaunchOptions;
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::raid::comfort_board::ComfortCard;
use crate::raid::truck::Pressable;
use crate::render::legibility::TextSource;
use crate::rig::LocalPlayer;
use crate::world::Layer;

/// A station of the tour.
#[derive(Clone, Debug, PartialEq)]
pub struct Station {
    pub name: String,
    /// How long it's meant to take: a nudge, not a cut.
    pub minutes: f32,
    /// Asked on leaving it, answered 1 to 5.
    pub question: String,
    /// The game's own arguments for it.
    pub args: Vec<String>,
    /// What to try, on the board under its name (the list's optional fifth
    /// field).
    pub hint: Option<String>,
}

/// The tour's stations, or what's wrong with the list (naming the line).
pub fn load(path: &Path) -> Result<Vec<Station>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('|').map(str::trim).collect();
        let at = || format!("{}:{}", path.display(), n + 1);
        if !(4..=5).contains(&f.len()) || f[0].is_empty() {
            return Err(format!(
                "{}: want name | minutes | question | game arguments [| what to try]",
                at()
            ));
        }
        let minutes = f[1]
            .parse::<f32>()
            .map_err(|_| format!("{}: minutes {:?} isn't a number", at(), f[1]))?;
        if f[3].contains("--playtest") {
            return Err(format!("{}: a station can't start a tour", at()));
        }
        out.push(Station {
            name: f[0].to_owned(),
            minutes,
            question: f[2].to_owned(),
            args: f[3].split_whitespace().map(String::from).collect(),
            hint: f.get(4).filter(|h| !h.is_empty()).map(|h| h.to_string()),
        });
    }
    if out.is_empty() {
        return Err(format!("{}: no stations", path.display()));
    }
    Ok(out)
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

/// The command line with its tour station's arguments after it (they
/// win), or as it was without `--playtest`. A list that won't read is
/// said on the console, and the game opens as if there were no tour.
pub fn with_station_args(mut args: Vec<String>) -> Vec<String> {
    let Some(path) = flag(&args, "--playtest") else {
        return args;
    };
    let index = flag(&args, "--playtest-station")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    match load(Path::new(&path)) {
        Ok(stations) => match stations.get(index) {
            Some(s) => args.extend(s.args.iter().cloned()),
            None => eprintln!("ERROR playtest: {path} has no station {index}"),
        },
        Err(e) => eprintln!("ERROR playtest: {e}"),
    }
    args
}

/// The command line for station `next`: this one's, with its station
/// number changed.
pub fn next_args(args: impl IntoIterator<Item = String>, next: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--playtest-station" {
            it.next();
            continue;
        }
        out.push(a);
    }
    out.extend(["--playtest-station".to_owned(), next.to_string()]);
    out
}

pub struct StationPlugin;

impl Plugin for StationPlugin {
    fn build(&self, app: &mut App) {
        let Some(opts) = app.world().get_resource::<LaunchOptions>().cloned() else {
            return;
        };
        let Some(path) = opts.playtest.clone() else {
            return;
        };
        let stations = match load(&path) {
            Ok(s) => s,
            Err(e) => {
                error!("playtest: {e}");
                return;
            }
        };
        if opts.playtest_station >= stations.len() {
            return;
        }
        let log = crate::persist::profile(&opts)
            .join("playtest")
            .join("tour.jsonl");
        app.insert_resource(Tour {
            tour: path.display().to_string(),
            stations,
            index: opts.playtest_station,
            phase: Phase::Playing,
            board: None,
            last_at: None,
            synced: false,
            flash: None,
            nudged: false,
            log,
        })
        .add_systems(
            Update,
            (skip_unstageable, put_up_board, sync, press_cards, board_text, nudge).chain(),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Looking at the station's thing.
    Playing,
    /// Asked the question, the five cards up.
    Rating,
    /// Rated: say what bugged you, then NEXT.
    Telling,
    /// Done: the next station's opening, or the tour's over.
    Leaving,
}

#[derive(Resource)]
pub struct Tour {
    /// The list's path, as given.
    pub tour: String,
    pub stations: Vec<Station>,
    pub index: usize,
    pub phase: Phase,
    /// The board, once it's up (seconds into the session then).
    board: Option<(Entity, f32)>,
    /// Where the player stood last frame, to follow a staging's move.
    last_at: Option<Vec3>,
    synced: bool,
    /// The flash in front of the eye, and frames it has left.
    flash: Option<(Entity, u32)>,
    nudged: bool,
    log: std::path::PathBuf,
}

impl Tour {
    pub fn station(&self) -> &Station {
        &self.stations[self.index]
    }

    /// One line in `tour.jsonl`, and the session log.
    fn note(&self, event: &str, extra: serde_json::Value) {
        let wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let mut line = serde_json::json!({
            "wall_ms": wall,
            "event": event,
            "tour": self.tour,
            "station": self.index,
            "of": self.stations.len(),
            "name": self.station().name,
        });
        if let (Some(l), Some(e)) = (line.as_object_mut(), extra.as_object()) {
            l.extend(e.clone());
        }
        info!("playtest: {line}");
        if let Some(dir) = self.log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// A card on the tour's board.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TourCard {
    /// What station this is, or what's asked (not pressed).
    Title,
    Next,
    Rate(u8),
}

fn card(c: TourCard, size: Vec2, style: ScreenStyle) -> impl Bundle {
    (
        Name::new("Tour card"),
        ScreenText {
            text: String::new(),
            size,
            style,
        },
        TextSource::Card,
        c,
        Pressable::Camp,
        Grabbable {
            points: vec![GrabPoint::free()],
        },
        Collider::cuboid(size.x, size.y, 0.03),
        CollisionLayers::new(Layer::Item, LayerMask::NONE),
        Visibility::default(),
    )
}

/// Where the board stands from the player: ahead and to the left, at
/// chest height, turned to face them.
const BOARD_FROM_PLAYER: Vec3 = Vec3::new(-0.45, 1.25, -0.8);

/// The board, once the player's there: the station's title, NEXT and
/// three comfort cards (turning, vignette, sound).
/// The board's pose for a player standing at `root`.
fn board_pose(root: Transform) -> Transform {
    let flat = Quat::from_rotation_y(root.rotation.to_euler(EulerRot::YXZ).0);
    let at = root.translation + flat * BOARD_FROM_PLAYER;
    let face = Transform::from_translation(at).looking_at(
        Vec3::new(root.translation.x, at.y, root.translation.z),
        Vec3::Y,
    );
    // Cards face +Z: turn round to face the player.
    face.with_rotation(face.rotation * Quat::from_rotation_y(std::f32::consts::PI))
}

/// A jump this long in one frame is a staging putting the player
/// somewhere (the truck, the camp table, the arcade's bench), metres.
const PLACED: f32 = 2.0;
/// Seconds into a station a staging may still move the player.
const PLACING: f32 = 30.0;

fn put_up_board(
    mut commands: Commands,
    mut tour: ResMut<Tour>,
    players: Query<&GlobalTransform, With<LocalPlayer>>,
    time: Res<Time>,
) {
    let Some(player) = players.iter().next() else {
        return;
    };
    let root = player.compute_transform();
    let last = tour.last_at.replace(root.translation);
    if tour.board.is_none() && tour.phase == Phase::Leaving {
        return;
    }
    if let Some((board, since)) = tour.board {
        // Moved by the station's own staging after the board went up: the
        // board goes with them.
        if last.is_some_and(|l| l.distance(root.translation) > PLACED)
            && time.elapsed_secs() - since < PLACING
        {
            commands.entity(board).insert(board_pose(root));
        }
        return;
    }
    let pose = board_pose(root);
    let board = commands
        .spawn((
            card(TourCard::Title, Vec2::new(0.5, 0.3), ScreenStyle::Paper),
            pose,
        ))
        .remove::<(Pressable, Grabbable)>()
        .id();
    commands.spawn((
        card(TourCard::Next, Vec2::new(0.22, 0.09), ScreenStyle::Card),
        Transform::from_xyz(0.14, -0.21, 0.01),
        ChildOf(board),
    ));
    for (i, c) in [
        ComfortCard::Turning,
        ComfortCard::Vignette,
        ComfortCard::Sound,
    ]
    .into_iter()
    .enumerate()
    {
        commands.spawn((
            Name::new("Comfort card"),
            c,
            Pressable::Setting,
            Grabbable {
                points: vec![GrabPoint::free()],
            },
            Collider::cuboid(0.24, 0.06, 0.03),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            TextSource::Card,
            ScreenText {
                text: String::new(),
                size: Vec2::new(0.24, 0.06),
                style: ScreenStyle::Card,
            },
            Transform::from_xyz(-0.38, 0.09 - i as f32 * 0.075, 0.01),
            Visibility::default(),
            ChildOf(board),
        ));
    }
    tour.board = Some((board, time.elapsed_secs()));
    let s = tour.station().clone();
    tour.note(
        "start",
        serde_json::json!({ "minutes": s.minutes, "args": s.args.join(" ") }),
    );
}

/// Once everything's in (or at once, with no window), the chirp and the
/// flash, noted with the wall clock: the recording's sync mark.
fn sync(
    mut commands: Commands,
    mut tour: ResMut<Tour>,
    loaded: Option<Res<crate::render::timeline::Loaded>>,
    opts: Res<LaunchOptions>,
    heads: Query<Entity, With<crate::rig::RigHead>>,
    bank: Option<Res<crate::audio::SoundBank>>,
    rng: Option<ResMut<crate::audio::AudioRng>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    if let Some((e, left)) = tour.flash {
        if left == 0 {
            commands.entity(e).despawn();
            tour.flash = None;
        } else {
            tour.flash = Some((e, left - 1));
        }
    }
    if tour.synced || tour.board.is_none() || (loaded.is_none() && !opts.headless) {
        return;
    }
    tour.synced = true;
    if let (Some(bank), Some(mut rng)) = (bank, rng) {
        crate::audio::play(
            &mut commands,
            &bank,
            &mut rng.0,
            "ui/tour_sync",
            None,
            "tour.sync",
            0.0,
            0.0,
        );
    }
    // No window, nothing to flash (a headless run): the note's enough.
    if let (Some(head), Some(mut meshes), Some(mut materials)) =
        (heads.iter().next(), meshes, materials)
    {
        let flash = commands
            .spawn((
                Mesh3d(meshes.add(Rectangle::new(2.0, 2.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    unlit: true,
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.0, -0.12),
                ChildOf(head),
            ))
            .id();
        tour.flash = Some((flash, 2));
    }
    tour.note("sync", serde_json::json!({}));
}

#[allow(clippy::too_many_arguments)]
fn press_cards(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    cards: Query<&TourCard>,
    mut tour: ResMut<Tour>,
    opts: Res<LaunchOptions>,
    mut exit: MessageWriter<AppExit>,
    mut leaving: Option<ResMut<crate::rig::loading::Leaving>>,
    time: Res<Time>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok(c) = cards.get(*item) else {
            continue;
        };
        let Some((board, since)) = tour.board else {
            continue;
        };
        match (tour.phase, *c) {
            (Phase::Playing, TourCard::Next) => {
                tour.phase = Phase::Rating;
                for k in 1..=5u8 {
                    commands.spawn((
                        card(TourCard::Rate(k), Vec2::new(0.08, 0.08), ScreenStyle::Card),
                        Transform::from_xyz(-0.2 + (k - 1) as f32 * 0.1, -0.21, 0.01),
                        ChildOf(board),
                    ));
                }
            }
            (Phase::Rating, TourCard::Rate(k)) => {
                tour.phase = Phase::Telling;
                let secs = time.elapsed_secs() - since;
                tour.note("rating", serde_json::json!({ "rating": k, "secs": secs }));
            }
            (Phase::Telling, TourCard::Next) => {
                tour.phase = Phase::Leaving;
                let secs = time.elapsed_secs() - since;
                tour.note("end", serde_json::json!({ "secs": secs }));
                move_on(&tour, &opts, &mut exit, leaving.as_deref_mut());
            }
            _ => {}
        }
    }
}

/// On to the next station (a relaunch with its number), or the tour's
/// over. A test, with a profile of its own and no window, asks and stays.
fn move_on(
    tour: &Tour,
    opts: &LaunchOptions,
    exit: &mut MessageWriter<AppExit>,
    leaving: Option<&mut crate::rig::loading::Leaving>,
) {
    let next = tour.index + 1;
    let to = if next < tour.stations.len() {
        crate::request_relaunch(next_args(std::env::args().skip(1), next));
        tour.stations[next].name.clone()
    } else {
        tour.note("done", serde_json::json!({}));
        "the end of the tour".to_owned()
    };
    if !crate::persist::owns_profile(opts) || opts.headless {
        return;
    }
    match leaving {
        Some(l) => l.go(&to),
        None => {
            exit.write(AppExit::Success);
        }
    }
}

/// A station staged for a fix this build doesn't have yet (a flag it
/// doesn't know, or an item `--give` names that isn't in the catalog) is
/// skipped, noted with why, and the tour moves on: half a station wastes
/// a playtester's minutes.
fn skip_unstageable(
    mut tour: ResMut<Tour>,
    opts: Res<LaunchOptions>,
    catalog: Option<Res<crate::items::Catalog>>,
    mut exit: MessageWriter<AppExit>,
    mut leaving: Option<ResMut<crate::rig::loading::Leaving>>,
    mut checked: Local<bool>,
) {
    if *checked {
        return;
    }
    let Some(catalog) = catalog else {
        return;
    };
    *checked = true;
    let mut why = Vec::new();
    if !opts.unknown_args.is_empty() {
        why.push(format!(
            "this build doesn't know {}",
            opts.unknown_args.join(" ")
        ));
    }
    if let Some(ids) = flag(&tour.station().args, "--give") {
        for id in ids.split(',').filter(|i| !i.is_empty()) {
            if catalog.kind(id).is_none() {
                why.push(format!("no item {id} in this build"));
            }
        }
    }
    if why.is_empty() {
        return;
    }
    tour.phase = Phase::Leaving;
    tour.note("skipped", serde_json::json!({ "why": why.join("; ") }));
    move_on(&tour, &opts, &mut exit, leaving.as_deref_mut());
}

/// What the board says, and the rating cards' numbers; once an answer's
/// in, the rating cards go.
fn board_text(
    mut commands: Commands,
    tour: Res<Tour>,
    mut cards: Query<(Entity, &TourCard, &mut ScreenText, &mut Visibility)>,
    time: Res<Time>,
) {
    let Some((_, since)) = tour.board else {
        return;
    };
    let s = tour.station();
    let over = time.elapsed_secs() - since > s.minutes * 60.0;
    for (e, c, mut text, mut vis) in &mut cards {
        let want = match (c, tour.phase) {
            (TourCard::Title, Phase::Playing) => format!(
                "PLAYTEST {}/{}: {}\n\nAbout {} min{}.\n{}\nMark anything odd: both sticks (F9).",
                tour.index + 1,
                tour.stations.len(),
                s.name.replace('_', " ").to_uppercase(),
                s.minutes,
                if over {
                    " (time's up when you're ready)"
                } else {
                    ""
                },
                match &s.hint {
                    Some(h) => format!("Try: {h}."),
                    None => "Look it over; say what you notice.".to_owned(),
                },
            ),
            (TourCard::Title, Phase::Rating) => {
                format!("{}\n\n1 = poor, 5 = great.", s.question)
            }
            (TourCard::Title, Phase::Telling) => {
                "Thanks. Now say out loud what\nbugged you, if anything.\n\nThen NEXT.".to_owned()
            }
            (TourCard::Title, Phase::Leaving) => {
                if tour.index + 1 < tour.stations.len() {
                    format!(
                        "On to {}...",
                        tour.stations[tour.index + 1].name.replace('_', " ")
                    )
                } else {
                    "That's the tour. Thank you!\nPress REPORT at camp, and send\nthe zip with the recording.".to_owned()
                }
            }
            (TourCard::Next, Phase::Playing) | (TourCard::Next, Phase::Telling) => {
                "NEXT".to_owned()
            }
            (TourCard::Next, _) => {
                *vis = Visibility::Hidden;
                continue;
            }
            (TourCard::Rate(_), Phase::Telling | Phase::Leaving) => {
                commands.entity(e).despawn();
                continue;
            }
            (TourCard::Rate(k), _) => k.to_string(),
        };
        if matches!(c, TourCard::Next) {
            *vis = Visibility::Inherited;
        }
        if text.text != want {
            text.text = want;
        }
    }
}

/// When a station's time is up, once, a line in the log.
fn nudge(mut tour: ResMut<Tour>, time: Res<Time>) {
    let Some((_, since)) = tour.board else {
        return;
    };
    if !tour.nudged
        && tour.phase == Phase::Playing
        && time.elapsed_secs() - since > tour.station().minutes * 60.0
    {
        tour.nudged = true;
        tour.note("time_up", serde_json::json!({}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(text: &str) -> std::path::PathBuf {
        let p =
            std::env::temp_dir().join(format!("tour_{}_{}.txt", std::process::id(), text.len()));
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn a_tour_list_reads_and_says_whats_wrong() {
        let p = list("# a tour\nyard | 3 | How was it? | --map training_yard --spawn 0,0,40,0\n");
        let s = load(&p).unwrap();
        assert_eq!(s[0].name, "yard");
        assert_eq!(s[0].args, ["--map", "training_yard", "--spawn", "0,0,40,0"]);
        let bad = list("yard | soon | ? | --map training_yard\n");
        assert!(load(&bad).unwrap_err().contains(":1: minutes"));
        let nested = list("yard | 3 | ? | --playtest other.txt\n");
        assert!(load(&nested).is_err(), "a station can't start a tour");
    }

    /// A station's arguments come after the command line's, so they win;
    /// the next station's command line keeps everything but the number.
    #[test]
    fn stations_splice_and_step_on() {
        let p = list("a | 1 | ? | --map training_yard --hour dusk\nb | 1 | ? | --map arcade\n");
        let path = p.display().to_string();
        let args: Vec<String> = ["--mirror", "--playtest", &path, "--playtest-station", "1"]
            .map(String::from)
            .into();
        let o = LaunchOptions::parse(args.clone());
        assert_eq!(o.map, "arcade");
        assert_eq!(o.playtest_station, 1);
        assert!(o.mirror);
        assert_eq!(
            next_args(args, 2),
            ["--mirror", "--playtest", &path, "--playtest-station", "2"]
        );
    }

    /// The tour the build ships (the camp board's PLAYTEST TOUR) reads,
    /// and each station's arguments open a valley there is.
    #[test]
    fn the_shipped_tour_reads() {
        let path = crate::asset_root_path().join(crate::camp::TOUR);
        let stations = load(&path).unwrap_or_else(|e| panic!("{e}"));
        for s in &stations {
            let o = LaunchOptions::parse(s.args.clone());
            assert!(
                crate::asset_root_path()
                    .join("maps")
                    .join(format!("{}.ron", o.map))
                    .exists(),
                "{}: no map {}",
                s.name,
                o.map
            );
        }
    }

    /// A bot plays a three-station tour as a player would: each station's
    /// board goes up, NEXT, a 4, NEXT; each but the last asks for the
    /// next station's session, and the notes say it all in order.
    #[test]
    fn a_bot_walks_a_three_station_tour() {
        let _one = crate::RESTART_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _ = crate::take_relaunch();
        let data = std::env::temp_dir().join(format!("tour_walk_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        let stations = "one | 1 | Q1? | --map training_yard --no-robots\n\
                        two | 1 | Q2? | --map training_yard --no-robots --hour dusk\n\
                        three | 1 | Q3? | --map training_yard --no-robots\n";
        let path = list(stations).display().to_string();
        for n in 0..3usize {
            let opts = LaunchOptions::parse(
                [
                    "--playtest",
                    &path,
                    "--playtest-station",
                    &n.to_string(),
                    "--data-dir",
                    &data.display().to_string(),
                ]
                .map(String::from),
            );
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            let find = |app: &mut App, want: TourCard| {
                for _ in 0..200 {
                    app.update();
                    let w = app.world_mut();
                    let mut q = w.query::<(Entity, &TourCard)>();
                    if let Some((e, _)) = q.iter(w).find(|(_, c)| **c == want) {
                        return e;
                    }
                }
                panic!("station {n}: no {want:?} card");
            };
            let press = |app: &mut App, item: Entity| {
                let w = app.world_mut();
                let player = w
                    .query_filtered::<Entity, With<LocalPlayer>>()
                    .iter(w)
                    .next()
                    .unwrap_or(item);
                w.write_message(InteractEvent::Pressed { item, player });
                for _ in 0..3 {
                    app.update();
                }
            };
            let next = find(&mut app, TourCard::Next);
            press(&mut app, next);
            let four = find(&mut app, TourCard::Rate(4));
            press(&mut app, four);
            assert_eq!(app.world().resource::<Tour>().phase, Phase::Telling);
            press(&mut app, next);
            assert_eq!(app.world().resource::<Tour>().phase, Phase::Leaving);
            let relaunch = crate::take_relaunch();
            if n < 2 {
                let args = relaunch.unwrap_or_else(|| panic!("station {n} asked for no next"));
                assert_eq!(
                    args[args.len() - 2..],
                    ["--playtest-station".to_owned(), (n + 1).to_string()]
                );
            } else {
                assert!(relaunch.is_none(), "the last station opens nothing");
            }
        }
        let notes = std::fs::read_to_string(data.join("playtest").join("tour.jsonl")).unwrap();
        let events: Vec<(String, u64, Option<u64>)> = notes
            .lines()
            .map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).unwrap();
                (
                    v["event"].as_str().unwrap().to_owned(),
                    v["station"].as_u64().unwrap(),
                    v["rating"].as_u64(),
                )
            })
            .collect();
        let _ = std::fs::remove_dir_all(&data);
        let mut want = Vec::new();
        for n in 0..3u64 {
            want.push(("start".to_owned(), n, None));
            want.push(("sync".to_owned(), n, None));
            want.push(("rating".to_owned(), n, Some(4)));
            want.push(("end".to_owned(), n, None));
        }
        want.push(("done".to_owned(), 2, None));
        assert_eq!(events, want);
    }

    /// The ingest finds a tour's chirps in a made-up recording, lines it up
    /// with the notes, and pulls out stations, ratings and frames
    /// (`scripts/playtest_ingest.py --self-test`).
    #[test]
    fn a_recording_lines_up_with_the_tour() {
        let script = crate::asset_root_path().join("../scripts/playtest_ingest.py");
        let Ok(out) = std::process::Command::new("python3")
            .arg(&script)
            .arg("--self-test")
            .output()
        else {
            eprintln!("no python3: skipped");
            return;
        };
        assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// `--spawn` puts the player there and keeps them there: the yard's
    /// truck start once put a station's player back in the truck.
    #[test]
    fn a_station_stands_the_player_where_it_says() {
        let opts = LaunchOptions::parse(
            "--map training_yard --no-robots --spawn 0,0,40,0"
                .split(' ')
                .map(String::from),
        );
        assert!(opts.places_player());
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..60 {
            app.update();
        }
        let w = app.world_mut();
        let at = w
            .query_filtered::<&GlobalTransform, With<LocalPlayer>>()
            .iter(w)
            .next()
            .expect("a player")
            .translation();
        assert!(
            Vec2::new(at.x, at.z).distance(Vec2::new(0.0, 40.0)) < 1.5,
            "the player stands at {at}, not (0, 40)"
        );
    }

    /// A staging that moves the player after the board's up (the arcade
    /// stands you at its bench) takes the board along.
    #[test]
    fn the_board_follows_a_staging_that_moves_you() {
        let p = list("boxes | 1 | Q? | --arcade\n");
        let data = std::env::temp_dir().join(format!("tour_arcade_{}", std::process::id()));
        let opts = LaunchOptions::parse(
            [
                "--playtest",
                &p.display().to_string(),
                "--data-dir",
                &data.display().to_string(),
            ]
            .map(String::from),
        );
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..240 {
            app.update();
        }
        let w = app.world_mut();
        let player = w
            .query_filtered::<&GlobalTransform, With<LocalPlayer>>()
            .iter(w)
            .next()
            .expect("a player")
            .translation();
        let board = w
            .query::<(&TourCard, &GlobalTransform)>()
            .iter(w)
            .find(|(c, _)| **c == TourCard::Title)
            .expect("the board")
            .1
            .translation();
        let _ = std::fs::remove_dir_all(&data);
        let off = Vec2::new(board.x - player.x, board.z - player.z).length();
        assert!(off < 1.5, "the board stands {off:.1} m from the player");
    }

    /// A station this build can't stage (a flag it doesn't know, an item
    /// it hasn't got) is skipped with why, and the tour moves on; the
    /// optional fifth field is what to try.
    #[test]
    fn a_station_this_build_cant_stage_is_skipped() {
        let _one = crate::RESTART_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let _ = crate::take_relaunch();
        let p = list(
            "future | 1 | Q? | --map training_yard --no-robots --grapple-from-orbit | swing on it\n\
             next | 1 | Q? | --map training_yard --no-robots\n",
        );
        let stations = load(&p).unwrap();
        assert_eq!(stations[0].hint.as_deref(), Some("swing on it"));
        assert_eq!(stations[1].hint, None);
        let data = std::env::temp_dir().join(format!("tour_skip_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        let opts = LaunchOptions::parse(
            [
                "--playtest",
                &p.display().to_string(),
                "--data-dir",
                &data.display().to_string(),
            ]
            .map(String::from),
        );
        assert_eq!(opts.unknown_args, ["--grapple-from-orbit"]);
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(app.world().resource::<Tour>().phase, Phase::Leaving);
        let args = crate::take_relaunch().expect("on to the next station");
        assert_eq!(args[args.len() - 1], "1");
        let notes = std::fs::read_to_string(data.join("playtest").join("tour.jsonl")).unwrap();
        let _ = std::fs::remove_dir_all(&data);
        assert!(
            notes.contains("\"skipped\"") && notes.contains("--grapple-from-orbit"),
            "{notes}"
        );
        assert!(!notes.contains("\"start\""), "no board for a skipped station");
    }

    /// The tour is a sandbox: no station has a career to save to.
    #[test]
    fn a_tour_never_touches_the_career() {
        let p = list("a | 1 | ? | --map marrow_creek\n");
        let o = LaunchOptions::parse(["--playtest", &p.display().to_string()].map(String::from));
        assert_eq!(o.map, "marrow_creek");
        assert!(crate::persist::career_path(&o).is_none());
    }
}
