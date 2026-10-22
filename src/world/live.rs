//! Live water, for any site whose layout has some (`tunnel_cinder`'s
//! overflow chamber first): its lamps hang on their cables down into its
//! flood, and once their power's on the water is live. The tell is the
//! arcing over it and its hum (and you're told, at its door); wade in and
//! it throws you. The isolator elsewhere cuts the room's circuit. Crawlers
//! keep out of it (`underground::block_works`).
//!
//! Logic in [`LivePlugin`] (headless-safe); the arcing in
//! [`LiveVisualsPlugin`].

use bevy::prelude::*;
use extraction_core::underground as ug;
use std::collections::HashSet;

use super::buildings::Building;
use super::furnace::{lever_bundle, wall_pose};
use super::underground::SitePower;

/// What a jolt does (hp), and how often one comes standing in it (s).
pub const SHOCK: f32 = 22.0;
pub const SHOCK_EVERY: f32 = 0.5;
/// How near its water you're told it's live (m).
pub const TOLD_AT: f32 = 3.0;

pub struct LivePlugin;

impl Plugin for LivePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Live>()
            .add_message::<Shocked>()
            .add_message::<Isolated>()
            .init_resource::<crate::bot::Counters>()
            .add_systems(
                Update,
                (
                    wire_live,
                    pull_isolators,
                    energise,
                    shock,
                    stage_scenario,
                    count_jolts,
                )
                    .chain(),
            );
    }
}

pub struct LiveVisualsPlugin;

impl Plugin for LiveVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, arc);
    }
}

/// The sites whose live water is live now.
#[derive(Resource, Default, Debug)]
pub struct Live(pub HashSet<Entity>);

/// A site's isolator: `cut`, its room's circuit is off.
#[derive(Component, Debug)]
pub struct Isolator {
    pub site: Entity,
    pub cut: bool,
}

/// Someone took a jolt.
#[derive(Message, Clone, Copy, Debug)]
pub struct Shocked {
    pub player: Entity,
    pub at: Vec3,
}

/// An isolator pulled (`cut`) or put back.
#[derive(Message, Clone, Copy, Debug)]
pub struct Isolated {
    pub site: Entity,
    pub cut: bool,
    /// The isolator, where it clunks.
    pub at: Vec3,
}

/// The arcing over a live room's water (client: its light).
#[derive(Component, Debug)]
pub struct Arcing {
    pub site: Entity,
}

fn wire_live(mut commands: Commands, buildings: Query<(Entity, &Building), Added<Building>>) {
    for (site, b) in &buildings {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        let Some(v) = &l.live else {
            continue;
        };
        let (room, at, rot) = &v.isolator;
        if let Some(pose) = wall_pose(l, room, *at, *rot, 1.2) {
            commands.spawn((
                lever_bundle("Isolator", pose, site),
                Isolator { site, cut: false },
            ));
        }
        if let Some(r) = l.rooms.iter().find(|r| r.id == v.room) {
            commands.spawn((
                Name::new("Arcing"),
                Arcing { site },
                Transform::from_xyz(r.at[0], l.floor_of(r) + 0.9, r.at[1]),
                Visibility::default(),
                ChildOf(site),
            ));
        }
    }
}

/// An isolator pulled: its room's circuit off (or back on), a clunk.
fn pull_isolators(
    mut events: MessageReader<crate::interact::InteractEvent>,
    mut isolators: Query<(&mut Isolator, &GlobalTransform)>,
    mut out: MessageWriter<Isolated>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for ev in events.read() {
        let crate::interact::InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok((mut iso, gt)) = isolators.get_mut(*item) else {
            continue;
        };
        iso.cut = !iso.cut;
        out.write(Isolated {
            site: iso.site,
            cut: iso.cut,
            at: gt.translation(),
        });
        noise.write(crate::robots::Noise {
            at: gt.translation(),
            radius: 15.0,
            source: None,
        });
        raid.say(if iso.cut {
            "The isolator comes down with a clunk. Somewhere past the wall, a hum stops."
        } else {
            "The isolator goes back up. Somewhere past the wall, a hum starts."
        });
    }
}

