//! The bench: three tables of kit and a fourth with the wave board, at
//! the ground's south edge facing the fight. The board's cards are the
//! arcade's settings and its one control (CALL WAVE), pressed in the
//! headset like any other card. RESTART (or a call after the end) sweeps
//! the ground and lays the tables out again where you stand.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::save::SavedItem;

use super::waves::{GATES, GateLamp, WaveCalled, describe, gate, wave};
use super::{Arcade, ArcadeSystems, Reset, RunPhase};
use crate::camp::{CampProps, TABLE};
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::raid::comfort_board::ComfortCard;
use crate::raid::truck::Pressable;
use crate::render::legibility::TextSource;
use crate::rig::LocalPlayer;
use crate::world::{Layer, Zone};

/// The board's cards.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeCard {
    /// Call the next wave.
    Call,
    /// Sweep the ground and lay the tables out again.
    Restart,
    /// Which wave a fresh run starts at.
    StartAt,
    /// The answer to "how did it feel?": 1 to 5, or 0 to skip the gun.
    Feel(u8),
}

/// The board's sheet: the run, the ladder, the last run's summary.
#[derive(Component)]
pub struct ArcadeSheet;

/// The gun board's sheet: the gun card, and the question after a run.
#[derive(Component)]
pub struct GunCardSheet;

/// The kit the bench lays out (`laid` once per run).
#[derive(Resource, Default)]
pub struct Laid(pub bool);

/// The feel cards' words, 1 to 5.
pub const FEEL_WORDS: [&str; 5] = ["poor", "lacking", "fine", "good", "great"];

/// The waves a fresh run may start at (START AT's turns).
pub const STARTS: [u32; 3] = [1, 4, 7];

/// The bench's frame (local -Z toward the fight) and the tables' places
/// on it: (x, z, yaw).
pub const TABLES: [(f32, f32, f32); 5] = [
    (-2.2, 0.0, 0.0),
    (-0.75, 0.0, 0.0),
    (0.75, 0.0, 0.0),
    // The board's table, forward and turned toward where you stand.
    (2.9, 0.9, 0.6),
    // The gun board's, as far the other side.
    (-4.35, 0.9, 0.6),
];

/// The wave board's table and the gun board's.
pub const BOARD_TABLE: usize = 3;
pub const GUN_TABLE: usize = 4;

/// Where you stand at the bench, its frame.
pub const STAND: Vec3 = Vec3::new(0.0, 0.0, 1.2);

/// The kit on each gun table: (catalog id, rounds, what's seated in it,
/// where on the table (x, z), lying how).
#[derive(Clone, Copy)]
enum Lie {
    /// A long gun on its side, the muzzle along the table.
    Long,
    /// On its side, facing away (a pistol, a magazine, a box).
    Side,
    /// As it falls.
    Flat,
}

type Kit = (
    &'static str,
    u32,
    Option<(&'static str, u32)>,
    [f32; 2],
    Lie,
);

pub const KIT_TABLES: usize = 3;

fn kit(table: usize) -> &'static [Kit] {
    match table {
        0 => &[
            ("ak74", 0, Some(("ak_mag", 30)), [0.0, -0.18], Lie::Long),
            ("ppsh", 0, Some(("ppsh_drum", 71)), [0.0, 0.08], Lie::Long),
            ("ak_mag", 30, None, [-0.35, 0.27], Lie::Side),
            ("ak_mag", 30, None, [-0.1, 0.27], Lie::Side),
        ],
        1 => &[
            ("mosin", 0, None, [0.0, -0.2], Lie::Long),
            ("sks", 0, None, [0.0, 0.04], Lie::Long),
            ("mosin_clip", 5, None, [-0.4, 0.26], Lie::Flat),
            ("mosin_clip", 5, None, [-0.25, 0.26], Lie::Flat),
            ("mosin_clip", 5, None, [-0.1, 0.26], Lie::Flat),
            ("sks_clip", 10, None, [0.15, 0.26], Lie::Flat),
            ("sks_clip", 10, None, [0.3, 0.26], Lie::Flat),
        ],
        _ => &[
            ("mp133", 0, None, [0.0, -0.18], Lie::Long),
            ("ammo_12ga", 20, None, [-0.35, 0.12], Lie::Flat),
            ("pm_pistol", 0, Some(("pm_mag", 8)), [0.2, 0.12], Lie::Side),
            ("pm_mag", 8, None, [0.38, 0.26], Lie::Side),
            ("pm_mag", 8, None, [0.48, 0.26], Lie::Side),
            ("field_dressing", 0, None, [-0.1, 0.27], Lie::Flat),
            ("field_dressing", 0, None, [0.05, 0.27], Lie::Flat),
        ],
    }
}

