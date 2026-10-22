//! A shelter's lockdown, for any site whose layout has one
//! (`bunker_marrow` first): its programme is still set, and once its
//! power's on a klaxon sounds in the hall, an amber lamp turning (the
//! tell), and after a while its gas lock's doors shut and bolt for good.
//! The klaxon brings the crawlers; the way out is then the escape tunnel
//! (or the drain). The layout's check makes sure every room still reaches
//! it.
//!
//! Logic in [`LockdownPlugin`] (headless-safe); the klaxon's lamp in
//! [`LockdownVisualsPlugin`].

use bevy::prelude::*;
use extraction_core::underground as ug;

use super::buildings::{Building, Door, DoorLeaf};
use super::underground::{PowerLocked, SEALED, SitePower};

/// How often the klaxon sounds, and how far it carries (s, m).
pub const KLAXON_EVERY: f32 = 2.5;
pub const KLAXON_HEARD: f32 = 50.0;

pub struct LockdownPlugin;

impl Plugin for LockdownPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Klaxon>()
            .add_message::<Sealed>()
            .init_resource::<crate::bot::Counters>()
            .add_systems(
                Update,
                (wire_lockdown, run_lockdown, stage_scenario, count_seals).chain(),
            );
    }
}

pub struct LockdownVisualsPlugin;

impl Plugin for LockdownVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, turn_the_lamp);
    }
}

/// On a site with a lockdown: counting down (`left`) once its power's
/// on; then sealed.
#[derive(Component, Debug, Default)]
pub struct Lockdown {
    pub left: Option<f32>,
    pub sealed: bool,
    blast: f32,
}

/// The klaxon's turning lamp, in its room.
#[derive(Component, Debug)]
pub struct KlaxonLamp {
    pub site: Entity,
}

/// The klaxon sounding (each blast), at its room.
#[derive(Message, Clone, Copy, Debug)]
pub struct Klaxon {
    pub site: Entity,
    pub at: Vec3,
}

/// The site's sealed.
#[derive(Message, Clone, Copy, Debug)]
pub struct Sealed {
    pub site: Entity,
}

/// The door leaves (building space hinges) a site's lockdown shuts.
pub fn sealed_hinges(id: &str) -> Vec<Vec3> {
    let (Some(l), Some(r), Some(g)) = (ug::layout(id), ug::raster(id), super::underground::geo(id))
    else {
        return Vec::new();
    };
    let Some(k) = &l.lockdown else {
        return Vec::new();
    };
    // The geometry's doors: each kiosk's, then the raster's in order.
    let first = r.portals.len();
    r.doors
        .iter()
        .enumerate()
        .filter(|(_, d)| k.holds(l, d))
        .filter_map(|(i, _)| g.doors.get(first + i).map(|d| d.hinge))
        .collect()
}

fn wire_lockdown(mut commands: Commands, buildings: Query<(Entity, &Building), Added<Building>>) {
    for (site, b) in &buildings {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        let Some(k) = &l.lockdown else {
            continue;
        };
        commands.entity(site).insert(Lockdown::default());
        if let Some(room) = l.rooms.iter().find(|r| r.id == k.room) {
            commands.spawn((
                Name::new("Klaxon"),
                KlaxonLamp { site },
                Transform::from_xyz(
                    room.at[0],
                    l.floor_of(room) + room.height - 0.25,
                    room.at[1],
                ),
                Visibility::default(),
                ChildOf(site),
            ));
        }
    }
}

