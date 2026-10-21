//! The camp kitchen beside the truck at base: a folding table with a
//! board, a bowl, a pot, a pan, a tin oven and a crock. Cosmetic — there is
//! no hunger — and run by the cooking graph lifted whole from the
//! prototypes (`extraction_core::cooking`).
//!
//! Drop food into a station. Board and bowl are worked: grab the knife or
//! whisk (each grab is a stroke). Pot, pan, oven and crock run on the
//! clock: turn the knob to start, and take it off before it burns. What a
//! station makes comes out as an item in it, to eat or to cook further.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::cooking::{Drive, Event, Food, Process, Station, TICKS_PER_SEC};
use extraction_core::map::MarkerKind;

use super::Raid;
use super::bench::{ScreenStyle, ScreenText};
use super::truck::Pressable;
use crate::interact::body::{Backpack, InBackpack};
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::world::{Layer, Zone};

/// Something happened at a station: a stroke of work, a dish done (for
/// its sounds, in `audio`). `process` is the station's first.
#[derive(Message, Clone, Copy, Debug)]
pub struct Cooking {
    pub station: Entity,
    pub process: Process,
    pub what: Event,
}

/// One cooking station, with the container food goes into.
#[derive(Component)]
pub struct Cook {
    pub name: &'static str,
    pub station: Station,
    pub tray: Entity,
    /// Seconds of clock not yet turned into ticks.
    pub carry: f32,
    /// The knob was turned with something finished in it: take it off.
    pub take: bool,
}

/// On food a station made, so it isn't put straight back in.
#[derive(Component)]
pub struct MadeAt(pub Entity);

/// A station's container.
#[derive(Component)]
pub struct CookTray;

/// The station's tool (knife, whisk) or knob.
#[derive(Component)]
pub struct CookControl(pub Entity);

const TABLE_HEIGHT: f32 = 0.78;

/// (name, processes, place on the table).
const STATIONS: &[(&str, &[Process], f32)] = &[
    ("Board", &[Process::Chop], -0.75),
    (
        "Bowl",
        &[Process::Mix, Process::Knead, Process::Churn, Process::Grind],
        -0.45,
    ),
    ("Pot", &[Process::Boil, Process::Steep], -0.15),
    ("Pan", &[Process::Fry], 0.15),
    ("Tin oven", &[Process::Bake], 0.45),
    ("Crock", &[Process::Soak, Process::Prove], 0.75),
];

pub struct KitchenPlugin;

impl Plugin for KitchenPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Cooking>()
            .add_systems(
                Startup,
                spawn_kitchen.after(crate::world::buildings::spawn_buildings),
            )
            .add_systems(
                Update,
                (
                    own_trays,
                    forget_made,
                    take_in_food,
                    use_controls,
                    cook,
                    label,
                )
                    .chain(),
            );
    }
}

