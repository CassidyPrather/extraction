//! The camp: the title screen, as a place. When the game opens you stand
//! at a folding table behind the parked truck, under a lantern, the Zone's
//! gate behind you. A board on the table carries the game's name, your
//! career so far, and cards to press:
//! - TRAINING YARD: practice, with the kit and nothing hunting you;
//! - NEW CAREER: press twice; the old logbook is put aside;
//! - TURNING, VIGNETTE: comfort settings, as on the truck's board;
//! - PACK UP: save and quit.
//!
//! To play, walk to the gate. Nothing else is a menu.
//!
//! The training yard (`assets/maps/training_yard.ron`) is where someone new
//! starts. Its stations (`onboarding`) have you do each thing once; a card
//! by the tailgate skips it, and pulling the truck's cord finishes it.
//! Either way the driver takes you on to the Zone.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::launch::LaunchOptions;
use crate::raid::Raid;
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::raid::comfort_board::ComfortCard;
use crate::raid::truck::{Pressable, Truck, base_pose};
use crate::rig::LocalPlayer;
use crate::world::{Layer, Zone};

/// The yard's cord, pulled before the end.
pub const YARD_CORD_LINE: &str = "Driver: \"Not done yet. Pull again to go.\"";

/// The practice map's name.
pub const TRAINING_YARD: &str = "training_yard";

pub struct CampPlugin;

impl Plugin for CampPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CampState>().add_systems(
            Update,
            (spawn_camp, stand_at_camp, press_camp, camp_text).chain(),
        );
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampCard {
    Title,
    Career,
    Training,
    NewCareer,
    PackUp,
    /// In the training yard: on to the Zone.
    SkipYard,
    /// Writes the playtest report (`report`) to send back.
    Report,
}

#[derive(Resource, Default)]
struct CampState {
    /// Where you stand at the table, once it's set up.
    stand: Option<Transform>,
    /// Frames left to keep putting the player there (the truck's own
    /// start runs too).
    hold: u32,
    /// NEW CAREER was pressed once; a second press before this (seconds
    /// of game time) burns the logbook.
    confirm_until: f32,
    /// In the yard: the cord was pulled once before the end; a second
    /// pull before this goes.
    cord_until: f32,
    /// What REPORT last wrote (the zip's name), or why it couldn't.
    report: Option<String>,
}

/// Whether this session opens at the camp: a real game (or `--camp`), not
/// the training yard.
pub fn camp_wanted(opts: &LaunchOptions) -> bool {
    crate::persist::owns_profile(opts)
        && opts.map != TRAINING_YARD
        && (opts.camp || crate::persist::career_path(opts).is_some())
}

/// The camp's frame: behind the truck on level ground off the road (and
/// clear of the camp kitchen beside it), facing the tailgate.
fn camp_pose(zone: &Zone) -> Transform {
    let truck = base_pose(zone);
    let tailgate = truck.transform_point(Vec3::new(0.0, 0.0, 2.2));
    let kitchen = zone
        .marker(&extraction_core::map::MarkerKind::TruckStop)
        .map(|m| Vec2::new(m.at[0] - 4.5, m.at[1] - 1.0));
    let mut best = (f32::INFINITY, tailgate);
    for i in -6..=6 {
        for j in 3..=10 {
            let p = truck.transform_point(Vec3::new(i as f32, 0.0, j as f32));
            let q = [p.x, p.z];
            // Level, off the road, a few metres back, not in the kitchen.
            let mut score = zone.hm.slope_deg(q) * 0.5
                + (Vec2::new(p.x, p.z).distance(tailgate.xz()) - 5.5).abs();
            for (dx, dz) in [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                if zone.hm.road_at([p.x + dx, p.z + dz]).weight > 0 {
                    score += 50.0;
                }
            }
            if kitchen.is_some_and(|k| k.distance(p.xz()) < 3.5) {
                score += 50.0;
            }
            if score < best.0 {
                best = (score, p);
            }
        }
    }
    let at = Vec3::new(best.1.x, zone.ground(best.1.x, best.1.z), best.1.z);
    // Local -Z toward the tailgate.
    let to = Vec3::new(tailgate.x - at.x, 0.0, tailgate.z - at.z);
    Transform::from_translation(at).looking_to(to.normalize_or(Vec3::NEG_Z), Vec3::Y)
}