/// Which sites' water is live: its lamps' power on, its isolator not cut.
fn energise(
    sites: Query<(Entity, &Building, &SitePower)>,
    isolators: Query<&Isolator>,
    mut live: ResMut<Live>,
) {
    for (site, b, power) in &sites {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        let Some(v) = &l.live else {
            continue;
        };
        let on = v.stage(l).is_some_and(|k| power.on(k));
        let cut = isolators.iter().any(|i| i.site == site && i.cut);
        // (Screenshots of it, `EXTRACTION_LIVE=1`: live from the start.)
        let shot = std::env::var_os("EXTRACTION_LIVE").is_some();
        let now = (on || shot) && !cut;
        if now != live.0.contains(&site) {
            if now {
                live.0.insert(site);
            } else {
                live.0.remove(&site);
            }
        }
    }
}

/// Standing in live water throws you, a jolt every half second; near it,
/// you're told once.
#[allow(clippy::type_complexity)]
fn shock(
    live: Res<Live>,
    sites: Query<(Entity, &Building, &Transform)>,
    mut bodies: Query<(
        Entity,
        &GlobalTransform,
        &crate::player::Body,
        &mut crate::health::Health,
    )>,
    mut shocked: MessageWriter<Shocked>,
    mut raid: ResMut<crate::raid::Raid>,
    time: Res<Time>,
    mut next: Local<f32>,
    mut told: Local<HashSet<Entity>>,
) {
    told.retain(|e| bodies.contains(*e));
    if live.0.is_empty() {
        return;
    }
    *next -= time.delta_secs();
    let jolt = *next <= 0.0;
    if jolt {
        *next = SHOCK_EVERY;
    }
    for (site, b, xf) in &sites {
        if !live.0.contains(&site) {
            continue;
        }
        let (Some(l), Some(r)) = (ug::layout(&b.prefab), ug::raster(&b.prefab)) else {
            continue;
        };
        let Some(room) = l
            .live
            .as_ref()
            .and_then(|v| l.rooms.iter().find(|r| r.id == v.room))
        else {
            continue;
        };
        let inv = xf.compute_affine().inverse();
        for (e, gt, body, mut h) in &mut bodies {
            let q = inv.transform_point3(gt.translation());
            let (dx, dz) = (
                (q.x - room.at[0]).abs() - room.size[0] * 0.5,
                (q.z - room.at[1]).abs() - room.size[1] * 0.5,
            );
            let near = dx.max(dz) < TOLD_AT && (q.y - l.floor_of(room)).abs() < 2.0;
            if near && told.insert(e) {
                raid.say("The water ahead hums, and blue light snaps over it: it's live.");
            }
            let inside = dx < 0.0
                && dz < 0.0
                && r.cell_of([q.x, q.z])
                    .is_some_and(|(a, c)| r.water[c * r.nx + a] > 0.05);
            if !inside || body.wading < 0.05 || !jolt || h.down {
                continue;
            }
            h.hp -= SHOCK;
            h.since_hit = 0.0;
            shocked.write(Shocked {
                player: e,
                at: gt.translation(),
            });
        }
    }
}

/// The scenario: in the service tunnel's cable vault, the bot throws its
/// breaker (the overflow chamber's door opens, its water live), goes to
/// the switch room's isolator, pulls it, and wades into the chamber
/// unhurt (`live_isolated`, no `live_jolts`).
pub const SCENARIO: &str = "live_water";
/// Filled in by the setup: the way to the isolator, and into the chamber
/// (walks there stand for the routes, doors and all).
pub const TO_ISOLATOR: Vec2 = Vec2::new(-9004.0, 0.0);
pub const INTO_IT: Vec2 = Vec2::new(-9004.0, 1.0);

