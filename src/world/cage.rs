//! A cage in a shaft, for any site whose layout has one (`adit_rime`'s
//! winze first): it rides between the shaft's foot and the floor of the
//! room at its top, called and sent by bells. A bell rung, it waits
//! before it moves, its landings' lamps flashing (the tell); a landing
//! it's not at shows red over an open shaft; and it comes down on
//! whoever stands under it.
//!
//! Logic in [`CagePlugin`] (headless-safe); its look and the landings'
//! lamps in [`CageVisualsPlugin`].

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::underground as ug;

use super::Layer;
use super::buildings::Building;
use super::furnace::{lever_bundle, wall_pose};

/// How fast the cage rides (m/s).
pub const CAGE_SPEED: f32 = 1.5;
/// A bell rung, how long before the cage moves (s).
pub const BELL_WAIT: f32 = 2.0;
/// What the cage does to whoever it comes down on.
pub const CRUSH: f32 = 70.0;
/// How far a bell is heard (m).
pub const BELL_HEARD: f32 = 35.0;
/// Its walls' height (m).
const WALL: f32 = 2.3;

pub struct CagePlugin;

impl Plugin for CagePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CageRung>()
            .add_message::<CageArrived>()
            .add_message::<CageCrushed>()
            .init_resource::<crate::bot::Counters>()
            .add_systems(Update, (wire_cage, stage_scenario, count_rides))
            .add_systems(
                FixedUpdate,
                (ring_bells, run_cage)
                    .chain()
                    .after(crate::interact::InteractSystems::Grab)
                    .before(crate::player::move_bodies),
            );
    }
}

pub struct CageVisualsPlugin;

impl Plugin for CageVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (cage_looks, gate_lamps));
    }
}

/// The cage: its floor at `y` (site space) in its shaft, bound for `to`
/// once a bell's `wait` is out.
#[derive(Component, Debug)]
pub struct Cage {
    pub site: Entity,
    pub y: f32,
    /// Its stops: the shaft's foot, the top room's floor.
    pub bottom: f32,
    pub top: f32,
    /// The shaft's head (its rope's sheave), site space.
    pub head: f32,
    pub to: Option<f32>,
    pub wait: f32,
    /// The shaft's centre (site space x, z), and the floor's half size.
    pub at: Vec2,
    pub half: Vec2,
    /// Its landings are across the shaft along z (walled on its x sides).
    pub along_z: bool,
}

impl Cage {
    /// Standing at a stop (true the top), going nowhere.
    pub fn at_stop(&self) -> Option<bool> {
        if self.to.is_some() {
            None
        } else if (self.y - self.top).abs() < 0.01 {
            Some(true)
        } else if (self.y - self.bottom).abs() < 0.01 {
            Some(false)
        } else {
            None
        }
    }
}

/// A bell pull: at a landing (the top's or the foot's), or in the cage.
#[derive(Component, Debug)]
pub struct CageBell {
    pub cage: Entity,
    pub stop: Option<bool>,
}

/// A landing's lamp: green with the cage there, red over the open shaft,
/// flashing with the bell rung.
#[derive(Component, Debug)]
pub struct GateLamp {
    pub cage: Entity,
    pub top: bool,
}

/// A cage's bell rung.
#[derive(Message, Clone, Copy, Debug)]
pub struct CageRung {
    pub site: Entity,
    pub at: Vec3,
}

/// The cage at a stop, with whoever rode it.
#[derive(Message, Clone, Debug)]
pub struct CageArrived {
    pub site: Entity,
    pub top: bool,
    pub riders: Vec<Entity>,
}

/// The cage came down on someone.
#[derive(Message, Clone, Copy, Debug)]
pub struct CageCrushed {
    pub player: Entity,
    pub site: Entity,
}

/// The rope's drawn from the cage's roof to the shaft's head.
#[derive(Component)]
struct CageRope;