const TABLE_TOP: f32 = 0.76;

fn spawn_camp(
    mut commands: Commands,
    trucks: Query<Entity, Added<Truck>>,
    zone: Res<Zone>,
    opts: Res<LaunchOptions>,
    mut state: ResMut<CampState>,
) {
    if trucks.is_empty() {
        return;
    }
    // (In the yard, the way out is on the training board by the tailgate:
    // `onboarding`.)
    if !camp_wanted(&opts) {
        return;
    }
    let pose = camp_pose(&zone);
    let camp = commands
        .spawn((
            Name::new("Camp"),
            pose,
            Visibility::default(),
            CampProps::Table,
        ))
        .id();
    // The table top is solid (a static collider of its own: a physics body
    // at the root would be posed by physics before its transform is).
    commands.spawn((
        Collider::cuboid(1.3, 0.04, 0.7),
        CollisionLayers::new(Layer::World, LayerMask::ALL),
        Transform::from_xyz(0.0, TABLE_TOP - 0.02, 0.0),
        ChildOf(camp),
    ));
    // The board stands at the table's back edge, leaning away.
    let board = commands
        .spawn((
            Name::new("Camp board"),
            Transform::from_xyz(0.0, TABLE_TOP + 0.44, -0.27)
                .with_rotation(Quat::from_rotation_x(-0.21)),
            Visibility::default(),
            CampProps::Board,
            ChildOf(camp),
        ))
        .id();
    let z = 0.016;
    commands.spawn((
        card(CampCard::Title, Vec2::new(1.1, 0.16)),
        crate::render::legibility::TextSource::CampTitle,
        Transform::from_xyz(0.0, 0.335, z),
        ChildOf(board),
    ));
    let mut career = card(CampCard::Career, Vec2::new(0.52, 0.56));
    career.0.style = ScreenStyle::Paper;
    commands.spawn((
        career,
        crate::render::legibility::TextSource::CampSheet,
        Transform::from_xyz(-0.28, -0.08, z),
        ChildOf(board),
    ));
    let column = [
        (Some(CampCard::Training), None),
        (Some(CampCard::NewCareer), None),
        (Some(CampCard::PackUp), None),
        (None, Some(ComfortCard::Turning)),
        (None, Some(ComfortCard::Vignette)),
        (None, Some(ComfortCard::Sound)),
    ];
    for (i, (camp_card, comfort)) in column.into_iter().enumerate() {
        let at = Transform::from_xyz(0.38, 0.195 - i as f32 * 0.112, z);
        if let Some(c) = camp_card {
            commands.spawn((
                card(c, Vec2::new(0.3, 0.1)),
                crate::render::legibility::TextSource::Card,
                at,
                ChildOf(board),
            ));
        } else if let Some(c) = comfort {
            commands.spawn((
                Name::new("Comfort card"),
                c,
                Pressable::Setting,
                Grabbable {
                    points: vec![GrabPoint::free()],
                },
                Collider::cuboid(0.3, 0.1, 0.03),
                CollisionLayers::new(Layer::Item, LayerMask::NONE),
                crate::render::legibility::TextSource::Card,
                ScreenText {
                    text: String::new(),
                    size: Vec2::new(0.3, 0.1),
                    style: ScreenStyle::Card,
                },
                at,
                Visibility::default(),
                ChildOf(board),
            ));
        }
    }
    // Under the career sheet: REPORT, for the playtest's report.
    commands.spawn((
        card(CampCard::Report, Vec2::new(0.4, 0.08)),
        crate::render::legibility::TextSource::Card,
        Transform::from_xyz(-0.28, -0.405, z),
        ChildOf(board),
    ));
    // You start at the table, facing the board (the truck beyond it).
    if opts.plays_for_real() && opts.camera.is_none() {
        state.stand = Some(pose * Transform::from_xyz(0.0, 0.0, 1.15));
        state.hold = 20;
    }
}

