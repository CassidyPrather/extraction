//! The memorial convoy. When someone goes down, a collector walks out of
//! the Gate to fetch the body — the remains bag and everything in it — and
//! carry it out of the Zone. Whatever it delivers is gone for good, and the
//! truck's memorial board gets another row. Bring it down before it reaches
//! the Gate and the bag drops where it falls.
//!
//! The collector is a walker body (same legs, same navigation) with no gun
//! and a thicker shell, and a drone flies escort.

use bevy::prelude::*;

use super::director::{Director, gate};
use super::drone::spawn_drone;
use super::walker::spawn_walker_with;
use super::{Job, JobKind, Robot};
use crate::interact::body::Backpack;
use crate::items::{Catalog, Item};
use crate::persist::Memorial;
use crate::raid::{Raid, Remains};
use crate::world::Zone;

/// Seconds after someone goes down before the collection order goes out.
pub const ORDER_DELAY: f32 = 20.0;
/// Seconds the collector spends loading the body.
pub const LOAD_TIME: f32 = 8.0;
/// At most this many convoys are out at once (the rest wait their turn).
pub const MAX_CONVOYS: usize = 2;

/// On a collector: the remains it was sent for, and whether it has them.
#[derive(Component, Debug)]
pub struct Collector {
    pub remains: Entity,
    pub loaded: bool,
}

/// On remains: a collector has been sent (or is about to be).
#[derive(Component, Debug)]
pub struct CollectionOrder {
    pub dispatch_in: f32,
    pub sent: bool,
}

#[derive(Message, Clone, Copy, Debug)]
pub enum ConvoyEvent {
    /// Finished loading at the site.
    Loaded { robot: Entity },
    /// Walked out through the Gate.
    Delivered { robot: Entity },
}

pub struct CollectorPlugin;

impl Plugin for CollectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ConvoyEvent>().add_systems(
            FixedUpdate,
            (
                order_collections,
                dispatch_collectors,
                convoy_events,
                convoy_wrecked,
            )
                .chain()
                .after(super::perceive),
        );
    }
}

/// Every new body gets a collection order.
fn order_collections(
    mut commands: Commands,
    remains: Query<Entity, (With<Remains>, Without<CollectionOrder>)>,
) {
    for e in &remains {
        commands.entity(e).insert(CollectionOrder {
            dispatch_in: ORDER_DELAY,
            sent: false,
        });
    }
}

fn dispatch_collectors(
    mut commands: Commands,
    mut orders: Query<(Entity, &mut CollectionOrder, &GlobalTransform)>,
    director: Option<Res<Director>>,
    zone: Res<Zone>,
    time: Res<Time>,
    mut raid: ResMut<Raid>,
    robots: Query<&super::Robot, Without<super::indoor::LocalNav>>,
    collectors: Query<(), With<Collector>>,
) {
    let Some(director) = director.filter(|d| d.enabled) else {
        return;
    };
    let dt = time.delta_secs();
    // A convoy is two robots off the director's budget, and only so many
    // are out at once: quick deaths queue, they don't stack.
    let mut active = robots.iter().filter(|r| !r.wrecked).count();
    let mut out = collectors.iter().count();
    for (remains, mut order, gt) in &mut orders {
        if order.sent {
            continue;
        }
        order.dispatch_in -= dt;
        if order.dispatch_in > 0.0 {
            continue;
        }
        if active + 2 > director.max_active || out >= MAX_CONVOYS {
            continue;
        }
        active += 2;
        out += 1;
        order.sent = true;
        let exit = gate(&zone);
        let site = gt.translation();
        let job = Job {
            kind: JobKind::Collect,
            site,
            duration: LOAD_TIME,
            exit,
        };
        let robot = spawn_walker_with(&mut commands, &zone, exit, job, false, 260.0);
        commands.entity(robot).insert((
            Name::new("Collector"),
            Collector {
                remains,
                loaded: false,
            },
        ));
        spawn_drone(
            &mut commands,
            exit + Vec3::Y * 15.0,
            Job {
                kind: JobKind::Response,
                site,
                duration: 90.0,
                exit,
            },
        );
        info!("collector dispatched to remains at {site:.0}");
        raid.say("Radio: \"...collection order filed. Convoy leaving the Gate.\"");
    }
}

/// Loading and delivery.
fn convoy_events(
    mut commands: Commands,
    mut events: MessageReader<ConvoyEvent>,
    mut collectors: Query<&mut Collector>,
    bags: Query<&Backpack, With<Remains>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut memorial: ResMut<Memorial>,
    mut raid: ResMut<Raid>,
) {
    for ev in events.read() {
        match *ev {
            ConvoyEvent::Loaded { robot } => {
                let Ok(mut c) = collectors.get_mut(robot) else {
                    continue;
                };
                if bags.get(c.remains).is_err() {
                    // Someone (or the Surge) got there first.
                    continue;
                }
                c.loaded = true;
                // The bag rides slung under the body.
                commands
                    .entity(c.remains)
                    .insert((ChildOf(robot), Transform::from_xyz(0.0, -0.75, 0.0)));
                raid.say("Radio: \"...remains recovered. Convoy returning to the Gate.\"");
            }
            ConvoyEvent::Delivered { robot } => {
                let Ok(c) = collectors.get(robot) else {
                    continue;
                };
                commands.entity(robot).try_despawn();
                if !c.loaded {
                    continue;
                }
                let Ok(bag) = bags.get(c.remains) else {
                    continue;
                };
                let names: Vec<String> = bag
                    .contents
                    .iter()
                    .filter_map(|e| items.get(*e).ok())
                    .map(|i| catalog.get(i.kind()).name.clone())
                    .collect();
                if !names.is_empty() {
                    memorial.0.convoys += 1;
                    raid.say(format!(
                        "The convoy is through the Gate. {} things you carried are gone.",
                        names.len()
                    ));
                    memorial.0.lose(names);
                }
                // (Despawning the robot takes the bag, its child, with it.)
            }
        }
    }
}

