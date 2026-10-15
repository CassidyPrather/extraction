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
//! starts. Its signs walk you through the kit; a card by the truck skips
//! it, and pulling the truck's cord finishes it. Either way the driver
//! takes you on to the Zone.

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
}

/// Whether this session opens at the camp: a real game (or `--camp`), not
/// the training yard.
pub fn camp_wanted(opts: &LaunchOptions) -> bool {
    !opts.headless
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
    if opts.map == TRAINING_YARD {
        // The way out of the yard: a card on a post by the tailgate.
        let truck = base_pose(&zone);
        let at = truck.transform_point(Vec3::new(-1.8, 0.0, 3.2));
        let at = Vec3::new(at.x, zone.ground(at.x, at.z), at.z);
        let post = commands
            .spawn((
                Name::new("Skip card post"),
                Transform::from_translation(at).with_rotation(truck.rotation),
                Visibility::default(),
                CampProps::Post,
            ))
            .id();
        commands.spawn((
            card(CampCard::SkipYard, Vec2::new(0.34, 0.2)),
            Transform::from_xyz(0.0, 1.35, 0.05),
            ChildOf(post),
        ));
        return;
    }
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
        Transform::from_xyz(0.0, 0.335, z),
        ChildOf(board),
    ));
    let mut career = card(CampCard::Career, Vec2::new(0.52, 0.56));
    career.0.style = ScreenStyle::Paper;
    commands.spawn((career, Transform::from_xyz(-0.28, -0.08, z), ChildOf(board)));
    let column = [
        (Some(CampCard::Training), None),
        (Some(CampCard::NewCareer), None),
        (Some(CampCard::PackUp), None),
        (None, Some(ComfortCard::Turning)),
        (None, Some(ComfortCard::Vignette)),
    ];
    for (i, (camp_card, comfort)) in column.into_iter().enumerate() {
        let at = Transform::from_xyz(0.38, 0.19 - i as f32 * 0.128, z);
        if let Some(c) = camp_card {
            commands.spawn((card(c, Vec2::new(0.3, 0.11)), at, ChildOf(board)));
        } else if let Some(c) = comfort {
            commands.spawn((
                Name::new("Comfort card"),
                c,
                Pressable::Setting,
                Grabbable {
                    points: vec![GrabPoint::free()],
                },
                Collider::cuboid(0.3, 0.11, 0.03),
                CollisionLayers::new(Layer::Item, LayerMask::NONE),
                ScreenText {
                    text: String::new(),
                    size: Vec2::new(0.3, 0.11),
                    style: ScreenStyle::Card,
                },
                at,
                Visibility::default(),
                ChildOf(board),
            ));
        }
    }
    // You start at the table, facing the board (the truck beyond it).
    if opts.scenario.is_none() && opts.camera.is_none() {
        state.stand = Some(pose * Transform::from_xyz(0.0, 0.0, 1.15));
        state.hold = 20;
    }
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
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        // In the yard, the truck's cord means "I'm done here".
        let yard_bell =
            opts.map == TRAINING_YARD && matches!(pressables.get(*item), Ok(Pressable::BellPull));
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
                    if !opts.headless {
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
            CampCard::SkipYard => {
                if !opts.headless {
                    crate::persist::mark_trained();
                }
                raid.say("\"Right then.\" The engine turns over.");
                let to = crate::persist::career_destination(&LaunchOptions::default())
                    .unwrap_or_else(|| "marrow_creek".into());
                leave(&opts, &to, false, &mut exit);
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
    if opts.headless {
        return;
    }
    if fresh {
        crate::request_fresh_restart(map);
    } else {
        crate::request_restart(map);
    }
    exit.write(AppExit::Success);
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
                t +=
                    "Your kit is in the crate in the truck.\n\nWalk out through the gate to start.";
                t
            }
            CampCard::Training => "TRAINING YARD".into(),
            CampCard::NewCareer if confirming => "SURE? AGAIN".into(),
            CampCard::NewCareer => "NEW CAREER".into(),
            CampCard::PackUp => "PACK UP".into(),
            CampCard::SkipYard => "ON TO THE ZONE\n(skip the yard)".into(),
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
