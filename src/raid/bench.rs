//! The bench in the truck bed: drop things in its tray, turn the knob to
//! pick from what the tray can make, pull the lever. Recipes and their
//! losses are in [`extraction_core::fabrication`].

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::fabrication::{Make, TrayItem, available, plan, recipe};

use super::Raid;
use super::truck::{FLOOR, Pressable, Truck};
use crate::interact::body::{Backpack, InBackpack};
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::weapons::Gun;
use crate::world::Layer;

/// The bench's state.
#[derive(Component, Debug, Default)]
pub struct Bench {
    /// Recipe ids the tray can make, in recipe-book order.
    pub options: Vec<&'static str>,
    pub selected: usize,
    /// (recipe id, seconds left).
    pub running: Option<(&'static str, f32)>,
    pub tray: Option<Entity>,
    /// What the last run made, for the display.
    pub last: Option<String>,
}

/// The bench's input/output tray (an open container).
#[derive(Component)]
pub struct Tray;

/// Text for an in-world display; the client renders it onto a panel
/// (`render::screen`).
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct ScreenText {
    pub text: String,
    /// Panel size in metres.
    pub size: Vec2,
    pub style: ScreenStyle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenStyle {
    /// A backlit amber LCD.
    #[default]
    Lcd,
    /// A sheet of paper, pencilled.
    Paper,
    /// A card with a few words in big letters.
    Card,
    /// A green-phosphor computer screen.
    Crt,
}

/// The bench in truck space.
pub const BENCH_AT: Vec3 = Vec3::new(0.72, FLOOR + 0.4, 0.15);
pub const BENCH_SIZE: Vec3 = Vec3::new(0.7, 0.8, 1.5);
const TRAY_HALF: Vec3 = Vec3::new(0.26, 0.08, 0.42);

pub struct BenchPlugin;

impl Plugin for BenchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (build_bench, press_bench, run_bench, bench_display).chain(),
        );
    }
}

fn build_bench(mut commands: Commands, trucks: Query<Entity, Added<Truck>>) {
    for truck in &trucks {
        let bench = commands
            .spawn((
                Name::new("Bench"),
                Transform::from_translation(BENCH_AT),
                Visibility::default(),
                ChildOf(truck),
            ))
            .id();
        let top = BENCH_SIZE.y * 0.5;
        let tray = commands
            .spawn((
                Name::new("Bench tray"),
                Tray,
                Backpack {
                    owner: Entity::PLACEHOLDER,
                    open: true,
                    model: extraction_core::inventory::Backpack {
                        capacity: 30.0,
                        mass: 0.0,
                    },
                    contents: Vec::new(),
                    carried: false,
                    half: TRAY_HALF,
                },
                Transform::from_xyz(0.0, top + TRAY_HALF.y, 0.0),
                Visibility::default(),
                ChildOf(bench),
            ))
            .id();
        commands.entity(bench).insert(Bench {
            tray: Some(tray),
            ..default()
        });
        commands.entity(bench).with_children(|c| {
            c.spawn((
                Collider::cuboid(BENCH_SIZE.x, 0.06, BENCH_SIZE.z),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                Transform::from_xyz(0.0, top - 0.03, 0.0),
            ));
            for (p, at) in [
                (
                    Pressable::BenchSelect,
                    Vec3::new(-BENCH_SIZE.x * 0.5 + 0.02, top - 0.08, -0.55),
                ),
                (
                    Pressable::BenchRun,
                    Vec3::new(-BENCH_SIZE.x * 0.5 + 0.02, top - 0.08, 0.6),
                ),
            ] {
                c.spawn((
                    Name::new(format!("{p:?}")),
                    p,
                    Grabbable {
                        points: vec![GrabPoint::free()],
                    },
                    Collider::cuboid(0.08, 0.08, 0.08),
                    CollisionLayers::new(Layer::Item, LayerMask::NONE),
                    Transform::from_translation(at),
                    Visibility::default(),
                ));
            }
            // The display, on the canopy wall above the bench, facing in.
            c.spawn((
                Name::new("Bench display"),
                ScreenText {
                    text: String::new(),
                    size: Vec2::new(0.62, 0.36),
                    style: ScreenStyle::Lcd,
                },
                Transform::from_xyz(0.36, 1.05, 0.0)
                    .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
                Visibility::default(),
            ));
        });
    }
}