/// Logic, as sites are built: the cage (parked at the top) with its floor
/// and side walls, its bells, its landings' lamps.
fn wire_cage(
    mut commands: Commands,
    buildings: Query<(Entity, &Building, &Transform), Added<Building>>,
) {
    for (site, b, xf) in &buildings {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        let Some(c) = &l.cage else {
            continue;
        };
        let room = |id: &str| l.rooms.iter().find(|r| r.id == id);
        let (Some(s), Some(t)) = (room(&c.shaft), room(&c.top)) else {
            continue;
        };
        let at = Vec2::from(s.at);
        let half = Vec2::new(
            s.size[0] * 0.5 - ug::CAGE_GAP,
            s.size[1] * 0.5 - ug::CAGE_GAP,
        );
        // Open toward its top room and the far side (the foot's way out),
        // walled on the other two.
        let along_z = (s.at[0] - t.at[0]).abs() < (s.at[1] - t.at[1]).abs();
        let toward = if along_z {
            Vec2::new(0.0, (t.at[1] - s.at[1]).signum())
        } else {
            Vec2::new((t.at[0] - s.at[0]).signum(), 0.0)
        };
        let (bottom, top) = (l.floor_of(s), l.floor_of(t));
        let world = |p: Vec3| xf.transform_point(p);
        let solid = || CollisionLayers::new(Layer::World, LayerMask::ALL);
        let cage = commands
            .spawn((
                Name::new("Cage"),
                Cage {
                    site,
                    y: top,
                    bottom,
                    top,
                    head: bottom + s.height,
                    to: None,
                    wait: 0.0,
                    at,
                    half,
                    along_z,
                },
                RigidBody::Kinematic,
                Transform::from_translation(world(Vec3::new(at.x, top, at.y)))
                    .with_rotation(xf.rotation),
                Visibility::default(),
            ))
            .with_children(|k| {
                k.spawn((
                    Name::new("Cage floor"),
                    Collider::cuboid(half.x * 2.0, 0.1, half.y * 2.0),
                    solid(),
                    Transform::from_xyz(0.0, -0.05, 0.0),
                ));
                for side in [-1.0, 1.0] {
                    let (size, p) = if along_z {
                        (
                            Vec3::new(0.06, WALL, half.y * 2.0),
                            Vec3::new(side * (half.x - 0.03), WALL * 0.5, 0.0),
                        )
                    } else {
                        (
                            Vec3::new(half.x * 2.0, WALL, 0.06),
                            Vec3::new(0.0, WALL * 0.5, side * (half.y - 0.03)),
                        )
                    };
                    k.spawn((
                        Name::new("Cage side"),
                        Collider::cuboid(size.x, size.y, size.z),
                        solid(),
                        Transform::from_translation(p),
                    ));
                }
            })
            .id();
        // The bell in the cage, on a side wall.
        let side = if along_z {
            Vec3::new(half.x - 0.2, 1.3, 0.0)
        } else {
            Vec3::new(0.0, 1.3, half.y - 0.2)
        };
        let q = Quat::from_rotation_arc(Vec3::NEG_Z, side.with_y(0.0).normalize());
        commands.spawn((
            lever_bundle(
                "Cage bell",
                Transform::from_translation(side).with_rotation(q),
                cage,
            ),
            CageBell { cage, stop: None },
        ));
        for (r, p, rot) in &c.bells {
            let Some(pose) = wall_pose(l, r, *p, *rot, 1.2) else {
                continue;
            };
            commands.spawn((
                lever_bundle("Cage bell", pose, site),
                CageBell {
                    cage,
                    stop: Some(*r == c.top),
                },
            ));
        }
        // A lamp over each landing's edge of the shaft.
        let edge = at + toward * Vec2::new(s.size[0], s.size[1]) * 0.5;
        let aside = Vec2::new(toward.y, toward.x) * 0.6;
        for (top_lamp, y, p) in [
            (true, top, edge + aside),
            (false, bottom, at * 2.0 - edge + aside),
        ] {
            commands.spawn((
                Name::new("Cage gate lamp"),
                GateLamp {
                    cage,
                    top: top_lamp,
                },
                Transform::from_xyz(p.x, y + 2.2, p.y),
                Visibility::default(),
                ChildOf(site),
            ));
        }
    }
}