/// The scenario staged: the bot into the cable vault; its breaker, its
/// isolator and its two routes filled in.
#[allow(clippy::type_complexity)]
fn stage_scenario(
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    buildings: Query<(Entity, &Building, &Transform)>,
    leaves: Query<(Entity, &super::buildings::DoorLeaf, &ChildOf)>,
    power: Query<&SitePower>,
    isolators: Query<(Entity, &Isolator)>,
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
                .filter(|l| l.live.is_some())
                .map(|l| (e, l, *xf))
        }) else {
            error!("live water scenario: none on this map");
            continue;
        };
        let (Ok(p), Some((iso, _))) = (
            power.get(site),
            isolators.iter().find(|(_, i)| i.site == site),
        ) else {
            continue;
        };
        let v = l.live.as_ref().unwrap();
        let Some(stage) = v.stage(l) else {
            continue;
        };
        let breaker = p.stages[stage].switch;
        let room = |id: &str| l.rooms.iter().find(|r| r.id == id);
        let (Some(vault), Some(switches), Some(live)) = (
            l.power.get(stage).and_then(|p| room(&p.room)),
            room(&v.isolator.0),
            room(&v.room),
        ) else {
            continue;
        };
        let Some(plan) = crate::sim::indoor::plan_buildings(&buildings, &leaves)
            .into_iter()
            .find(|p| p.entity == site)
        else {
            continue;
        };
        let at =
            |r: &ug::Room| xf.transform_point(Vec3::new(r.at[0], l.floor_of(r) + 0.05, r.at[1]));
        let start = at(vault);
        let iso_at = Vec2::from(switches.at) + Vec2::from(v.isolator.1) * 0.5;
        let (Some(first), Some(second)) = (
            crate::sim::raider::indoor_route(&plan, start, iso_at, 0.8),
            crate::sim::raider::indoor_route(&plan, at(switches), live.at.into(), 1.0),
        ) else {
            error!("live water scenario: no way round");
            continue;
        };
        for (mut t, mut bot) in &mut players {
            t.translation = start;
            let mut presses = [breaker, iso].into_iter();
            let tasks: Vec<crate::bot::Task> = bot.tasks.drain(..).collect();
            for task in tasks {
                match task {
                    crate::bot::Task::WalkTo { to, .. } if to == TO_ISOLATOR => {
                        bot.tasks.extend(first.iter().cloned());
                    }
                    crate::bot::Task::WalkTo { to, .. } if to == INTO_IT => {
                        bot.tasks.extend(second.iter().cloned());
                    }
                    crate::bot::Task::Press(e) if e == Entity::PLACEHOLDER => {
                        let e = presses.next().unwrap_or(Entity::PLACEHOLDER);
                        bot.tasks.push_back(crate::bot::Task::Press(e));
                    }
                    other => bot.tasks.push_back(other),
                }
            }
        }
        info!(
            "scenario: live water, {} and {} steps",
            first.len(),
            second.len()
        );
    }
}

/// Counts the isolators pulled and the jolts taken.
fn count_jolts(
    mut shocked: MessageReader<Shocked>,
    mut isolated: MessageReader<Isolated>,
    mut counters: ResMut<crate::bot::Counters>,
) {
    for _ in shocked.read() {
        counters.bump("live_jolts");
    }
    for i in isolated.read() {
        if i.cut {
            counters.bump("live_isolated");
        }
    }
}

