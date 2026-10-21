//! Offerings: the Zone's strangest sink. A few places (the old well, the
//! stone ring) take what you leave in them. Leave something and step back;
//! after a moment it's gone, and something is given back, once a raid per
//! place:
//!
//! - **Food** buys *calm*: the Zone stops paying attention to you — the
//!   director's account is wiped and pending response teams are called
//!   off.
//! - **Curios** (the old watch, the photograph, the chess piece) buy
//!   *information*: a voice tells you where an artifact is lying this
//!   cycle.
//! - **Artifacts** buy *time*: the Surge holds off ten minutes more.
//!
//! Anything else is taken without a word.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::anomaly::layout;
use extraction_core::items::Category;
use extraction_core::map::MarkerKind;

use super::AttentionBudget;
use crate::interact::Held;
use crate::interact::body::Backpack;
use crate::items::{Catalog, Item};
use crate::launch::LaunchOptions;
use crate::raid::{Phase, Raid};
use crate::world::{Layer, Zone};

#[derive(Component, Debug)]
pub struct Offering {
    pub name: String,
    /// Seconds since something was last put in (it waits for you to step
    /// back).
    pub settle: f32,
    /// Raid number this place last answered in.
    pub answered: Option<u32>,
}

/// Something was accepted (for visuals and sound).
#[derive(Message, Clone, Debug)]
pub struct OfferingTaken {
    pub at: Vec3,
    pub boon: Boon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boon {
    Calm,
    Information,
    Time,
    Nothing,
}

/// Minutes the Surge holds off for an artifact.
pub const TIME_BOON: f32 = 10.0 * 60.0;
/// ...and for a stormglass at the well, which drinks it down.
pub const LONG_TIME_BOON: f32 = 30.0 * 60.0;

/// The few places that answer one stone in their own way (`docs/LORE.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The Surge held off far longer.
    LongTime,
    /// Taken back: the stone rolls out at your feet.
    Refused,
    /// The Zone goes quiet, as for food.
    Calm,
    /// A stone's whereabouts, as for a curio.
    Whisper,
}

/// How `place` answers stone `id`, if in its own way: what it does, and
/// its line.
pub fn special_answer(place: &str, id: &str) -> Option<(Answer, &'static str)> {
    Some(match (place, id) {
        ("The well", "art_stormglass") => (
            Answer::LongTime,
            "The well drinks it down. The sky goes still, like after snow.",
        ),
        ("The cairn", "art_lodestone") => (
            Answer::Refused,
            "The cairn won't have it. The stone rolls back to your feet.",
        ),
        ("The burnt chapel", "art_hearth") => (
            Answer::Calm,
            "The ash takes it warm. Everything goes slow and quiet.",
        ),
        ("The chapel font", "art_shard") => (
            Answer::Whisper,
            "The water shows you a stone, not far from here.",
        ),
        _ => return None,
    })
}

pub struct OfferingPlugin;

impl Plugin for OfferingPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OfferingTaken>()
            .add_systems(Startup, spawn_offerings)
            .add_systems(Update, (own_offerings, accept_offerings).chain());
    }
}

fn spawn_offerings(mut commands: Commands, zone: Res<Zone>) {
    for m in zone.map.markers_of(&MarkerKind::Offering) {
        let at = zone.on_ground(m.at[0], m.at[1]);
        commands
            .spawn((
                Name::new(m.name.clone()),
                Offering {
                    name: m.name.clone(),
                    settle: 0.0,
                    answered: None,
                },
                Backpack {
                    owner: Entity::PLACEHOLDER,
                    open: true,
                    model: extraction_core::inventory::Backpack {
                        capacity: 40.0,
                        mass: 0.0,
                    },
                    contents: Vec::new(),
                    carried: false,
                    half: Vec3::new(0.45, 0.35, 0.45),
                },
                Transform::from_translation(at + Vec3::Y * 0.7),
                Visibility::default(),
            ))
            .with_child((
                // A lip to stand at; the opening itself is the container.
                Collider::cylinder(0.75, 0.6),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                Transform::from_xyz(0.0, -0.4, 0.0),
            ));
    }
}

fn own_offerings(
    mut sites: Query<&mut Backpack, With<Offering>>,
    locals: Query<Entity, Added<crate::rig::LocalPlayer>>,
) {
    for p in &locals {
        for mut b in &mut sites {
            b.owner = p;
        }
    }
}