/// A bell pulled: rung (heard), and the cage called to that landing, or
/// sent from it (or, from inside, to the other stop), once the wait's out.
fn ring_bells(
    mut events: MessageReader<crate::interact::InteractEvent>,
    bells: Query<(&CageBell, &GlobalTransform)>,
    mut cages: Query<&mut Cage>,
    mut rung: MessageWriter<CageRung>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for ev in events.read() {
        let crate::interact::InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok((bell, gt)) = bells.get(*item) else {
            continue;
        };
        let Ok(mut c) = cages.get_mut(bell.cage) else {
            continue;
        };
        let at = gt.translation();
        rung.write(CageRung { site: c.site, at });
        noise.write(crate::robots::Noise {
            at,
            radius: BELL_HEARD,
            source: None,
        });
        if c.to.is_some() {
            continue;
        }
        let up = match (bell.stop, c.at_stop()) {
            (None, Some(top)) => !top,
            // Stopped short (on someone): back up.
            (None, None) => true,
            (Some(stop), Some(here)) if stop == here => !here,
            (Some(stop), _) => stop,
        };
        c.to = Some(if up { c.top } else { c.bottom });
        c.wait = BELL_WAIT;
        raid.say("Three bells, and somewhere the winding engine takes the strain.");
    }
}

/// The cage rides; whoever stands in it goes with it, and whoever stands
/// under it as it comes down stops it, hard.
#[allow(clippy::type_complexity)]
fn run_cage(
    mut cages: Query<(&mut Cage, &mut Transform)>,
    sites: Query<&Transform, (With<Building>, Without<Cage>)>,
    mut players: Query<
        (
            Entity,
            &mut Transform,
            &crate::player::Body,
            &mut crate::health::Health,
        ),
        (
            With<crate::player::Player>,
            Without<Cage>,
            Without<Building>,
        ),
    >,
    mut arrived: MessageWriter<CageArrived>,
    mut crushed: MessageWriter<CageCrushed>,
    mut raid: ResMut<crate::raid::Raid>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut c, mut t) in &mut cages {
        let Some(to) = c.to else {
            continue;
        };
        if c.wait > 0.0 {
            c.wait -= dt;
            continue;
        }
        let Ok(xf) = sites.get(c.site) else {
            continue;
        };
        let inv = xf.compute_affine().inverse();
        let step = (to - c.y).clamp(-CAGE_SPEED * dt, CAGE_SPEED * dt);
        let ny = c.y + step;
        // Who's on it, and who's under it, by their feet (site space).
        let mut riders = Vec::new();
        let mut under = None;
        for (e, pt, body, _) in &players {
            let feet = pt.translation
                + body
                    .feet
                    .map_or(Vec3::ZERO, |f| pt.rotation * Vec3::new(f.x, 0.0, f.y));
            let q = inv.transform_point3(feet);
            if (q.x - c.at.x).abs() > c.half.x + 0.1 || (q.z - c.at.y).abs() > c.half.y + 0.1 {
                continue;
            }
            if q.y > c.y - 0.25 && q.y < c.y + 0.6 {
                riders.push(e);
            } else if step < 0.0 && q.y < c.y - 0.25 && ny - 0.1 < q.y + body.height.max(0.9) {
                under = Some(e);
            }
        }
        if let Some(e) = under {
            c.to = None;
            if let Ok((_, _, _, mut h)) = players.get_mut(e) {
                h.hp -= CRUSH;
                h.since_hit = 0.0;
                h.bleeding = (h.bleeding + 1.5).min(3.0);
            }
            crushed.write(CageCrushed {
                player: e,
                site: c.site,
            });
            raid.say("The cage comes down on you, and the rope goes slack.");
            continue;
        }
        c.y = ny;
        t.translation = xf.transform_point(Vec3::new(c.at.x, c.y, c.at.y));
        for e in &riders {
            if let Ok((_, mut pt, _, _)) = players.get_mut(*e) {
                pt.translation.y += step;
            }
        }
        if (c.y - to).abs() < 1e-4 {
            c.y = to;
            c.to = None;
            arrived.write(CageArrived {
                site: c.site,
                top: to == c.top,
                riders,
            });
        }
    }
}