/// A collector brought down with the bag aboard drops it.
fn convoy_wrecked(
    mut commands: Commands,
    collectors: Query<(Entity, &Collector, &Robot, &GlobalTransform)>,
    zone: Res<Zone>,
    mut memorial: ResMut<Memorial>,
    mut raid: ResMut<Raid>,
) {
    for (e, c, robot, gt) in &collectors {
        if !robot.wrecked {
            continue;
        }
        commands.entity(e).remove::<Collector>();
        if !c.loaded {
            continue;
        }
        let p = gt.translation();
        commands
            .entity(c.remains)
            .remove::<ChildOf>()
            .insert(Transform::from_translation(Vec3::new(
                p.x,
                zone.ground(p.x, p.z) + 0.25,
                p.z,
            )));
        memorial.0.recovered += 1;
        raid.say("The collector is down. Your kit is lying in the wreck.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{SpawnItem, spawn_item};

    /// A body a short walk from the Gate, with one thing in the bag.
    fn app_with_remains() -> (App, Entity) {
        // A full valley's ground (its patrol strength: `director`).
        let opts = crate::LaunchOptions {
            robots: true,
            map: extraction_core::map::TEST_FIELD_ID.into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        // Quiet the routine jobs so only the convoy walks.
        app.world_mut().resource_mut::<Director>().next_job = 1e9;
        let zone = app.world().resource::<Zone>().clone();
        let g = gate(&zone);
        let at = zone.on_ground(g.x + 18.0, g.z + 18.0) + Vec3::Y * 0.25;
        let bag = crate::persist::spawn_remains(app.world_mut(), at);
        let catalog = app.world().resource::<Catalog>().clone();
        let kind = catalog.kind("cleaning_kit").unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let item = {
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
            .get_mut::<Backpack>(bag)
            .unwrap()
            .contents
            .push(item);
        app.world_mut().entity_mut(item).insert((
            crate::interact::body::InBackpack { bag },
            ChildOf(bag),
            avian3d::prelude::RigidBodyDisabled,
            avian3d::prelude::ColliderDisabled,
        ));
        (app, bag)
    }

    /// Five deaths in quick succession don't send five convoys at once:
    /// no more than MAX_CONVOYS are out, the rest wait their turn.
    #[test]
    fn quick_deaths_queue_their_convoys() {
        let (mut app, _) = app_with_remains();
        let zone = app.world().resource::<Zone>().clone();
        let g = gate(&zone);
        for k in 0..4 {
            let at = zone.on_ground(g.x + 30.0 + k as f32 * 6.0, g.z + 20.0) + Vec3::Y * 0.25;
            crate::persist::spawn_remains(app.world_mut(), at);
        }
        let mut most = 0;
        let mut any = false;
        for _ in 0..90 * 40 {
            for mut o in app
                .world_mut()
                .query::<&mut CollectionOrder>()
                .iter_mut(app.world_mut())
            {
                o.dispatch_in = o.dispatch_in.min(0.0);
            }
            app.update();
            let out = app
                .world_mut()
                .query::<&Collector>()
                .iter(app.world())
                .count();
            most = most.max(out);
            any |= out > 0;
        }
        assert!(any, "a convoy went");
        assert!(most <= MAX_CONVOYS, "{most} convoys out at once");
    }

    #[test]
    fn convoy_takes_the_body_out() {
        let (mut app, bag) = app_with_remains();
        let mut loaded_at = None;
        for tick in 0..90 * 150 {
            app.update();
            let carried = app
                .world_mut()
                .query::<&Collector>()
                .iter(app.world())
                .any(|c| c.loaded);
            if carried && loaded_at.is_none() {
                loaded_at = Some(tick);
            }
            if app.world().resource::<Memorial>().0.convoys > 0 {
                break;
            }
        }
        assert!(loaded_at.is_some(), "the collector reached the body");
        let m = &app.world().resource::<Memorial>().0;
        assert_eq!(m.convoys, 1, "the convoy got out");
        assert_eq!(m.lost, vec!["Cleaning kit".to_string()]);
        assert!(
            app.world().get_entity(bag).is_err(),
            "the bag left the Zone"
        );
    }

    #[test]
    fn wrecking_the_collector_drops_the_bag() {
        let (mut app, bag) = app_with_remains();
        let mut wrecked = false;
        for _ in 0..90 * 120 {
            app.update();
            let loaded = app
                .world_mut()
                .query::<(Entity, &Collector)>()
                .iter(app.world())
                .find(|(_, c)| c.loaded)
                .map(|(e, _)| e);
            if let Some(robot) = loaded {
                app.world_mut().write_message(super::super::PartDestroyed {
                    robot,
                    part: robot,
                    role: super::super::PartRole::Core,
                    at: Vec3::ZERO,
                });
                wrecked = true;
            }
            if wrecked && app.world().get::<ChildOf>(bag).is_none() {
                break;
            }
        }
        assert!(wrecked, "the collector loaded the bag");
        app.update();
        assert!(app.world().get::<ChildOf>(bag).is_none(), "the bag dropped");
        let m = &app.world().resource::<Memorial>().0;
        assert_eq!(m.convoys, 0);
        assert_eq!(m.recovered, 1);
        assert_eq!(app.world().get::<Backpack>(bag).unwrap().contents.len(), 1);
    }
}