/// Client: the arcing over live water, a cold blue light snapping on and
/// off.
fn arc(
    mut commands: Commands,
    mut arcs: Query<(Entity, &Arcing, Option<&mut PointLight>)>,
    live: Res<Live>,
    time: Res<Time>,
) {
    let t = time.elapsed_secs();
    for (e, a, light) in &mut arcs {
        // (A light only while it's live: dead, it costs nothing.)
        let on = live.0.contains(&a.site);
        let mut light = match (on, light) {
            (true, Some(l)) => l,
            (true, None) => {
                commands.entity(e).insert(PointLight {
                    color: Color::srgb(0.55, 0.75, 1.0),
                    intensity: 0.0,
                    range: 7.0,
                    shadow_maps_enabled: false,
                    ..default()
                });
                continue;
            }
            (false, Some(_)) => {
                commands.entity(e).remove::<PointLight>();
                continue;
            }
            (false, None) => continue,
        };
        // A spark now and then, never steady.
        let snap = ((t * 23.0).sin() * (t * 7.3).sin()).abs() > 0.82
            || std::env::var_os("EXTRACTION_LIVE").is_some();
        light.intensity = if snap { 40_000.0 } else { 0.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::InteractEvent;

    const SITE: &str = "tunnel_cinder";

    fn site(app: &mut App) -> (Entity, Transform, Entity, Entity) {
        let (e, xf, switch) = {
            let mut q = app
                .world_mut()
                .query::<(Entity, &Building, &Transform, &SitePower)>();
            q.iter(app.world())
                .find(|(_, b, _, _)| b.prefab == SITE)
                .map(|(e, _, t, p)| (e, *t, p.stages[0].switch))
                .expect("the service tunnel, powered")
        };
        let iso = app
            .world_mut()
            .query::<(Entity, &Isolator)>()
            .iter(app.world())
            .find(|(_, i)| i.site == e)
            .map(|(i, _)| i)
            .expect("its isolator");
        (e, xf, switch, iso)
    }

    /// The overflow chamber's water is dead till the vault's breaker is in;
    /// then it's live (told at its door, and a jolt standing in it), till
    /// the switch room's isolator is pulled.
    #[test]
    fn the_overflow_is_live_till_isolated() {
        let mut vr = crate::vr_harness::Vr::new("cinder_works");
        let player = vr.player;
        let (site, xf, switch, iso) = site(&mut vr.app);
        let l = ug::layout(SITE).unwrap();
        let room = l.rooms.iter().find(|r| r.id == "overflow").unwrap();
        let wet = xf.transform_point(Vec3::new(
            room.at[0] - 1.0,
            l.floor_of(room) + 0.02,
            room.at[1] + 1.0,
        ));
        let hp = |vr: &crate::vr_harness::Vr| {
            vr.app
                .world()
                .get::<crate::health::Health>(player)
                .unwrap()
                .hp
        };
        let reset = |vr: &mut crate::vr_harness::Vr| {
            let mut h = vr
                .app
                .world_mut()
                .get_mut::<crate::health::Health>(player)
                .unwrap();
            h.hp = h.max;
            h.bleeding = 0.0;
            h.down = false;
        };

        // Dead water: wade it unhurt.
        vr.stand(wet, 0.0);
        vr.step(90 * 2);
        let wading = vr
            .app
            .world()
            .get::<crate::player::Body>(player)
            .unwrap()
            .wading;
        assert!(wading > 0.3, "it's flooded: {wading}");
        assert!(!vr.app.world().resource::<Live>().0.contains(&site));
        let full = hp(&vr);
        vr.step(90);
        assert!(hp(&vr) >= full - 0.5, "dead water doesn't hurt");

        // The breaker in: live. A jolt, and another.
        vr.app.world_mut().write_message(InteractEvent::Pressed {
            item: switch,
            player,
        });
        vr.step(3);
        assert!(vr.app.world().resource::<Live>().0.contains(&site), "live");
        reset(&mut vr);
        let before = hp(&vr);
        vr.step(90);
        assert!(
            hp(&vr) <= before - SHOCK * 1.5,
            "thrown: {} from {before}",
            hp(&vr)
        );

        // Isolated: dead again.
        vr.app
            .world_mut()
            .write_message(InteractEvent::Pressed { item: iso, player });
        vr.step(3);
        assert!(
            !vr.app.world().resource::<Live>().0.contains(&site),
            "isolated"
        );
        reset(&mut vr);
        let before = hp(&vr);
        vr.step(90);
        assert!(hp(&vr) >= before - 0.5, "unhurt, isolated");
    }

    /// The crawlers keep out of it live, and walk it isolated.
    #[test]
    fn crawlers_keep_out_of_live_water() {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: true,
                map: "cinder_works".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        for _ in 0..10 {
            app.update();
        }
        let (_, _, switch, iso) = site(&mut app);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let l = ug::layout(SITE).unwrap();
        let room = l.rooms.iter().find(|r| r.id == "overflow").unwrap();
        let shut = |app: &App| -> usize {
            let grids = app
                .world()
                .resource::<super::super::underground::ResidentGrids>();
            let sites = &grids
                .0
                .values()
                .find(|g| g.id == SITE)
                .expect("its residents")
                .sites;
            let indoor = app.world().resource::<crate::robots::indoor::IndoorSites>();
            sites
                .iter()
                .map(|&k| {
                    let g = &indoor.sites[k].nav.grid;
                    (0..g.n * g.n)
                        .filter(|c| {
                            let p = g.centre(c % g.n, c / g.n);
                            (p[0] - room.at[0]).abs() < room.size[0] * 0.5 - 0.4
                                && (p[1] - room.at[1]).abs() < room.size[1] * 0.5 - 0.4
                                && g.cost[*c] == extraction_core::nav::BLOCKED
                        })
                        .count()
                })
                .sum()
        };
        let before = shut(&app);
        let press = |app: &mut App, item: Entity| {
            app.world_mut()
                .write_message(InteractEvent::Pressed { item, player });
            for _ in 0..60 {
                app.update();
            }
        };
        press(&mut app, switch);
        let live = shut(&app);
        assert!(live > before + 10, "shut live: {before} -> {live}");
        press(&mut app, iso);
        assert_eq!(shut(&app), before, "open again, isolated");
    }
}