/// The scenario: on the adit's shaft station, the bot steps into the
/// cage and rings it down; at the foot it sends it up and calls it back,
/// rides it up, and steps out where it began, unhurt (`cage_rides`).
pub const SCENARIO: &str = "cage_ride";
/// Filled in by the setup: in the cage, out at its foot, out at its top.
pub const IN_CAGE: Vec2 = Vec2::new(-9001.0, 1.0);
pub const AT_FOOT: Vec2 = Vec2::new(-9001.0, 2.0);
pub const AT_TOP: Vec2 = Vec2::new(-9001.0, 3.0);

/// The scenario staged: the bot onto the shaft station, its walks and its
/// bells filled in (the bells in order: the cage's, the foot's twice, the
/// cage's).
fn stage_scenario(
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    cages: Query<(Entity, &Cage)>,
    sites: Query<(&Building, &Transform), Without<crate::rig::LocalPlayer>>,
    bells: Query<(Entity, &CageBell)>,
    mut players: Query<(&mut Transform, &mut crate::bot::Bot), With<crate::rig::LocalPlayer>>,
) {
    for s in setups.read() {
        if s.name != SCENARIO {
            continue;
        }
        let Some((cage, c)) = cages.iter().next() else {
            error!("cage scenario: no cage on this map");
            continue;
        };
        let Ok((b, xf)) = sites.get(c.site) else {
            continue;
        };
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        let Some(def) = &l.cage else {
            continue;
        };
        let room = |id: &str| l.rooms.iter().find(|r| r.id == id);
        // The foot's way out: the room linked to the shaft.
        let foot = l
            .links
            .iter()
            .find_map(|k| {
                (k.from == def.shaft)
                    .then_some(&k.to)
                    .or((k.to == def.shaft).then_some(&k.from))
            })
            .and_then(|id| room(id));
        let (Some(top), Some(foot)) = (room(&def.top), foot) else {
            continue;
        };
        let w = |at: [f32; 2], y: f32| xf.transform_point(Vec3::new(at[0], y, at[1]));
        let flat = |p: Vec3| Vec2::new(p.x, p.z);
        let in_cage = flat(w(c.at.into(), c.y));
        let at_top = w(top.at, l.floor_of(top) + 0.05);
        let at_foot = flat(w(foot.at, l.floor_of(foot)));
        let bell = |stop: Option<bool>| {
            bells
                .iter()
                .find(|(_, b)| b.cage == cage && b.stop == stop)
                .map(|(e, _)| e)
                .unwrap_or(Entity::PLACEHOLDER)
        };
        let mut presses =
            [bell(None), bell(Some(false)), bell(Some(false)), bell(None)].into_iter();
        for (mut t, mut bot) in &mut players {
            t.translation = at_top;
            for task in bot.tasks.iter_mut() {
                match task {
                    crate::bot::Task::WalkTo { to, .. }
                    | crate::bot::Task::Expect(crate::bot::Expect::Near(to, _)) => {
                        if *to == IN_CAGE {
                            *to = in_cage;
                        } else if *to == AT_FOOT {
                            *to = at_foot;
                        } else if *to == AT_TOP {
                            *to = flat(at_top);
                        }
                    }
                    crate::bot::Task::Press(e) if *e == Entity::PLACEHOLDER => {
                        *e = presses.next().unwrap_or(Entity::PLACEHOLDER);
                    }
                    _ => {}
                }
            }
        }
        info!("scenario: the cage at {in_cage:?}, its foot {at_foot:?}, its top {at_top:?}");
    }
}