/// On the board's table, in front of the board: the throwables.
const BOARD_TABLE_KIT: [Kit; 2] = [
    ("rgd5", 0, None, [-0.3, 0.2], Lie::Flat),
    ("rgd5", 0, None, [-0.1, 0.2], Lie::Flat),
];

pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Laid>()
            .add_systems(
                Update,
                (set_up, stand_at_bench)
                    .chain()
                    .in_set(ArcadeSystems::Setup),
            )
            .add_systems(
                Update,
                (press_cards, sheet_text, gun_card_text, card_text)
                    .chain()
                    .in_set(ArcadeSystems::Play),
            )
            .add_systems(
                PostUpdate,
                (sweep, lay_tables)
                    .chain()
                    .after(crate::raid::drop_everything),
            );
    }
}

/// The bench's frame: on the bench marker, facing north into the fight.
pub fn bench_pose(zone: &Zone) -> Transform {
    let at = zone
        .marker(&MarkerKind::Insertion)
        .map(|m| m.at)
        .unwrap_or([0.0, 27.0]);
    let p = Vec3::new(at[0], zone.ground(at[0], at[1]), at[1]);
    Transform::from_translation(p).looking_to(Vec3::NEG_Z, Vec3::Y)
}

/// One table's frame on the ground: down to the lowest ground under its
/// feet, so no leg hangs in the air.
fn table_pose(zone: &Zone, bench: &Transform, i: usize) -> Transform {
    let (x, z, yaw) = TABLES[i];
    let mut t = *bench * Transform::from_xyz(x, 0.0, z).with_rotation(Quat::from_rotation_y(yaw));
    let feet = [
        Vec3::new(-0.6, 0.0, -0.31),
        Vec3::new(-0.6, 0.0, 0.31),
        Vec3::new(0.6, 0.0, -0.31),
        Vec3::new(0.6, 0.0, 0.31),
    ];
    t.translation.y = feet
        .iter()
        .map(|f| {
            let p = t.transform_point(*f);
            zone.ground(p.x, p.z)
        })
        .fold(f32::MAX, f32::min);
    t
}