fn spawn_kitchen(mut commands: Commands, zone: Res<Zone>) {
    let Some(stop) = zone.marker(&MarkerKind::TruckStop) else {
        return;
    };
    // Beside the truck, off the road.
    let at = [stop.at[0] - 4.5, stop.at[1] - 1.0];
    let ground = zone.ground(at[0], at[1]);
    let kitchen = commands
        .spawn((
            Name::new("Camp kitchen"),
            Transform::from_xyz(at[0], ground, at[1])
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
        ))
        .id();
    commands.entity(kitchen).with_children(|c| {
        c.spawn((
            Collider::cuboid(1.9, 0.04, 0.6),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
            Transform::from_xyz(0.0, TABLE_HEIGHT - 0.02, 0.0),
            KitchenTable,
        ));
    });
    for (name, processes, x) in STATIONS {
        let station = Station::new(processes);
        let tray = commands
            .spawn((
                Name::new(format!("{name} (food)")),
                CookTray,
                Backpack {
                    owner: Entity::PLACEHOLDER,
                    open: true,
                    model: extraction_core::inventory::Backpack {
                        capacity: 6.0,
                        mass: 0.0,
                    },
                    contents: Vec::new(),
                    carried: false,
                    half: Vec3::new(0.12, 0.08, 0.14),
                },
                Transform::from_xyz(*x, TABLE_HEIGHT + 0.08, 0.0),
                Visibility::default(),
                ChildOf(kitchen),
            ))
            .id();
        let cook = commands
            .spawn((
                Name::new(*name),
                Cook {
                    name,
                    station,
                    tray,
                    carry: 0.0,
                    take: false,
                },
                Transform::from_xyz(*x, TABLE_HEIGHT, 0.0),
                Visibility::default(),
                ChildOf(kitchen),
            ))
            .id();
        commands.entity(cook).with_children(|c| {
            c.spawn((
                Name::new(format!("{name} control")),
                CookControl(cook),
                Pressable::Cook,
                Grabbable {
                    points: vec![GrabPoint::free()],
                },
                Collider::cuboid(0.06, 0.06, 0.06),
                CollisionLayers::new(Layer::Item, LayerMask::NONE),
                Transform::from_xyz(0.0, 0.03, 0.22),
                Visibility::default(),
            ));
            c.spawn((
                crate::render::legibility::TextSource::KitchenLabel,
                ScreenText {
                    text: String::new(),
                    size: Vec2::new(0.26, 0.09),
                    // A label strip: a few words, big (paper is set for a
                    // page of lines, and came out 6 mm high).
                    style: ScreenStyle::Card,
                },
                Transform::from_xyz(0.0, 0.32, -0.25),
                Visibility::default(),
            ));
        });
    }
}

/// Marks the table top (for visuals).
#[derive(Component)]
pub struct KitchenTable;

/// The trays belong to whoever is playing (so drops land in them).
fn own_trays(
    mut trays: Query<&mut Backpack, With<CookTray>>,
    locals: Query<Entity, Added<crate::rig::LocalPlayer>>,
) {
    for p in &locals {
        for mut t in &mut trays {
            t.owner = p;
        }
    }
}

/// Which cooking food an item is.
pub fn food_of(catalog: &Catalog, item: &Item) -> Option<Food> {
    let def = catalog.get(item.kind());
    let tag = def.tags.iter().find_map(|t| t.strip_prefix("food:"))?;
    Food::ALL.iter().copied().find(|f| format!("{f:?}") == tag)
}

/// The item that stands for a food (prefer made ones over found ones).
pub fn item_for(catalog: &Catalog, food: Food) -> Option<extraction_core::items::ItemKind> {
    let tag = format!("food:{food:?}");
    let mut found = None;
    for (kind, def) in catalog.iter() {
        if def.has_tag(&tag) {
            if def.id.starts_with("cook_") {
                return Some(kind);
            }
            found.get_or_insert(kind);
        }
    }
    found
}

/// Once made food has left its station's tray, it can go back in.
fn forget_made(
    mut commands: Commands,
    made: Query<(Entity, &MadeAt, Option<&InBackpack>)>,
    cooks: Query<&Cook>,
) {
    for (e, m, bag) in &made {
        let home = cooks.get(m.0).map(|c| c.tray).ok();
        if bag.map(|b| b.bag) != home {
            commands.entity(e).remove::<MadeAt>();
        }
    }
}

/// Food dropped into an idle station goes into the station.
fn take_in_food(
    mut commands: Commands,
    mut cooks: Query<(Entity, &mut Cook)>,
    mut trays: Query<&mut Backpack, With<CookTray>>,
    items: Query<(&Item, Option<&MadeAt>)>,
    catalog: Res<Catalog>,
) {
    for (me, mut cook) in &mut cooks {
        let Ok(mut tray) = trays.get_mut(cook.tray) else {
            continue;
        };
        if !cook.station.is_idle() {
            continue;
        }
        let mut taken = Vec::new();
        for e in &tray.contents {
            let Ok((item, made)) = items.get(*e) else {
                continue;
            };
            // Things made here wait to be taken out.
            if made.is_some_and(|m| m.0 == me) {
                continue;
            }
            let Some(food) = food_of(&catalog, item) else {
                continue;
            };
            if cook.station.put(food).is_ok() {
                taken.push(*e);
            }
        }
        for e in &taken {
            commands.entity(*e).despawn();
        }
        tray.contents.retain(|e| !taken.contains(e));
    }
}