/// Counts the rides with someone in the cage, and the crushings.
fn count_rides(
    mut arrived: MessageReader<CageArrived>,
    mut crushed: MessageReader<CageCrushed>,
    mut counters: ResMut<crate::bot::Counters>,
) {
    for a in arrived.read() {
        if !a.riders.is_empty() {
            counters.bump("cage_rides");
        }
    }
    for _ in crushed.read() {
        counters.bump("cage_crushed");
    }
}

/// Client: the cage drawn (its floor, mesh sides, bonnet, rope) and its
/// landings' lamps.
fn cage_looks(
    mut commands: Commands,
    cages: Query<(Entity, &Cage), Added<Cage>>,
    lamps: Query<Entity, Added<GateLamp>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let iron = || StandardMaterial {
        base_color: Color::srgb(0.17, 0.16, 0.15),
        perceptual_roughness: 0.75,
        metallic: 0.6,
        ..default()
    };
    for (e, c) in &cages {
        let iron = materials.add(iron());
        let mut bar = |size: Vec3, at: Vec3| {
            commands.spawn((
                crate::render::world_props::Blockout,
                Mesh3d(meshes.add(Cuboid::from_size(size))),
                MeshMaterial3d(iron.clone()),
                Transform::from_translation(at),
                ChildOf(e),
            ));
        };
        let (hx, hz) = (c.half.x, c.half.y);
        bar(Vec3::new(hx * 2.0, 0.06, hz * 2.0), Vec3::Y * -0.03);
        bar(Vec3::new(hx * 2.0, 0.05, hz * 2.0), Vec3::Y * (WALL + 0.1));
        // Corner posts, and the side mesh as bars.
        for (sx, sz) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
            bar(
                Vec3::new(0.06, WALL + 0.1, 0.06),
                Vec3::new(sx * (hx - 0.03), (WALL + 0.1) * 0.5, sz * (hz - 0.03)),
            );
        }
        for k in 1..8 {
            let f = k as f32 / 8.0 * 2.0 - 1.0;
            for side in [-1.0, 1.0] {
                let p = if c.along_z {
                    Vec3::new(side * (hx - 0.03), WALL * 0.5, f * hz)
                } else {
                    Vec3::new(f * hx, WALL * 0.5, side * (hz - 0.03))
                };
                bar(Vec3::new(0.03, WALL, 0.03), p);
            }
        }
        commands.spawn((
            CageRope,
            crate::render::world_props::Blockout,
            Mesh3d(meshes.add(Cylinder::new(0.025, 1.0))),
            MeshMaterial3d(iron.clone()),
            Transform::from_xyz(0.0, WALL + 0.6, 0.0),
            ChildOf(e),
        ));
    }
    for e in &lamps {
        commands.entity(e).insert((
            Mesh3d(meshes.add(Sphere::new(0.07))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: LinearRgba::rgb(4.0, 0.2, 0.1),
                ..default()
            })),
            PointLight {
                color: Color::srgb(1.0, 0.1, 0.05),
                intensity: 1500.0,
                range: 5.0,
                shadow_maps_enabled: false,
                ..default()
            },
        ));
    }
}