/// The yard's way out: ON TO THE ZONE (`onboarding` puts it up).
pub fn skip_card() -> (
    ScreenText,
    CampCard,
    Pressable,
    Grabbable,
    Collider,
    CollisionLayers,
    Visibility,
) {
    card(CampCard::SkipYard, Vec2::new(0.34, 0.2))
}

/// A pressable card with its words (filled in by `camp_text`).
fn card(
    c: CampCard,
    size: Vec2,
) -> (
    ScreenText,
    CampCard,
    Pressable,
    Grabbable,
    Collider,
    CollisionLayers,
    Visibility,
) {
    (
        ScreenText {
            text: String::new(),
            size,
            style: ScreenStyle::Card,
        },
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

/// The camp's furniture, dressed by the client (`render::props`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampProps {
    Table,
    Board,
    Post,
}

fn stand_at_camp(
    mut state: ResMut<CampState>,
    mut players: Query<
        (
            &mut Transform,
            Option<&mut Position>,
            Option<&mut Rotation>,
            Option<&mut crate::player::Body>,
        ),
        With<LocalPlayer>,
    >,
) {
    if state.hold == 0 {
        return;
    }
    state.hold -= 1;
    let Some(stand) = state.stand else {
        return;
    };
    // The body is a physics body: set its pose there too, or the next
    // step puts it back.
    for (mut t, pos, rot, body) in &mut players {
        if let Some(mut b) = body {
            b.velocity = Vec3::ZERO;
        }
        t.translation = stand.translation;
        t.rotation = stand.rotation;
        if let Some(mut p) = pos {
            p.0 = stand.translation;
        }
        if let Some(mut r) = rot {
            r.0 = stand.rotation;
        }
    }
}

fn press_camp(
    mut events: MessageReader<InteractEvent>,
    cards: Query<&CampCard>,
    pressables: Query<&Pressable>,
    mut state: ResMut<CampState>,
    opts: Res<LaunchOptions>,
    time: Res<Time>,
    mut raid: ResMut<Raid>,
    mut exit: MessageWriter<AppExit>,
    mut slot: Option<ResMut<crate::persist::SaveSlot>>,
    onboarding: Option<Res<crate::onboarding::Onboarding>>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        // In the yard, the truck's cord means "I'm done here". Before the
        // last station it asks first: a cord within reach of where you
        // start is easy to pull by mistake, and it's the end of the yard.
        let mut yard_bell =
            opts.map == TRAINING_YARD && matches!(pressables.get(*item), Ok(Pressable::BellPull));
        if yard_bell && !onboarding.as_ref().is_none_or(|o| o.ready_to_go()) {
            let now = time.elapsed_secs();
            if now > state.cord_until {
                state.cord_until = now + 6.0;
                raid.say(YARD_CORD_LINE);
                yard_bell = false;
            }
        }
        let Some(c) = cards
            .get(*item)
            .ok()
            .copied()
            .or(yard_bell.then_some(CampCard::SkipYard))
        else {
            continue;
        };
        match c {
            CampCard::Title | CampCard::Career => {}
            CampCard::Training => {
                raid.say("The driver nods at the yard. \"Take your time.\"");
                leave(&opts, TRAINING_YARD, false, &mut exit);
            }
            CampCard::NewCareer => {
                let now = time.elapsed_secs();
                if now > state.confirm_until {
                    state.confirm_until = now + 6.0;
                } else {
                    // Don't write the old career back on the way out.
                    if let Some(slot) = slot.as_mut() {
                        slot.0 = None;
                    }
                    if crate::persist::owns_profile(&opts) {
                        crate::persist::burn_career(&opts);
                    }
                    leave(&opts, TRAINING_YARD, true, &mut exit);
                }
            }
            CampCard::PackUp => {
                if !opts.headless {
                    exit.write(AppExit::Success);
                }
            }
            CampCard::Report => {
                let data = opts
                    .data_dir
                    .clone()
                    .unwrap_or_else(crate::persist::data_dir);
                state.report = Some(match crate::report::write_report(&data) {
                    Ok(path) => {
                        info!("report: written to {}", path.display());
                        raid.say("The driver nods. \"Send that back to them.\"");
                        path.file_name()
                            .map_or(String::new(), |n| n.to_string_lossy().to_string())
                    }
                    Err(e) => {
                        warn!("report: couldn't write it: {e}");
                        "couldn't write it".into()
                    }
                });
            }
            CampCard::SkipYard => {
                if crate::persist::owns_profile(&opts) {
                    crate::persist::mark_trained(&opts);
                }
                raid.say("\"Right then.\" The engine turns over.");
                leave(&opts, &crate::persist::next_valley(&opts), false, &mut exit);
            }
        }
    }
}