fn use_controls(
    mut events: MessageReader<InteractEvent>,
    controls: Query<&CookControl>,
    mut cooks: Query<&mut Cook>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok(control) = controls.get(*item) else {
            continue;
        };
        let Ok(mut cook) = cooks.get_mut(control.0) else {
            continue;
        };
        match cook.station.drive() {
            Drive::Effort => {
                let _ = cook.station.work();
            }
            _ => {
                if cook.station.is_idle() {
                    let _ = cook.station.start();
                } else if cook.station.ready().is_some() {
                    // Off the heat and out.
                    cook.take = true;
                } else {
                    // Turning the knob mid-way stops it.
                    cook.station.abort();
                }
            }
        }
    }
}

/// The clock, and finished food out into the tray.
fn cook(
    mut commands: Commands,
    mut cooks: Query<(Entity, &mut Cook)>,
    mut trays: Query<&mut Backpack, With<CookTray>>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut raid: ResMut<Raid>,
    mut cooking: MessageWriter<Cooking>,
) {
    let dt = time.delta_secs();
    for (me, mut cook) in &mut cooks {
        if cook.station.drive() != Drive::Effort {
            cook.carry += dt * TICKS_PER_SEC as f32;
            while cook.carry >= 1.0 {
                cook.carry -= 1.0;
                cook.station.tick();
            }
        }
        let name = cook.name;
        let process = cook.station.processes()[0];
        for ev in cook.station.drain() {
            cooking.write(Cooking {
                station: me,
                process,
                what: ev,
            });
            match ev {
                Event::Smoking => raid.say(format!("The {} is smoking.", name.to_lowercase())),
                Event::Burnt => raid.say(format!(
                    "Something in the {} has burnt.",
                    name.to_lowercase()
                )),
                _ => {}
            }
        }
        if cook.station.ready().is_none() {
            cook.take = false;
            continue;
        }
        // Worked stations hand over when done; the clock ones wait for the
        // knob, and burn if you leave them.
        if cook.station.drive() == Drive::Effort || std::mem::take(&mut cook.take) {
            let Ok(made) = cook.station.take() else {
                continue;
            };
            let Some(kind) = item_for(&catalog, made) else {
                continue;
            };
            let Ok(mut tray) = trays.get_mut(cook.tray) else {
                continue;
            };
            let e = spawn_item(
                &mut commands,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::IDENTITY,
                    condition: 1.0,
                },
            );
            tray.contents.push(e);
            commands.entity(e).insert((
                MadeAt(me),
                InBackpack { bag: cook.tray },
                ChildOf(cook.tray),
                Transform::from_xyz(0.0, 0.0, 0.0),
                RigidBodyDisabled,
                ColliderDisabled,
            ));
            raid.say(format!("{}: {}.", name, catalog.get(kind).name));
        }
    }
}