/// Client: the rope paid out to the cage; the landings' lamps green with
/// the cage there, red over the open shaft, flashing amber with a bell
/// rung.
#[allow(clippy::type_complexity)]
fn gate_lamps(
    cages: Query<(Entity, &Cage)>,
    children: Query<&Children>,
    mut ropes: Query<&mut Transform, With<CageRope>>,
    mut lamps: Query<(
        &GateLamp,
        &mut PointLight,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for (e, c) in &cages {
        let len = (c.head - c.y - WALL - 0.15).max(0.1);
        for k in children.iter_descendants(e) {
            if let Ok(mut t) = ropes.get_mut(k) {
                t.translation.y = WALL + 0.15 + len * 0.5;
                t.scale.y = len;
            }
        }
    }
    let blink = (time.elapsed_secs() * 4.0).fract() < 0.5;
    for (lamp, mut light, mat) in &mut lamps {
        let Ok((_, c)) = cages.get(lamp.cage) else {
            continue;
        };
        let rgb = if c.wait > 0.0 {
            if blink { [1.0, 0.5, 0.05] } else { [0.0; 3] }
        } else if c.at_stop() == Some(lamp.top) {
            [0.2, 1.0, 0.25]
        } else {
            [1.0, 0.1, 0.05]
        };
        light.color = Color::srgb(rgb[0], rgb[1], rgb[2]);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.emissive = LinearRgba::rgb(rgb[0] * 4.0, rgb[1] * 4.0, rgb[2] * 4.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::InteractEvent;
    use crate::vr_harness::Vr;

    const SITE: &str = "adit_rime";

    fn cage(vr: &mut Vr) -> (Entity, Transform) {
        let mut q = vr.app.world_mut().query::<(Entity, &Cage)>();
        let (e, c) = q.iter(vr.app.world()).next().expect("the adit's cage");
        let site = c.site;
        (e, *vr.app.world().get::<Transform>(site).unwrap())
    }

    fn state(vr: &Vr, e: Entity) -> (f32, Option<f32>, f32) {
        let c = vr.app.world().get::<Cage>(e).unwrap();
        (c.y, c.to, c.wait)
    }

    fn bell(vr: &mut Vr, cage: Entity, stop: Option<bool>) -> Entity {
        let mut q = vr.app.world_mut().query::<(Entity, &CageBell)>();
        q.iter(vr.app.world())
            .find(|(_, b)| b.cage == cage && b.stop == stop)
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("a bell for {stop:?}"))
    }

    fn ring(vr: &mut Vr, bell: Entity) {
        let player = vr.player;
        vr.app
            .world_mut()
            .write_message(InteractEvent::Pressed { item: bell, player });
    }

    fn hp(vr: &Vr) -> f32 {
        vr.app
            .world()
            .get::<crate::health::Health>(vr.player)
            .unwrap()
            .hp
    }

    /// In the cage at the shaft station, the bell rung: it waits (the
    /// tell), then rides down the winze with you in it, unhurt, to the deep
    /// level; the foot's bell sends it back up. Standing in the shaft's
    /// foot as it comes down, it comes down on you, and stops; its own
    /// bell takes it back up.
    #[test]
    fn riding_the_cage_and_standing_under_it() {
        let mut vr = Vr::new("rime_fell");
        let (e, xf) = cage(&mut vr);
        let l = ug::layout(SITE).unwrap();
        let (top, bottom, at) = {
            let c = vr.app.world().get::<Cage>(e).unwrap();
            (c.top, c.bottom, c.at)
        };
        assert!(top - bottom > 10.0, "a deep winze: {top} {bottom}");
        assert_eq!(
            vr.app.world().get::<Cage>(e).unwrap().at_stop(),
            Some(true),
            "parked at the top"
        );
        let world = |y: f32, dz: f32| xf.transform_point(Vec3::new(at.x, y, at.y + dz));

        // In, and the bell in the cage.
        vr.stand(world(top + 0.02, 0.0), 0.0);
        vr.step(30);
        let full = hp(&vr);
        let inside = bell(&mut vr, e, None);
        ring(&mut vr, inside);
        vr.step(90);
        let (y, to, wait) = state(&vr, e);
        assert_eq!(to, Some(bottom), "bound down");
        assert!(
            wait > 0.0 && (y - top).abs() < 1e-3,
            "it waits first: {y} {wait}"
        );
        let mut arrived = false;
        for _ in 0..90 * 15 {
            vr.step(1);
            if state(&vr, e).1.is_none() {
                arrived = true;
                break;
            }
        }
        assert!(arrived, "the cage got down: {:?}", state(&vr, e));
        vr.step(30);
        let feet = vr.feet();
        assert!(
            (feet.y - world(bottom, 0.0).y).abs() < 0.3,
            "rode it down: feet {feet:?}, foot {:?}",
            world(bottom, 0.0)
        );
        assert!(hp(&vr) > full - 1.0, "unhurt: {} of {full}", hp(&vr));

        // Out at the foot (toward the level station), and the foot's bell
        // sends it back up.
        let station = l.rooms.iter().find(|r| r.id == "level_station").unwrap();
        let out = xf.transform_point(Vec3::new(station.at[0], bottom + 0.05, station.at[1]));
        vr.stand(out, 0.0);
        let foot = bell(&mut vr, e, Some(false));
        ring(&mut vr, foot);
        vr.step(90 * 13);
        assert_eq!(
            vr.app.world().get::<Cage>(e).unwrap().at_stop(),
            Some(true),
            "sent back up"
        );

        // Under it, in the shaft's foot, when it's sent down from the top.
        vr.stand(world(bottom + 0.05, 0.0), 0.0);
        vr.step(30);
        let before = hp(&vr);
        let station_bell = bell(&mut vr, e, Some(true));
        ring(&mut vr, station_bell);
        vr.step(90 * 13);
        let (y, to, _) = state(&vr, e);
        assert!(to.is_none() && y > bottom + 0.8, "it stopped on you: {y}");
        assert!(
            hp(&vr) <= before - CRUSH + 1.0,
            "and it hurt: {} from {before}",
            hp(&vr)
        );
        ring(&mut vr, inside);
        vr.step(90 * 13);
        assert_eq!(
            vr.app.world().get::<Cage>(e).unwrap().at_stop(),
            Some(true),
            "its own bell takes it back up"
        );
    }

    /// Nothing walks into the winze by accident: a raider's routes stop
    /// at its edge, top and foot, and the crawlers' grid has it shut.
    #[test]
    fn nothing_walks_into_the_winze() {
        use crate::sim::indoor::{BuildingPlan, plan_buildings};
        use bevy::ecs::system::RunSystemOnce;
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: true,
                map: "rime_fell".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        for _ in 0..10 {
            app.update();
        }
        let plan: BuildingPlan = app
            .world_mut()
            .run_system_once(
                |b: Query<(Entity, &Building, &Transform)>,
                 d: Query<(Entity, &super::super::buildings::DoorLeaf, &ChildOf)>| {
                    plan_buildings(&b, &d)
                },
            )
            .unwrap()
            .into_iter()
            .find(|p| p.name == SITE)
            .unwrap();
        let l = ug::layout(SITE).unwrap();
        let room = |id: &str| l.rooms.iter().find(|r| r.id == id).unwrap();
        let at = |id: &str| {
            let r = room(id);
            plan.xf
                .transform_point(Vec3::new(r.at[0], l.floor_of(r) + 0.1, r.at[1]))
        };
        let winze = Vec2::from(room("winze").at);
        let drift = Vec2::from(room("drift").at);
        assert!(
            crate::sim::raider::indoor_route(&plan, at("plat"), drift, 0.5).is_some(),
            "the shaft station's on the way"
        );
        for from in ["plat", "level_station"] {
            assert!(
                crate::sim::raider::indoor_route(&plan, at(from), winze, 0.3).is_none(),
                "a raider walked from {from} into the winze"
            );
        }
        let grids = app
            .world()
            .resource::<crate::world::underground::ResidentGrids>();
        let sites = &grids
            .0
            .values()
            .find(|g| g.id == SITE)
            .expect("the adit's residents")
            .sites;
        let indoor = app.world().resource::<crate::robots::indoor::IndoorSites>();
        let w = room("winze");
        for &k in sites {
            let grid = &indoor.sites[k].nav.grid;
            let mut seen = 0;
            for j in 0..grid.n {
                for i in 0..grid.n {
                    let c = grid.centre(i, j);
                    if (c[0] - w.at[0]).abs() < w.size[0] * 0.5 - 0.2
                        && (c[1] - w.at[1]).abs() < w.size[1] * 0.5 - 0.2
                    {
                        seen += 1;
                        assert_eq!(
                            grid.cost[j * grid.n + i],
                            extraction_core::nav::BLOCKED,
                            "the winze open to crawlers at {c:?}"
                        );
                    }
                }
            }
            assert!(seen > 0, "the crawlers' grid covers the winze");
        }
    }
}