/// End this session and open the game again in `map`.
fn leave(opts: &LaunchOptions, map: &str, fresh: bool, exit: &mut MessageWriter<AppExit>) {
    info!(
        "camp: on to {map}{}",
        if fresh { " (new career)" } else { "" }
    );
    // A headless run with its own profile (a test) asks to restart but
    // doesn't quit: the test opens the next session itself.
    if !crate::persist::owns_profile(opts) {
        return;
    }
    if fresh {
        crate::request_fresh_restart(map);
    } else {
        crate::request_restart(map);
    }
    if !opts.headless {
        exit.write(AppExit::Success);
    }
}

fn camp_text(
    mut cards: Query<(&CampCard, &mut ScreenText)>,
    raid: Res<Raid>,
    zone: Res<Zone>,
    state: Res<CampState>,
    time: Res<Time>,
) {
    let confirming = time.elapsed_secs() < state.confirm_until;
    for (c, mut text) in &mut cards {
        let t = match c {
            CampCard::Title => "EXTRACTION".to_string(),
            CampCard::Career => {
                let mut t = format!("{}\n\n", zone.map.name.to_uppercase());
                if raid.raids == 0 {
                    t += "A new logbook.\n\n";
                } else {
                    t += &format!(
                        "Raids: {}\nHome: {}   Lost: {}\nSurges seen: {}\n\n",
                        raid.raids, raid.extractions, raid.deaths, raid.cycle
                    );
                }
                if raid.recalled {
                    t += "The machines went home.\nThe valleys are quiet.\n\n";
                }
                t +=
                    "Your kit is in the crate in the truck.\n\nWalk out through the gate to start.";
                t
            }
            CampCard::Training => "TRAINING YARD".into(),
            CampCard::NewCareer if confirming => "SURE? AGAIN".into(),
            CampCard::NewCareer => "NEW CAREER".into(),
            CampCard::PackUp => "PACK UP".into(),
            CampCard::SkipYard => "ON TO THE ZONE\n(skip the yard)".into(),
            CampCard::Report => match &state.report {
                Some(done) => format!("REPORT: {done}\n(in the reports folder)"),
                None => "REPORT (for the playtest)".into(),
            },
        };
        if text.text != t {
            text.text = t;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camp_is_for_real_games_only() {
        let mut o = LaunchOptions::default();
        o.headless = true;
        assert!(!camp_wanted(&o));
        let mut o = LaunchOptions::default();
        o.camp = true;
        assert!(camp_wanted(&o));
        o.map = TRAINING_YARD.into();
        assert!(!camp_wanted(&o));
    }

    fn open(opts: LaunchOptions) -> App {
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..8 {
            app.update();
        }
        app
    }

    fn card(app: &mut App, which: CampCard) -> Option<Entity> {
        app.world_mut()
            .query::<(Entity, &CampCard)>()
            .iter(app.world())
            .find(|(_, c)| **c == which)
            .map(|(e, _)| e)
    }

    fn press(app: &mut App, item: Entity) {
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .write_message(InteractEvent::Pressed { item, player });
        app.update();
    }

    /// Someone new, start to finish through the menus: the yard first, on
    /// to a valley with the camp and a career, back to the yard from the
    /// camp, and a new career. Each "restart" opens the session the game
    /// would relaunch into.
    #[test]
    fn a_new_player_goes_from_the_yard_to_the_camp_and_round_again() {
        let _restarts = crate::RESTART_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _ = crate::take_restart();
        let dir = std::env::temp_dir()
            .join(format!("extraction-test-{}", std::process::id()))
            .join("journey");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let base = LaunchOptions {
            data_dir: Some(dir.clone()),
            robots: false,
            ..Default::default()
        };
        let relaunch = |map: &str| LaunchOptions {
            map: map.into(),
            map_explicit: true,
            ..base.clone()
        };

        // First launch: the yard, with nothing hunting and a way out.
        let mut o = base.clone();
        crate::persist::choose_start(&mut o);
        assert_eq!(o.map, TRAINING_YARD, "someone new starts in the yard");
        assert!(!camp_wanted(&o));
        let mut yard = open(o);
        let skip = card(&mut yard, CampCard::SkipYard).expect("a skip card by the truck");
        assert!(
            card(&mut yard, CampCard::Title).is_none(),
            "no camp in the yard"
        );
        press(&mut yard, skip);
        let to = crate::take_restart().expect("skipping the yard drives on");
        assert_eq!((to.map.as_str(), to.fresh), ("marrow_creek", false));
        assert!(crate::persist::trained(&base), "the yard is marked done");
        drop(yard);

        // The valley: the camp, and a career written to the profile.
        let o = relaunch(&to.map);
        assert!(camp_wanted(&o));
        let mut valley = open(o);
        for c in [
            CampCard::Title,
            CampCard::Career,
            CampCard::Training,
            CampCard::NewCareer,
            CampCard::PackUp,
            CampCard::Report,
        ] {
            assert!(card(&mut valley, c).is_some(), "no {c:?} card at camp");
        }
        // REPORT writes the playtest's zip in the profile.
        let report = card(&mut valley, CampCard::Report).unwrap();
        press(&mut valley, report);
        let zips = std::fs::read_dir(dir.join("reports"))
            .map(|d| d.count())
            .unwrap_or(0);
        assert_eq!(zips, 1, "REPORT writes one zip in <data dir>/reports");
        valley.world_mut().write_message(crate::persist::SaveNow);
        valley.update();
        assert!(
            dir.join("career.ron").exists(),
            "the career is saved in the profile"
        );

        // Next launch, no --map: straight back to that valley.
        let mut o = base.clone();
        crate::persist::choose_start(&mut o);
        assert_eq!(o.map, "marrow_creek");

        // Back to the yard from the camp; the career stays.
        let training = card(&mut valley, CampCard::Training).unwrap();
        press(&mut valley, training);
        let to = crate::take_restart().expect("the training card drives to the yard");
        assert_eq!((to.map.as_str(), to.fresh), (TRAINING_YARD, false));
        assert!(dir.join("career.ron").exists());

        // NEW CAREER asks twice; then the logbook is put aside.
        let new = card(&mut valley, CampCard::NewCareer).unwrap();
        press(&mut valley, new);
        assert!(crate::take_restart().is_none(), "one press only asks");
        press(&mut valley, new);
        let to = crate::take_restart().expect("the second press starts over");
        assert_eq!((to.map.as_str(), to.fresh), (TRAINING_YARD, true));
        drop(valley);
        assert!(!dir.join("career.ron").exists() && dir.join("career.ron.old").exists());
        assert!(!crate::persist::trained(&base));
        let mut o = base.clone();
        crate::persist::choose_start(&mut o);
        assert_eq!(
            o.map, TRAINING_YARD,
            "a new career starts in the yard again"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_yard_saves_nothing() {
        let mut o = LaunchOptions::default();
        o.map = TRAINING_YARD.into();
        assert!(crate::persist::career_path(&o).is_none());
    }
}

// Sizes the client dresses the camp to (`render::props`).
pub const TABLE: Vec3 = Vec3::new(1.3, TABLE_TOP, 0.7);
pub const BOARD: Vec2 = Vec2::new(1.2, 0.9);
pub const POST_HEIGHT: f32 = 1.5;