/// The power on: the klaxon, and the countdown; out, the doors shut and
/// bolted.
#[allow(clippy::too_many_arguments)]
fn run_lockdown(
    mut commands: Commands,
    mut sites: Query<(Entity, &mut Lockdown, &SitePower, &Building, &Transform)>,
    mut doors: Query<(Entity, &mut Door, &DoorLeaf, &ChildOf)>,
    lamps: Query<(&KlaxonLamp, &GlobalTransform)>,
    mut klaxon: MessageWriter<Klaxon>,
    mut sealed: MessageWriter<Sealed>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut raid: ResMut<crate::raid::Raid>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (site, mut lk, power, b, xf) in &mut sites {
        if lk.sealed {
            continue;
        }
        let Some(k) = ug::layout(&b.prefab).and_then(|l| l.lockdown.as_ref()) else {
            continue;
        };
        let Some(left) = lk.left else {
            // (Screenshots of it, `EXTRACTION_LOCKDOWN=1`: sounding from
            // the start, and never sealing.)
            let shot = std::env::var_os("EXTRACTION_LOCKDOWN").is_some();
            if shot || power.stages.iter().any(|s| s.on) {
                lk.left = Some(k.after);
                lk.blast = 0.0;
                raid.say(format!(
                    "A klaxon, and a voice off a tape: \"Attack warning. Shelter sealing in {:.0} seconds.\"",
                    k.after
                ));
            }
            continue;
        };
        let at = lamps
            .iter()
            .find(|(l, _)| l.site == site)
            .map_or(xf.translation, |(_, g)| g.translation());
        lk.blast -= dt;
        if lk.blast <= 0.0 {
            lk.blast = KLAXON_EVERY;
            klaxon.write(Klaxon { site, at });
            noise.write(crate::robots::Noise {
                at,
                radius: KLAXON_HEARD,
                source: None,
            });
        }
        let left = if std::env::var_os("EXTRACTION_LOCKDOWN").is_some() {
            k.after
        } else {
            left - dt
        };
        if left > 0.0 {
            lk.left = Some(left);
            continue;
        }
        lk.left = None;
        lk.sealed = true;
        let hinges = sealed_hinges(&b.prefab);
        for (d, mut door, leaf, parent) in &mut doors {
            if parent.parent() == site && hinges.iter().any(|h| leaf.0.hinge.distance(*h) < 1e-3) {
                door.open = false;
                commands.entity(d).insert(PowerLocked {
                    site,
                    stage: SEALED,
                });
            }
        }
        sealed.write(Sealed { site });
        raid.say("The gas lock's doors swing to and the bolts shoot home. Sealed: out by the escape tunnel.");
    }
}

/// The scenario: in the shelter's generator room, the bot starts it, waits
/// out the klaxon, and goes out by the escape tunnel (`shelter_sealed`).
pub const SCENARIO: &str = "shelter_lockdown";
/// Filled in by the setup: the way out to the escape tunnel (a walk
/// there stands for the route, doors and all).
pub const OUT: Vec2 = Vec2::new(-9003.0, 0.0);