fn accept_offerings(
    mut commands: Commands,
    mut sites: Query<(&mut Offering, &mut Backpack, &GlobalTransform)>,
    items: Query<&Item, Without<Held>>,
    players: Query<&GlobalTransform, With<crate::rig::LocalPlayer>>,
    catalog: Res<Catalog>,
    mut raid: ResMut<Raid>,
    mut budget: ResMut<AttentionBudget>,
    director: Option<ResMut<crate::robots::director::Director>>,
    zone: Res<Zone>,
    opts: Res<LaunchOptions>,
    time: Res<Time>,
    mut taken: MessageWriter<OfferingTaken>,
) {
    let mut director = director;
    let dt = time.delta_secs();
    for (mut site, mut bag, gt) in &mut sites {
        if bag.contents.is_empty() {
            site.settle = 0.0;
            continue;
        }
        // Wait for the giver to step back.
        let near = players
            .iter()
            .any(|p| p.translation().distance(gt.translation()) < 1.6);
        site.settle = if near { 0.0 } else { site.settle + dt };
        if site.settle < 2.5 {
            continue;
        }
        let gifts: Vec<Entity> = bag.contents.drain(..).collect();
        let mut kinds = Vec::new();
        let mut special = None;
        for e in &gifts {
            let id = items.get(*e).map(|it| catalog.get(it.kind()).id.clone());
            if let Some((answer, line)) = id
                .as_deref()
                .ok()
                .and_then(|id| special_answer(&site.name, id))
            {
                if answer == Answer::Refused {
                    // Back out at the giver's feet, as it went in (a new
                    // body: the old one's physics were folded away in the
                    // bag).
                    if let Ok(it) = items.get(*e) {
                        let out = gt.translation() + Vec3::new(0.0, -0.3, 0.9);
                        crate::items::spawn_item(
                            &mut commands,
                            &catalog,
                            crate::items::SpawnItem {
                                kind: it.kind(),
                                transform: Transform::from_translation(out),
                                condition: it.condition,
                            },
                        );
                    }
                    commands.entity(*e).despawn();
                    raid.say(line);
                    continue;
                }
                special.get_or_insert((answer, line));
            }
            if let Ok(it) = items.get(*e) {
                kinds.push(catalog.get(it.kind()));
            }
            commands.entity(*e).despawn();
        }
        if kinds.is_empty() {
            continue;
        }
        let has = |c: Category| kinds.iter().any(|d| d.category == c);
        let already = site.answered == Some(raid.raids);
        let at_base = already || raid.phase == Phase::AtBase;
        let boon = if at_base {
            Boon::Nothing
        } else if let Some((answer, _)) = special {
            match answer {
                Answer::LongTime => Boon::Time,
                // Never kept, so never answered (it rolled back out).
                Answer::Refused => Boon::Nothing,
                Answer::Calm => Boon::Calm,
                Answer::Whisper => Boon::Information,
            }
        } else if has(Category::Artifact) {
            Boon::Time
        } else if has(Category::Curio) {
            Boon::Information
        } else if has(Category::Food) {
            Boon::Calm
        } else {
            Boon::Nothing
        };
        if boon != Boon::Nothing {
            site.answered = Some(raid.raids);
        }
        // A place answering a stone its own way says so in its own words.
        let line = special.filter(|_| !at_base).map(|(_, l)| l);
        match boon {
            Boon::Time => {
                let long = special.is_some_and(|(a, _)| a == Answer::LongTime) && !at_base;
                raid.clock -= if long { LONG_TIME_BOON } else { TIME_BOON };
                match line {
                    Some(l) => raid.say(l),
                    None => raid.say(format!(
                        "{}: the air loosens. The Surge feels further off.",
                        site.name
                    )),
                }
            }
            Boon::Information => {
                let here = gt.translation();
                let hint = layout(&zone.map, &zone.hm, opts.seed, raid.cycle)
                    .into_iter()
                    .filter(|a| a.artifact.is_some())
                    .min_by(|a, b| {
                        let da = Vec2::new(a.at[0] - here.x, a.at[1] - here.z).length();
                        let db = Vec2::new(b.at[0] - here.x, b.at[1] - here.z).length();
                        da.total_cmp(&db)
                    });
                if let Some(l) = line {
                    raid.say(l);
                }
                let text = match hint {
                    Some(a) => {
                        let d = Vec2::new(a.at[0] - here.x, a.at[1] - here.z);
                        let place = zone
                            .map
                            .regions
                            .iter()
                            .find(|r| extraction_core::geom::point_in_polygon(&r.polygon, a.at))
                            .map(|r| r.name.clone())
                            .unwrap_or_else(|| "the trees".into());
                        format!(
                            "{}: a voice, very close: \"{} metres {}, in {}. It hums.\"",
                            site.name,
                            (d.length() / 10.0).round() * 10.0,
                            bearing(d),
                            place
                        )
                    }
                    None => format!(
                        "{}: a voice, very close: \"Nothing grows this time.\"",
                        site.name
                    ),
                };
                raid.say(text);
            }
            Boon::Calm => {
                budget.budget = 0.0;
                if let Some(d) = director.as_mut() {
                    d.responses.clear();
                }
                match line {
                    Some(l) => raid.say(l),
                    None => raid.say(format!("{}: the Zone goes quiet around you.", site.name)),
                }
            }
            Boon::Nothing => {
                raid.say(format!("{}: it's gone. Nothing answers.", site.name));
            }
        }
        taken.write(OfferingTaken {
            at: gt.translation(),
            boon,
        });
    }
}