fn label(cooks: Query<(&Cook, &Children)>, mut screens: Query<&mut ScreenText>) {
    for (cook, children) in &cooks {
        let s = &cook.station;
        let contents: Vec<String> = s.contents().iter().map(|f| format!("{f:?}")).collect();
        let mut text = format!("{}: ", cook.name);
        if let Some((p, r)) = s.plan() {
            let toward = r.map_or("mush".to_string(), |r| format!("{:?}", r.output));
            text += &format!("{} -> {:?} makes {toward}", contents.join(", "), p);
            text += if s.drive() == Drive::Effort {
                " (work it)"
            } else {
                " (turn knob)"
            };
        } else if !s.is_idle() && s.ready().is_none() {
            text += &format!("{:.0}%", s.progress() * 100.0);
        } else {
            text += "empty";
        }
        for c in children.iter() {
            if let Ok(mut t) = screens.get_mut(c) {
                if t.text != text {
                    t.text = text.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(app: &mut App, name: &str) -> (Entity, Entity, Entity) {
        let (cook, tray) = app
            .world_mut()
            .query::<(Entity, &Cook)>()
            .iter(app.world())
            .find(|(_, c)| c.name == name)
            .map(|(e, c)| (e, c.tray))
            .unwrap();
        let control = app
            .world_mut()
            .query::<(Entity, &CookControl)>()
            .iter(app.world())
            .find(|(_, c)| c.0 == cook)
            .map(|(e, _)| e)
            .unwrap();
        (cook, tray, control)
    }

    fn drop_in(app: &mut App, tray: Entity, id: &str) {
        let catalog = app.world().resource::<Catalog>().clone();
        let kind = catalog.kind(id).unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let e = {
            let mut c = Commands::new(&mut queue, app.world());
            spawn_item(
                &mut c,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::IDENTITY,
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.world_mut()
            .get_mut::<Backpack>(tray)
            .unwrap()
            .contents
            .push(e);
        app.world_mut().entity_mut(e).insert((
            InBackpack { bag: tray },
            ChildOf(tray),
            RigidBodyDisabled,
            ColliderDisabled,
        ));
    }

    fn press(app: &mut App, control: Entity) {
        app.world_mut().write_message(InteractEvent::Pressed {
            item: control,
            player: Entity::PLACEHOLDER,
        });
        app.update();
    }

    fn tray_ids(app: &mut App, tray: Entity) -> Vec<String> {
        let catalog = app.world().resource::<Catalog>().clone();
        let contents = app.world().get::<Backpack>(tray).unwrap().contents.clone();
        contents
            .iter()
            .filter_map(|e| app.world().get::<Item>(*e))
            .map(|i| catalog.get(i.kind()).id.clone())
            .collect()
    }

    #[test]
    fn chop_a_carrot_and_brew_tea() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let (_, board, knife) = station(&mut app, "Board");
        drop_in(&mut app, board, "carrot");
        app.update();
        for _ in 0..40 {
            press(&mut app, knife);
            if tray_ids(&mut app, board)
                .iter()
                .any(|i| i == "cook_chopped_carrot")
            {
                break;
            }
        }
        assert_eq!(tray_ids(&mut app, board), vec!["cook_chopped_carrot"]);

        let (_, pot, knob) = station(&mut app, "Pot");
        let run_until_ready = |app: &mut App| {
            for _ in 0..90 * 120 {
                app.update();
                let ready = app
                    .world_mut()
                    .query::<&Cook>()
                    .iter(app.world())
                    .any(|c| c.name == "Pot" && c.station.ready().is_some());
                if ready {
                    return true;
                }
            }
            false
        };
        // Boil the water first...
        drop_in(&mut app, pot, "water_bottle");
        app.update();
        press(&mut app, knob);
        assert!(run_until_ready(&mut app), "the water boils");
        press(&mut app, knob);
        assert_eq!(tray_ids(&mut app, pot), vec!["cook_hot_water"]);
        // ...lift it out and back in with the leaves, and let it steep.
        let hot = app.world().get::<Backpack>(pot).unwrap().contents[0];
        app.world_mut().entity_mut(hot).remove::<MadeAt>();
        drop_in(&mut app, pot, "tea_tin");
        app.update();
        press(&mut app, knob);
        assert!(run_until_ready(&mut app), "the tea steeps");
        press(&mut app, knob);
        let got_tea = tray_ids(&mut app, pot).contains(&"cook_tea".to_string());
        assert!(got_tea, "tray: {:?}", tray_ids(&mut app, pot));
    }
}