fn press_bench(
    mut events: MessageReader<InteractEvent>,
    pressables: Query<(&Pressable, &ChildOf)>,
    mut benches: Query<&mut Bench>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok((p, parent)) = pressables.get(*item) else {
            continue;
        };
        let Ok(mut bench) = benches.get_mut(parent.parent()) else {
            continue;
        };
        match p {
            Pressable::BenchSelect if bench.running.is_none() && !bench.options.is_empty() => {
                bench.selected = (bench.selected + 1) % bench.options.len();
            }
            Pressable::BenchRun if bench.running.is_none() => {
                if let Some(id) = bench.options.get(bench.selected).copied() {
                    let secs = recipe(id).map_or(5.0, |r| r.seconds);
                    bench.running = Some((id, secs));
                }
            }
            _ => {}
        }
    }
}

/// What's in a tray, as the recipe matcher sees it (guns by their
/// mechanism's condition).
fn tray_items(
    bag: &Backpack,
    items: &Query<(&mut Item, Option<&mut Gun>)>,
) -> (Vec<Entity>, Vec<TrayItem>) {
    let mut es = Vec::new();
    let mut ts = Vec::new();
    for e in &bag.contents {
        if let Ok((it, gun)) = items.get(*e) {
            es.push(*e);
            ts.push(TrayItem {
                kind: it.kind(),
                condition: gun.map_or(it.condition, |g| g.firearm.condition),
            });
        }
    }
    (es, ts)
}

/// Keeps the option list current, and finishes runs.
pub fn run_bench(
    mut commands: Commands,
    mut benches: Query<&mut Bench>,
    mut bags: Query<&mut Backpack, With<Tray>>,
    mut items: Query<(&mut Item, Option<&mut Gun>)>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut raid: ResMut<Raid>,
) {
    let dt = time.delta_secs();
    for mut bench in &mut benches {
        let Some(tray_e) = bench.tray else {
            continue;
        };
        let Ok(mut bag) = bags.get_mut(tray_e) else {
            continue;
        };
        let (entities, tray) = tray_items(&bag, &items);
        let options: Vec<&'static str> = available(&tray, &catalog).iter().map(|r| r.id).collect();
        if options != bench.options {
            let keep = bench.options.get(bench.selected).copied();
            bench.options = options;
            bench.selected = keep
                .and_then(|k| bench.options.iter().position(|o| *o == k))
                .unwrap_or(0);
        }
        let Some((id, left)) = bench.running else {
            continue;
        };
        let left = left - dt;
        if left > 0.0 {
            bench.running = Some((id, left));
            continue;
        }
        bench.running = None;
        let Some(r) = recipe(id) else {
            continue;
        };
        // Re-check: someone may have taken something out mid-run.
        let Some(picks) = plan(r, &tray, &catalog) else {
            bench.last = Some("Something's missing.".into());
            continue;
        };
        let used: Vec<Entity> = picks.iter().map(|&i| entities[i]).collect();
        match r.makes {
            Make::Items(out) => {
                for e in &used {
                    commands.entity(*e).despawn();
                }
                bag.contents.retain(|e| !used.contains(e));
                let mut made = Vec::new();
                for (k, (oid, n)) in out.iter().enumerate() {
                    let Some(kind) = catalog.kind(oid) else {
                        continue;
                    };
                    for j in 0..*n {
                        let e = spawn_item(
                            &mut commands,
                            &catalog,
                            SpawnItem {
                                kind,
                                transform: Transform::IDENTITY,
                                condition: 1.0,
                            },
                        );
                        let slot = (k * 3 + j as usize) as f32;
                        let local = Vec3::new(
                            ((slot * 0.37) % 1.0 - 0.5) * TRAY_HALF.x * 1.6,
                            0.0,
                            ((slot * 0.61) % 1.0 - 0.5) * TRAY_HALF.z * 1.6,
                        );
                        bag.contents.push(e);
                        commands.entity(e).insert((
                            InBackpack { bag: tray_e },
                            ChildOf(tray_e),
                            Transform::from_translation(local),
                            RigidBodyDisabled,
                            ColliderDisabled,
                        ));
                    }
                    made.push(format!("{} x{n}", catalog.get(kind).name));
                }
                bench.last = Some(format!("Made: {}", made.join(", ")));
            }
            Make::Repair(amount) => {
                // The gun stays; the parts are used up.
                let mut gun_e = None;
                for e in &used {
                    if items.get(*e).is_ok_and(|(_, g)| g.is_some()) {
                        gun_e = Some(*e);
                    } else {
                        commands.entity(*e).despawn();
                    }
                }
                bag.contents
                    .retain(|e| Some(*e) == gun_e || !used.contains(e));
                if let Some(Ok((mut item, Some(mut gun)))) = gun_e.map(|g| items.get_mut(g)) {
                    gun.firearm.condition = (gun.firearm.condition + amount).min(1.0);
                    gun.firearm.fouling = 0.0;
                    item.condition = gun.firearm.condition;
                    bench.last = Some(format!("Repaired to {:.0}%", gun.firearm.condition * 100.0));
                }
            }
        }
        raid.say(format!("Bench: {}", bench.last.clone().unwrap_or_default()));
    }
}

