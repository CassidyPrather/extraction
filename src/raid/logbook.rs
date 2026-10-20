//! The logbook: a card on the truck bed's left wall, by the tailgate. After
//! each raid it says how it went — how long you were out, what you wrecked,
//! what came home with you (or what the convoy took) — under the career's
//! running record.

use bevy::prelude::*;
use std::collections::BTreeMap;

use super::truck::{FLOOR, Truck};
use super::{Phase, Raid};
use crate::anomaly::artifacts::CarriedBy;
use crate::items::{Catalog, Item};
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::rig::LocalPlayer;
use crate::robots::{PartDestroyed, PartRole, Robot, RobotKind};

/// How the last raid went.
#[derive(Clone, Debug, PartialEq)]
pub struct RaidEntry {
    pub number: u32,
    pub home: bool,
    pub minutes: f32,
    /// Robots wrecked, by name.
    pub wrecked: BTreeMap<&'static str, u32>,
    /// Things carried home (or lost, if down).
    pub items: usize,
    /// Artifacts among them, by name.
    pub artifacts: Vec<String>,
}

#[derive(Resource, Default, Debug)]
pub struct Logbook {
    /// Tallied while out.
    wrecked: BTreeMap<&'static str, u32>,
    started: f32,
    pub last: Option<RaidEntry>,
    /// The last raid as the card shows it (saved with the career).
    pub written: Option<String>,
    /// Turned to the career page.
    pub career: bool,
}

#[derive(Component)]
pub struct LogbookCard;

pub struct LogbookPlugin;

impl Plugin for LogbookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Logbook>().add_systems(
            Update,
            (
                spawn_card,
                tally_wrecks,
                write_entries,
                turn_page,
                card_text,
            )
                .chain()
                .after(super::depart)
                .after(super::downed),
        );
    }
}

fn spawn_card(mut commands: Commands, trucks: Query<Entity, Added<Truck>>) {
    for truck in &trucks {
        commands.spawn((
            Name::new("Logbook"),
            LogbookCard,
            super::truck::Pressable::Logbook,
            crate::interact::Grabbable {
                points: vec![crate::interact::GrabPoint::free()],
            },
            avian3d::prelude::Collider::cuboid(0.34, 0.25, 0.03),
            avian3d::prelude::CollisionLayers::new(
                crate::world::Layer::Item,
                avian3d::prelude::LayerMask::NONE,
            ),
            ScreenText {
                text: String::new(),
                size: Vec2::new(0.34, 0.25),
                style: ScreenStyle::Card,
            },
            Transform::from_xyz(-1.12, FLOOR + 1.66, 0.8)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
            ChildOf(truck),
        ));
    }
}

fn robot_name(kind: RobotKind, grappler: bool, collector: bool) -> &'static str {
    match kind {
        RobotKind::Drone => "drone",
        RobotKind::Walker if grappler => "grappler",
        RobotKind::Walker if collector => "collector",
        RobotKind::Walker => "walker",
        RobotKind::Dirigible => "airship",
        RobotKind::Pylon => "pylon",
    }
}

fn tally_wrecks(
    mut destroyed: MessageReader<PartDestroyed>,
    robots: Query<(
        &Robot,
        Has<crate::robots::grappler::Grappler>,
        Has<crate::robots::collector::Collector>,
    )>,
    raid: Res<Raid>,
    mut book: ResMut<Logbook>,
) {
    for d in destroyed.read() {
        if d.role != PartRole::Core || raid.phase == Phase::AtBase {
            continue;
        }
        if let Ok((r, g, c)) = robots.get(d.robot) {
            *book.wrecked.entry(robot_name(r.kind, g, c)).or_default() += 1;
        }
    }
}

/// Opens an entry stepping out, closes it getting out or going down.
fn write_entries(
    raid: Res<Raid>,
    mut book: ResMut<Logbook>,
    players: Query<Entity, With<LocalPlayer>>,
    carried: Query<(&Item, &CarriedBy)>,
    catalog: Res<Catalog>,
    mut was: Local<Option<std::mem::Discriminant<Phase>>>,
) {
    let now = std::mem::discriminant(&raid.phase);
    let before = was.replace(now);
    if before == Some(now) {
        return;
    }
    let back = std::mem::discriminant(&Phase::AtBase);
    if before == Some(back) && raid.phase == Phase::Out {
        book.wrecked.clear();
        book.started = raid.clock;
        return;
    }
    let home = matches!(raid.phase, Phase::Extracting { .. });
    if !home && !matches!(raid.phase, Phase::Down { .. }) {
        return;
    }
    let Some(player) = players.iter().next() else {
        return;
    };
    let mut items = 0;
    let mut artifacts = Vec::new();
    for (item, by) in &carried {
        if by.0 != player {
            continue;
        }
        items += 1;
        let def = catalog.get(item.kind());
        if def.category == extraction_core::items::Category::Artifact {
            artifacts.push(def.name.clone());
        }
    }
    artifacts.sort();
    let entry = RaidEntry {
        number: raid.raids,
        home,
        minutes: (raid.clock - book.started).max(0.0) / 60.0,
        wrecked: std::mem::take(&mut book.wrecked),
        items,
        artifacts,
    };
    book.written = Some(entry_text(&entry));
    book.last = Some(entry);
}