/// A compass word for a direction on the map (north is -Z).
pub(crate) fn bearing(d: Vec2) -> &'static str {
    let a = d.x.atan2(-d.y).to_degrees().rem_euclid(360.0);
    const NAMES: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    NAMES[(((a + 22.5) / 45.0) as usize) % 8]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::body::InBackpack;

    #[test]
    fn bearings() {
        assert_eq!(bearing(Vec2::new(0.0, -10.0)), "north");
        assert_eq!(bearing(Vec2::new(10.0, 0.0)), "east");
        assert_eq!(bearing(Vec2::new(0.0, 10.0)), "south");
        assert_eq!(bearing(Vec2::new(-10.0, -10.0)), "north-west");
    }

    /// Offers `id` at `place` on `map`, out in the Zone at clock 100 s;
    /// returns the app and the offered item.
    fn offer(map: &str, place: &str, id: &str) -> (App, Entity) {
        let opts = LaunchOptions {
            robots: false,
            map: map.into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let bag_e = app
            .world_mut()
            .query::<(Entity, &Offering)>()
            .iter(app.world())
            .find(|(_, o)| o.name == place)
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("{map} has {place}"));
        {
            let mut raid = app.world_mut().resource_mut::<Raid>();
            raid.phase = Phase::Out;
            raid.raids = 1;
            raid.clock = 100.0;
        }
        let catalog = app.world().resource::<Catalog>().clone();
        let kind = catalog.kind(id).unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let e = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::IDENTITY,
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.world_mut()
            .get_mut::<Backpack>(bag_e)
            .unwrap()
            .contents
            .push(e);
        app.world_mut().entity_mut(e).insert((
            InBackpack { bag: bag_e },
            ChildOf(bag_e),
            RigidBodyDisabled,
            ColliderDisabled,
        ));
        for _ in 0..90 * 4 {
            app.update();
        }
        (app, e)
    }

    fn said(app: &App, text: &str) -> bool {
        app.world()
            .resource::<Raid>()
            .log
            .iter()
            .any(|(l, _)| l.contains(text))
    }

    #[test]
    fn the_well_drinks_a_stormglass_and_holds_the_surge_off_long() {
        let (app, e) = offer("marrow_creek", "The well", "art_stormglass");
        assert!(app.world().get_entity(e).is_err(), "taken");
        let clock = app.world().resource::<Raid>().clock;
        assert!(
            clock < 100.0 - LONG_TIME_BOON + 10.0,
            "held off long (clock {clock})"
        );
        assert!(said(&app, "The well drinks it down"));
    }

    #[test]
    fn the_cairn_wont_have_a_lodestone() {
        let (mut app, _) = offer("kettle_pass", "The cairn", "art_lodestone");
        let catalog = app.world().resource::<Catalog>().clone();
        let loose = app
            .world_mut()
            .query_filtered::<&Item, Without<InBackpack>>()
            .iter(app.world())
            .filter(|i| catalog.get(i.kind()).id == "art_lodestone")
            .count();
        assert_eq!(loose, 1, "it rolled back out, loose at your feet");
        let clock = app.world().resource::<Raid>().clock;
        assert!(clock > 100.0, "and nothing given for it (clock {clock})");
        assert!(said(&app, "The cairn won't have it"));
    }

    #[test]
    fn the_burnt_chapel_and_the_font_answer_their_stones() {
        let (app, _) = offer("cinder_works", "The burnt chapel", "art_hearth");
        assert!(said(&app, "The ash takes it warm"));
        assert_eq!(app.world().resource::<AttentionBudget>().budget, 0.0);
        let (app, _) = offer("kettle_pass", "The chapel font", "art_shard");
        assert!(said(&app, "The water shows you a stone"));
        // The usual stone's time is not given for it: it whispers instead.
        assert!(app.world().resource::<Raid>().clock > 100.0);
    }

    #[test]
    fn an_artifact_at_the_well_holds_off_the_surge() {
        let opts = LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let (well, bag_e) = app
            .world_mut()
            .query::<(Entity, &Offering)>()
            .iter(app.world())
            .map(|(e, o)| (o.name.clone(), e))
            .find(|(n, _)| n == "The well")
            .map(|(n, e)| (n, e))
            .unwrap();
        let _ = well;
        {
            let mut raid = app.world_mut().resource_mut::<Raid>();
            raid.phase = Phase::Out;
            raid.raids = 1;
            raid.clock = 100.0;
        }
        let catalog = app.world().resource::<Catalog>().clone();
        let kind = catalog.kind("art_hearth").unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let e = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::IDENTITY,
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.world_mut()
            .get_mut::<Backpack>(bag_e)
            .unwrap()
            .contents
            .push(e);
        app.world_mut().entity_mut(e).insert((
            InBackpack { bag: bag_e },
            ChildOf(bag_e),
            RigidBodyDisabled,
            ColliderDisabled,
        ));
        for _ in 0..90 * 4 {
            app.update();
        }
        assert!(app.world().get_entity(e).is_err(), "the hearth was taken");
        let clock = app.world().resource::<Raid>().clock;
        assert!(
            clock < 100.0 - TIME_BOON + 10.0,
            "the Surge held off (clock {clock})"
        );
    }
}