/// The scenario staged: the bot into the generator room; its switch, and
/// its way out to the escape tunnel (as a raider would go), filled in.
#[allow(clippy::type_complexity)]
fn stage_scenario(
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    buildings: Query<(Entity, &Building, &Transform)>,
    leaves: Query<(Entity, &DoorLeaf, &ChildOf)>,
    power: Query<&SitePower>,
    mut players: Query<
        (&mut Transform, &mut crate::bot::Bot),
        (With<crate::rig::LocalPlayer>, Without<Building>),
    >,
) {
    for s in setups.read() {
        if s.name != SCENARIO {
            continue;
        }
        let Some((site, l, xf)) = buildings.iter().find_map(|(e, b, xf)| {
            ug::layout(&b.prefab)
                .filter(|l| l.lockdown.is_some())
                .map(|l| (e, l, *xf))
        }) else {
            error!("lockdown scenario: no shelter on this map");
            continue;
        };
        let Ok(switch) = power.get(site).map(|p| p.stages[0].switch) else {
            continue;
        };
        let Some(plan) = crate::sim::indoor::plan_buildings(&buildings, &leaves)
            .into_iter()
            .find(|p| p.entity == site)
        else {
            continue;
        };
        let room = |id: &str| l.rooms.iter().find(|r| r.id == id);
        let (Some(engine), Some(out)) =
            (room("generator"), room(&l.lockdown.as_ref().unwrap().out))
        else {
            continue;
        };
        let start = xf.transform_point(Vec3::new(
            engine.at[0],
            l.floor_of(engine) + 0.05,
            engine.at[1],
        ));
        let Some(route) = crate::sim::raider::indoor_route(&plan, start, out.at.into(), 1.0) else {
            error!("lockdown scenario: no way out to `{}`", out.id);
            continue;
        };
        let end = xf.transform_point(Vec3::new(out.at[0], l.floor_of(out), out.at[1]));
        for (mut t, mut bot) in &mut players {
            t.translation = start;
            let tasks: Vec<crate::bot::Task> = bot.tasks.drain(..).collect();
            for task in tasks {
                match task {
                    crate::bot::Task::WalkTo { to, .. } if to == OUT => {
                        bot.tasks.extend(route.iter().cloned());
                    }
                    crate::bot::Task::Expect(crate::bot::Expect::Near(to, r)) if to == OUT => {
                        bot.tasks
                            .push_back(crate::bot::Task::Expect(crate::bot::Expect::Near(
                                Vec2::new(end.x, end.z),
                                r,
                            )));
                    }
                    crate::bot::Task::Press(e) if e == Entity::PLACEHOLDER => {
                        bot.tasks.push_back(crate::bot::Task::Press(switch));
                    }
                    other => bot.tasks.push_back(other),
                }
            }
        }
        info!("scenario: the shelter's way out, {} steps", route.len());
    }
}

fn count_seals(mut sealed: MessageReader<Sealed>, mut counters: ResMut<crate::bot::Counters>) {
    for _ in sealed.read() {
        counters.bump("shelter_sealed");
    }
}