#[allow(clippy::type_complexity)]
fn card(c: ArcadeCard, size: Vec2) -> impl Bundle {
    (
        Name::new("Arcade card"),
        ScreenText {
            text: String::new(),
            size,
            style: ScreenStyle::Card,
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

/// Puts up the bench, the board and the gates' lamps once the world is
/// in, and stands you at the bench.
fn set_up(
    mut commands: Commands,
    zone: Res<Zone>,
    mut arcade: ResMut<Arcade>,
    players: Query<(), With<LocalPlayer>>,
    opts: Res<crate::launch::LaunchOptions>,
) {
    if arcade.bench.is_some() || players.is_empty() {
        return;
    }
    let bench = bench_pose(&zone);
    arcade.bench = Some(bench);
    arcade.stand = Some(bench * Transform::from_translation(STAND));
    // A screenshot's camera places itself.
    arcade.hold = if opts.camera.is_some() { 0 } else { 30 };
    for i in 0..TABLES.len() {
        let pose = table_pose(&zone, &bench, i);
        let table = commands
            .spawn((
                Name::new("Arcade table"),
                pose,
                Visibility::default(),
                CampProps::Table,
            ))
            .id();
        commands.spawn((
            Collider::cuboid(TABLE.x, 0.04, TABLE.z),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
            RigidBody::Static,
            Transform::from_xyz(0.0, TABLE.y - 0.02, 0.0),
            ChildOf(table),
        ));
        if i == BOARD_TABLE {
            board(&mut commands, table);
        }
        if i == GUN_TABLE {
            gun_board(&mut commands, table);
        }
    }
    for (g, (name, _, lamp)) in GATES.iter().enumerate() {
        let p = Vec3::new(lamp[0], zone.ground(lamp[0], lamp[1]), lamp[1]);
        commands.spawn((
            Name::new(format!("Arcade lamp, {} gate", name.to_lowercase())),
            GateLamp {
                gate: g,
                lit: false,
            },
            Transform::from_translation(p),
            Visibility::default(),
        ));
    }
    info!("arcade: bench at {:.1}", bench.translation);
}

/// A board's sheet (the wave board's and the gun card), m.
pub const SHEET: Vec2 = Vec2::new(0.66, 0.82);

/// The board on the last table: the sheet on the left, the cards down
/// the right.
fn board(commands: &mut Commands, table: Entity) {
    let board = commands
        .spawn((
            Name::new("Arcade board"),
            crate::camp::board_pose(),
            Visibility::default(),
            CampProps::Board,
            ChildOf(table),
        ))
        .id();
    let z = 0.016;
    commands.spawn((
        Name::new("Arcade sheet"),
        ArcadeSheet,
        ScreenText {
            text: String::new(),
            size: SHEET,
            style: ScreenStyle::Paper,
        },
        // The camp's board on a camp table, read as the camp's sheet is:
        // standing at the table, within reach of its cards.
        TextSource::CampSheet,
        Transform::from_xyz(-0.24, 0.0, z),
        Visibility::default(),
        ChildOf(board),
    ));
    commands.spawn((
        card(ArcadeCard::Call, Vec2::new(0.4, 0.2)),
        Transform::from_xyz(0.35, 0.28, z),
        ChildOf(board),
    ));
    commands.spawn((
        card(ArcadeCard::Restart, Vec2::new(0.4, 0.1)),
        Transform::from_xyz(0.35, 0.1, z),
        ChildOf(board),
    ));
    commands.spawn((
        card(ArcadeCard::StartAt, Vec2::new(0.4, 0.1)),
        Transform::from_xyz(0.35, -0.02, z),
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
            Collider::cuboid(0.4, 0.08, 0.03),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            TextSource::Card,
            ScreenText {
                text: String::new(),
                size: Vec2::new(0.4, 0.08),
                style: ScreenStyle::Card,
            },
            Transform::from_xyz(0.35, -0.14 - i as f32 * 0.1, z),
            Visibility::default(),
            ChildOf(board),
        ));
    }
}

/// The gun board on its table: the card on the left, the answers to
/// "how did it feel?" down the right.
fn gun_board(commands: &mut Commands, table: Entity) {
    let board = commands
        .spawn((
            Name::new("Arcade gun board"),
            crate::camp::board_pose(),
            Visibility::default(),
            CampProps::Board,
            ChildOf(table),
        ))
        .id();
    let z = 0.016;
    commands.spawn((
        Name::new("Arcade gun card"),
        GunCardSheet,
        ScreenText {
            text: String::new(),
            size: SHEET,
            style: ScreenStyle::Paper,
        },
        // The camp's board on a camp table, read as the camp's sheet is:
        // standing at the table, within reach of its cards.
        TextSource::CampSheet,
        Transform::from_xyz(-0.24, 0.0, z),
        Visibility::default(),
        ChildOf(board),
    ));
    for n in 0..=5u8 {
        // 5 at the top, SKIP at the foot.
        let row = if n == 0 { 5.0 } else { (5 - n) as f32 };
        commands.spawn((
            card(ArcadeCard::Feel(n), Vec2::new(0.4, 0.09)),
            Transform::from_xyz(0.35, 0.3 - row * 0.115, z),
            ChildOf(board),
        ));
    }
}

/// Stands the player at the bench for a few frames (after set-up, and
/// after a reset): a physics body has its pose set there too.
#[allow(clippy::type_complexity)]
fn stand_at_bench(
    mut arcade: ResMut<Arcade>,
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
    if arcade.hold == 0 {
        return;
    }
    arcade.hold -= 1;
    let Some(stand) = arcade.stand else {
        return;
    };
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

fn press_cards(
    mut events: MessageReader<InteractEvent>,
    cards: Query<&ArcadeCard>,
    mut arcade: ResMut<Arcade>,
    mut feel: ResMut<super::record::Feel>,
    mut calls: MessageWriter<WaveCalled>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok(c) = cards.get(*item) else {
            continue;
        };
        match c {
            ArcadeCard::Call => {
                calls.write(WaveCalled);
            }
            ArcadeCard::Restart => {
                if arcade.reset == Reset::None {
                    arcade.reset = Reset::Dropped;
                }
            }
            ArcadeCard::StartAt => {
                // Only between runs: a run is one ladder.
                if matches!(arcade.phase, RunPhase::Ready | RunPhase::Over { .. }) {
                    let i = STARTS
                        .iter()
                        .position(|s| *s == arcade.start_at)
                        .unwrap_or(0);
                    arcade.start_at = STARTS[(i + 1) % STARTS.len()];
                }
            }
            ArcadeCard::Feel(n) => feel.answer(*n),
        }
    }
}

/// The board's sheet: what's on, what comes next and from where, the
/// ladder, the score, and once a run is over, its summary.
pub fn sheet(arcade: &Arcade) -> String {
    let mut s = String::from("THE ARCADE\n\n");
    match arcade.phase {
        RunPhase::Ready => s += "Take what you want.\nCALL WAVE when ready.\n",
        RunPhase::Warning { wave: n, left } => {
            s += &format!(
                "WAVE {n} IN {:.0} s\n{}\n{} GATE (lamp lit)\n",
                left.ceil(),
                describe(&wave(n)).to_uppercase(),
                GATES[gate(n)].0
            );
        }
        RunPhase::Fighting { wave: n } => {
            s += &format!(
                "WAVE {n}: {}\nin from the {} gate\n",
                describe(&wave(n)).to_uppercase(),
                GATES[gate(n)].0
            );
        }
        RunPhase::Cleared { wave: n } => s += &format!("WAVE {n} CLEARED\n"),
        RunPhase::Over { wave: n } => s += &format!("DOWN ON WAVE {n}\n"),
    }
    if !matches!(arcade.phase, RunPhase::Ready) {
        s += &format!(
            "Score {}   Time {}:{:02}\n",
            arcade.score,
            arcade.run_t as u32 / 60,
            arcade.run_t as u32 % 60
        );
    }
    if let (RunPhase::Over { .. }, Some(sum)) = (arcade.phase, &arcade.summary) {
        s += "\n";
        s += sum;
        return s;
    }
    // The ladder from the next wave on: nothing comes that isn't here.
    let next = arcade.next_wave();
    s += "\nNEXT:\n";
    for n in next..next + 4 {
        s += &format!(
            "{n}. {} ({})\n",
            describe(&wave(n)),
            GATES[gate(n)].0.to_lowercase()
        );
    }
    if arcade.best != (0, 0) {
        s += &format!("\nBest: wave {}, score {}\n", arcade.best.0, arcade.best.1);
    }
    s
}

fn sheet_text(arcade: Res<Arcade>, mut sheets: Query<&mut ScreenText, With<ArcadeSheet>>) {
    let text = sheet(&arcade);
    for mut s in &mut sheets {
        if s.text != text {
            s.text = text.clone();
        }
    }
}

fn gun_card_text(
    arcade: Res<Arcade>,
    rec: Res<super::record::Recording>,
    bests: Res<super::record::GunBests>,
    feel: Res<super::record::Feel>,
    mut sheets: Query<&mut ScreenText, With<GunCardSheet>>,
) {
    let text = super::record::card(&arcade, &rec, &bests, &feel);
    for mut s in &mut sheets {
        if s.text != text {
            s.text = text.clone();
        }
    }
}

fn card_text(
    arcade: Res<Arcade>,
    feel: Res<super::record::Feel>,
    mut cards: Query<(&ArcadeCard, &mut ScreenText)>,
) {
    for (c, mut s) in &mut cards {
        let text = match c {
            ArcadeCard::Call => match arcade.phase {
                RunPhase::Warning { .. } | RunPhase::Fighting { .. } => {
                    format!("WAVE {} ON", arcade.wave())
                }
                RunPhase::Over { .. } => "NEW RUN".to_string(),
                _ => format!("CALL WAVE {}", arcade.next_wave()),
            },
            ArcadeCard::Restart => "RESTART".to_string(),
            ArcadeCard::StartAt => format!("START AT WAVE {}", arcade.start_at),
            // Blank until there's a question.
            ArcadeCard::Feel(_) if feel.gun().is_none() => String::new(),
            ArcadeCard::Feel(0) => "SKIP".to_string(),
            ArcadeCard::Feel(n) => format!("{n}  {}", FEEL_WORDS[*n as usize - 1]),
        };
        if s.text != text {
            s.text = text;
        }
    }
}

/// RESTART, in two frames: everything carried is dropped where you
/// stand (as on going down; the raid's own `drop_everything` does it),
/// then the next frame the ground is swept of it, the machines and every
/// loose thing, you're healed and stood at the bench, and the tables are
/// laid again.
fn sweep(world: &mut World) {
    let step = world.resource::<Arcade>().reset;
    match step {
        Reset::None => {}
        Reset::Dropped => {
            let player = world
                .query_filtered::<(Entity, &Transform), With<LocalPlayer>>()
                .iter(world)
                .next()
                .map(|(e, t)| (e, t.translation));
            if let Some((player, at)) = player {
                world.write_message(crate::raid::truck::DropEverything { player, at });
            }
            world.resource_mut::<Arcade>().reset = Reset::Sweep;
        }
        Reset::Sweep => {
            let mut gone: Vec<Entity> = Vec::new();
            gone.extend(
                world
                    .query_filtered::<Entity, With<super::waves::WaveMachine>>()
                    .iter(world),
            );
            gone.extend(
                world
                    .query_filtered::<Entity, With<crate::raid::Remains>>()
                    .iter(world),
            );
            gone.extend(
                world
                    .query_filtered::<Entity, (
                        With<crate::items::Item>,
                        Without<crate::interact::Held>,
                        Without<crate::interact::InSocket>,
                        Without<crate::interact::body::InBackpack>,
                        Without<ChildOf>,
                    )>()
                    .iter(world),
            );
            gone.sort();
            gone.dedup();
            let n = gone.len();
            for e in gone {
                if let Ok(em) = world.get_entity_mut(e) {
                    em.despawn();
                }
            }
            heal(world);
            let mut a = world.resource_mut::<Arcade>();
            a.reset = Reset::None;
            a.phase = RunPhase::Ready;
            a.score = 0;
            a.run_t = 0.0;
            a.summary = None;
            if world
                .resource::<crate::launch::LaunchOptions>()
                .camera
                .is_none()
            {
                world.resource_mut::<Arcade>().hold = 10;
            }
            world.resource_mut::<Laid>().0 = false;
            info!("arcade: restart, {n} things swept");
        }
    }
}

/// Whole again: health, wounds, a broken leg.
fn heal(world: &mut World) {
    let players: Vec<Entity> = world
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(world)
        .collect();
    for p in players {
        if let Some(mut h) = world.get_mut::<crate::health::Health>(p) {
            *h = crate::health::Health::default();
        }
        if let Some(mut w) = world.get_mut::<crate::interact::wounds::Wounds>(p) {
            w.0.clear();
        }
        world.entity_mut(p).remove::<(
            crate::interact::wounds::Fracture,
            crate::interact::wounds::Bloodied,
        )>();
    }
}

/// Lays the kit on the tables (at set-up and after each reset).
fn lay_tables(world: &mut World) {
    if world.resource::<Laid>().0 {
        return;
    }
    let Some(bench) = world.resource::<Arcade>().bench else {
        return;
    };
    let zone = world.resource::<Zone>().clone();
    let lie = |l: Lie| match l {
        Lie::Long => {
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)
        }
        Lie::Side => Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        Lie::Flat => Quat::IDENTITY,
    };
    let mut n = 0;
    for t in 0..TABLES.len() {
        let pose = table_pose(&zone, &bench, t);
        let list: &[Kit] = match t {
            t if t < KIT_TABLES => kit(t),
            BOARD_TABLE => &BOARD_TABLE_KIT,
            _ => &[],
        };
        for (id, rounds, inserted, [x, z], l) in list.iter().copied() {
            let at = pose * Transform::from_xyz(x, TABLE.y + 0.06, z).with_rotation(lie(l));
            let mut saved = saved(id, rounds);
            if let Some((m, r)) = inserted {
                saved.inserted.push(self::saved(m, r));
            }
            if crate::persist::restore_item(world, &saved, at).is_some() {
                n += 1;
            }
        }
    }
    world.resource_mut::<Laid>().0 = true;
    info!("arcade: {n} things laid on the bench");
    // Shots only: `EXTRACTION_ARCADE_CALL=1` calls a wave once the bench is
    // laid, so a screenshot shows the board's warning and the gate's lamp.
    if std::env::var("EXTRACTION_ARCADE_CALL").is_ok_and(|v| v == "1") {
        world.write_message(WaveCalled);
    }
}

fn saved(id: &str, rounds: u32) -> SavedItem {
    SavedItem {
        id: id.into(),
        condition: 0.9,
        rounds,
        charge_wh: -1.0,
        local: [0.0; 3],
        turn: [0.0, 0.0, 0.0, 1.0],
        gun: None,
        inserted: Vec::new(),
        loads: Default::default(),
    }
}

/// The board's sheet and the gun card at their fullest, for the
/// legibility gauge (`render::legibility`): the furthest wave's call, a
/// run's end with every machine down, and the gun card with more guns than
/// it lists.
#[cfg(test)]
pub fn fullest_panels() -> Vec<(String, ScreenText, TextSource)> {
    let mut a = Arcade {
        start_at: 7,
        best: (99, 99999),
        ..default()
    };
    let mut texts = vec![("sheet, ready".to_string(), sheet(&a))];
    a.phase = RunPhase::Warning {
        wave: 99,
        left: 10.0,
    };
    a.score = 99999;
    a.run_t = 5999.0;
    texts.push(("sheet, wave called".into(), sheet(&a)));
    a.phase = RunPhase::Fighting { wave: 99 };
    texts.push(("sheet, fighting".into(), sheet(&a)));
    let mut run = super::stats::RunStats {
        cause: Some("heavy".into()),
        waves_cleared: 98,
        ..default()
    };
    for m in ["walker", "drone", "hound", "heavy"] {
        run.machines.insert(
            m.into(),
            super::stats::KillStats {
                kills: 999,
                ttk: vec![111.0],
                shots: 1,
                hits: 1,
            },
        );
    }
    a.phase = RunPhase::Over { wave: 99 };
    a.summary = Some(run.board(99, 99999, 5999.0, true));
    texts.push(("sheet, down".into(), sheet(&a)));
    texts.push(("gun card".into(), super::stats::fullest_card()));
    texts
        .into_iter()
        .map(|(what, text)| {
            (
                format!("arcade {what}"),
                ScreenText {
                    text,
                    size: SHEET,
                    style: ScreenStyle::Paper,
                },
                TextSource::CampSheet,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_thing_on_the_bench_is_in_the_catalog() {
        let catalog = crate::items::Catalog::load();
        let all = (0..KIT_TABLES)
            .flat_map(|t| kit(t).iter())
            .chain(BOARD_TABLE_KIT.iter());
        for (id, _, inserted, ..) in all {
            assert!(catalog.kind(id).is_some(), "{id}");
            if let Some((m, _)) = inserted {
                assert!(catalog.kind(m).is_some(), "{m}");
            }
        }
    }

    #[test]
    fn the_sheet_says_what_comes_before_it_comes() {
        let mut a = Arcade {
            start_at: 1,
            ..default()
        };
        let s = sheet(&a);
        assert!(s.contains("1. walker (north)"), "{s}");
        assert!(s.contains("4. hound (north)"), "{s}");
        a.phase = RunPhase::Warning { wave: 3, left: 7.2 };
        let s = sheet(&a);
        assert!(s.contains("WAVE 3 IN 8 s"), "{s}");
        assert!(s.contains("WALKER + DRONE"), "{s}");
        assert!(s.contains("WEST GATE"), "{s}");
    }
}