fn bench_display(benches: Query<(&Bench, &Children)>, mut screens: Query<&mut ScreenText>) {
    for (bench, children) in &benches {
        let mut lines = vec!["WORKBENCH".to_string(), String::new()];
        if let Some((id, left)) = bench.running {
            let name = recipe(id).map_or(id, |r| r.name);
            lines.push(format!("{name}..."));
            lines.push(format!("{left:.0} s"));
        } else if bench.options.is_empty() {
            lines.push("Tray: nothing to work with.".into());
            lines.push("Parts to strip, powder and brass".into());
            lines.push("to pack, a worn gun and parts.".into());
        } else {
            for (i, id) in bench.options.iter().enumerate().take(6) {
                let name = recipe(id).map_or(*id, |r| r.name);
                let mark = if i == bench.selected { ">" } else { " " };
                lines.push(format!("{mark} {name}"));
            }
            lines.push(String::new());
            lines.push("Knob: next   Lever: run".into());
        }
        if let Some(l) = &bench.last {
            lines.push(String::new());
            lines.push(l.clone());
        }
        let text = lines.join("\n");
        for c in children.iter() {
            if let Ok(mut s) = screens.get_mut(c) {
                if s.text != text {
                    s.text = text.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bench_strips_and_packs() {
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
        let (bench_e, tray) = app
            .world_mut()
            .query::<(Entity, &Bench)>()
            .iter(app.world())
            .map(|(e, b)| (e, b.tray.unwrap()))
            .next()
            .expect("the truck has a bench");
        let catalog = app.world().resource::<Catalog>().clone();
        let put = |app: &mut App, id: &str| {
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
        };
        for id in [
            "robot_ammo_belt",
            "robot_ammo_belt",
            "rotor_assembly",
            "servo",
            "servo",
        ] {
            put(&mut app, id);
        }
        app.update();
        let names = |app: &mut App| -> Vec<String> {
            let bag = app.world().get::<Backpack>(tray).unwrap().contents.clone();
            bag.iter()
                .filter_map(|e| app.world().get::<Item>(*e))
                .map(|i| catalog.get(i.kind()).id.clone())
                .collect()
        };
        let run = |app: &mut App, id: &str| {
            let mut b = app.world_mut().get_mut::<Bench>(bench_e).unwrap();
            let i = b
                .options
                .iter()
                .position(|o| *o == id)
                .unwrap_or_else(|| panic!("{id} in {:?}", b.options));
            b.selected = i;
            b.running = Some((recipe(id).unwrap().id, 0.0));
            drop(b);
            app.update();
            app.update();
        };
        run(&mut app, "pull_belt");
        run(&mut app, "pull_belt");
        run(&mut app, "strip_servo");
        run(&mut app, "strip_servo");
        let have = names(&mut app);
        assert_eq!(
            have.iter().filter(|n| *n == "powder").count(),
            2,
            "{have:?}"
        );
        assert_eq!(have.iter().filter(|n| *n == "brass").count(), 2, "{have:?}");
        assert_eq!(
            have.iter().filter(|n| *n == "scrap_metal").count(),
            2,
            "{have:?}"
        );
        run(&mut app, "pack_545");
        let have = names(&mut app);
        assert!(have.contains(&"ammo_545".to_string()), "{have:?}");
        assert!(
            have.contains(&"rotor_assembly".to_string()),
            "untouched: {have:?}"
        );
        assert!(!have.contains(&"powder".to_string()), "{have:?}");
    }
}