/// Client: the klaxon's lamp, amber, turning while it sounds; dark
/// otherwise.
fn turn_the_lamp(
    mut commands: Commands,
    mut lamps: Query<(Entity, &KlaxonLamp, &mut Transform, Option<&mut SpotLight>)>,
    sites: Query<&Lockdown>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for (e, lamp, mut t, light) in &mut lamps {
        let on = sites.get(lamp.site).is_ok_and(|l| l.left.is_some());
        let Some(mut light) = light else {
            commands.entity(e).insert((
                SpotLight {
                    color: Color::srgb(1.0, 0.55, 0.1),
                    intensity: 0.0,
                    range: 14.0,
                    outer_angle: 0.5,
                    inner_angle: 0.3,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Mesh3d(meshes.add(Cylinder::new(0.09, 0.14))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.6, 0.35, 0.05),
                    emissive: LinearRgba::rgb(2.0, 0.9, 0.1),
                    ..default()
                })),
            ));
            continue;
        };
        light.intensity = if on { 60_000.0 } else { 0.0 };
        if on {
            let yaw = time.elapsed_secs() * 4.0;
            t.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-0.5);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::InteractEvent;
    use crate::vr_harness::Vr;

    const SITE: &str = "bunker_marrow";

    /// The generator started: the klaxon sounds (heard far), and the
    /// countdown runs; the gas lock's doors stand till it's out, then shut
    /// and bolt, and a hand on one doesn't open it. The escape tunnel is
    /// still reached from the hall, through no sealed door, and the dorm's
    /// door still opens.
    #[test]
    fn the_shelter_seals_and_the_tunnel_stays_open() {
        use crate::sim::indoor::{BuildingPlan, plan_buildings};
        use bevy::ecs::system::RunSystemOnce;
        let mut vr = Vr::new("marrow_creek");
        #[derive(Resource, Default)]
        struct Heard(Vec<crate::robots::Noise>);
        vr.app.init_resource::<Heard>().add_systems(
            Update,
            |mut n: MessageReader<crate::robots::Noise>, mut h: ResMut<Heard>| {
                h.0.extend(n.read().cloned())
            },
        );
        let player = vr.player;
        let (site, switch) = {
            let mut q = vr
                .app
                .world_mut()
                .query::<(Entity, &Building, &SitePower)>();
            q.iter(vr.app.world())
                .find(|(_, b, _)| b.prefab == SITE)
                .map(|(e, _, p)| (e, p.stages[0].switch))
                .expect("the shelter, powered")
        };
        let after = ug::layout(SITE).unwrap().lockdown.as_ref().unwrap().after;
        let hinges = sealed_hinges(SITE);
        assert_eq!(hinges.len(), 2, "the gas lock's two doors");
        let gas_lock = |vr: &mut Vr| -> Vec<Entity> {
            let mut q = vr.app.world_mut().query::<(Entity, &DoorLeaf, &ChildOf)>();
            q.iter(vr.app.world())
                .filter(|(_, l, p)| {
                    p.parent() == site && hinges.iter().any(|h| l.0.hinge.distance(*h) < 1e-3)
                })
                .map(|(e, _, _)| e)
                .collect()
        };
        let doors = gas_lock(&mut vr);
        assert_eq!(doors.len(), 2);
        // Open them both, as you'd leave them coming in.
        for d in &doors {
            vr.app.world_mut().get_mut::<Door>(*d).unwrap().open = true;
        }
        assert!(vr.app.world().get::<Lockdown>(site).unwrap().left.is_none());

        vr.app.world_mut().write_message(InteractEvent::Pressed {
            item: switch,
            player,
        });
        vr.step(90 * 2);
        let lk = vr.app.world().get::<Lockdown>(site).unwrap();
        assert!(lk.left.is_some() && !lk.sealed, "counting down: {lk:?}");
        assert!(
            vr.app
                .world()
                .resource::<Heard>()
                .0
                .iter()
                .any(|n| n.radius >= KLAXON_HEARD),
            "the klaxon carries"
        );
        for d in &doors {
            assert!(vr.app.world().get::<Door>(*d).unwrap().open, "not yet");
            assert!(vr.app.world().get::<PowerLocked>(*d).is_none());
        }
        vr.step((90.0 * (after - 2.0 + 1.0)) as usize);
        assert!(
            vr.app.world().get::<Lockdown>(site).unwrap().sealed,
            "sealed"
        );
        for d in &doors {
            assert!(!vr.app.world().get::<Door>(*d).unwrap().open, "shut");
            assert_eq!(
                vr.app.world().get::<PowerLocked>(*d).map(|l| l.stage),
                Some(SEALED),
                "bolted"
            );
            vr.app
                .world_mut()
                .write_message(InteractEvent::Pressed { item: *d, player });
            vr.step(3);
            assert!(
                !vr.app.world().get::<Door>(*d).unwrap().open,
                "and stays so"
            );
        }

        // Out by the escape tunnel: a raider's route there from the hall
        // goes through no sealed door.
        let plan: BuildingPlan = vr
            .app
            .world_mut()
            .run_system_once(
                |b: Query<(Entity, &Building, &Transform)>,
                 d: Query<(Entity, &DoorLeaf, &ChildOf)>| plan_buildings(&b, &d),
            )
            .unwrap()
            .into_iter()
            .find(|p| p.name == SITE)
            .unwrap();
        let l = ug::layout(SITE).unwrap();
        let at = |id: &str| {
            let r = l.rooms.iter().find(|r| r.id == id).unwrap();
            plan.xf
                .transform_point(Vec3::new(r.at[0], l.floor_of(r) + 0.1, r.at[1]))
        };
        let escape = l.rooms.iter().find(|r| r.id == "escape").unwrap();
        let route = crate::sim::raider::indoor_route(&plan, at("hall"), escape.at.into(), 1.0)
            .expect("a way to the escape tunnel");
        assert!(
            !route
                .iter()
                .any(|t| matches!(t, crate::bot::Task::OpenDoor(d) if doors.contains(d))),
            "not through the gas lock"
        );
    }
}