/// "3 drones, 1 walker".
fn wreck_list(w: &BTreeMap<&'static str, u32>) -> String {
    let parts: Vec<String> = w
        .iter()
        .map(|(name, n)| {
            if *n == 1 {
                format!("1 {name}")
            } else {
                format!("{n} {name}s")
            }
        })
        .collect();
    if parts.is_empty() {
        "nothing".into()
    } else {
        parts.join(", ")
    }
}

fn entry_text(e: &RaidEntry) -> String {
    let mut lines = vec![format!(
        "RAID {}: {}, {:.0} MIN",
        e.number,
        if e.home { "HOME" } else { "LOST" },
        e.minutes.max(1.0)
    )];
    lines.push(format!("Wrecked {}", wreck_list(&e.wrecked)));
    if e.home {
        lines.push(format!("Brought home {} things", e.items));
    } else {
        lines.push(format!("The convoy took {} things", e.items));
    }
    if !e.artifacts.is_empty() {
        lines.push(e.artifacts.join(", "));
    }
    lines.join("\n")
}

/// Pressing the logbook turns it over: the last raid, or the career.
fn turn_page(
    mut events: MessageReader<crate::interact::InteractEvent>,
    cards: Query<(), With<LogbookCard>>,
    mut book: ResMut<Logbook>,
) {
    for ev in events.read() {
        if let crate::interact::InteractEvent::Pressed { item, .. } = ev {
            if cards.contains(*item) {
                book.career = !book.career;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn card_text(
    raid: Res<Raid>,
    book: Res<Logbook>,
    progression: Res<crate::progression::Progression>,
    epilogue: Res<crate::progression::Epilogue>,
    memorial: Res<crate::persist::Memorial>,
    stashes: Query<&crate::interact::body::Backpack, With<super::truck::Stash>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut cards: Query<&mut ScreenText, With<LogbookCard>>,
    added: Query<(), Added<LogbookCard>>,
    mut last: Local<String>,
) {
    let text = if book.career {
        let keys_home = stashes
            .iter()
            .flat_map(|s| s.contents.iter())
            .filter_map(|e| items.get(*e).ok())
            .filter(|i| {
                let id = &catalog.get(i.kind()).id;
                crate::capstone::KEYS.iter().any(|(k, _)| k == id)
            })
            .count();
        let page = crate::progression::career_page(
            &progression.0,
            &raid,
            &memorial.0,
            keys_home,
            epilogue.0,
        );
        format!("{page}\n\n(press: last raid)")
    } else {
        let record = format!(
            "{} raids, {} home, {} lost",
            raid.raids, raid.extractions, raid.deaths
        );
        match &book.written {
            Some(e) => format!("LOGBOOK\n{e}\n\n{record}\n(press: career)"),
            None => format!("LOGBOOK\n\n{record}\n(press: career)"),
        }
    };
    // Fitting the text is not free: only when it changes.
    if *last == text && added.is_empty() {
        return;
    }
    *last = text.clone();
    for mut c in &mut cards {
        let text = crate::render::screen::wrap_to_fit(&text, c.size, c.style);
        if c.text != text {
            c.text = text.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raid_home_is_written_up() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        assert!(
            app.world_mut()
                .query_filtered::<(), With<LogbookCard>>()
                .iter(app.world())
                .count()
                == 1,
            "one logbook in the truck"
        );
        {
            let mut raid = app.world_mut().resource_mut::<Raid>();
            raid.phase = Phase::Out;
            raid.raids += 1;
        }
        app.update();
        // A drone goes down while we're out.
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let drone = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::robots::drone::spawn_drone(
                &mut c,
                Vec3::new(0.0, 80.0, 0.0),
                crate::robots::Job {
                    kind: crate::robots::JobKind::Watch,
                    site: Vec3::ZERO,
                    duration: 999.0,
                    exit: Vec3::ZERO,
                },
            )
        };
        queue.apply(app.world_mut());
        app.update();
        app.world_mut().write_message(PartDestroyed {
            robot: drone,
            part: drone,
            role: PartRole::Core,
            at: Vec3::ZERO,
        });
        app.world_mut().resource_mut::<Raid>().clock += 600.0;
        app.update();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Extracting { left: 4.0 };
        app.update();
        let e = app
            .world()
            .resource::<Logbook>()
            .last
            .clone()
            .expect("an entry");
        assert!(e.home);
        assert_eq!(e.wrecked.get("drone"), Some(&1));
        assert!((e.minutes - 10.0).abs() < 0.5, "{}", e.minutes);
        app.update();
        let text = app
            .world_mut()
            .query_filtered::<&ScreenText, With<LogbookCard>>()
            .iter(app.world())
            .next()
            .unwrap()
            .text
            .clone();
        assert!(text.contains("HOME, 10 MIN"), "{text}");
        assert!(text.contains("Wrecked 1 drone"), "{text}");
    }
}
