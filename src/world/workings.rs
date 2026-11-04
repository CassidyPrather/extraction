//! The deep workings' mechanics, for any site whose layout has them
//! (`deep_kettle` first): firedamp that a spark sets off, the canary that
//! smells it, the mine cart on its rail, shoring that brings the roof
//! down when it's shot out, and things left lying about (`items`).
//!
//! Logic in [`WorkingsPlugin`] (headless-safe); the look of the canary's
//! glow, the shoring and the fallen roof in [`WorkingsVisualsPlugin`].

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::underground as ug;
use std::collections::HashMap;

use super::Layer;
use super::buildings::Building;

/// Breathing firedamp: hit points a second.
pub const GAS_HARM: f32 = 2.0;
/// A pocket going up: the blast's harm to everything in it.
pub const GAS_BLAST: f32 = 65.0;
/// How far the bang's heard.
pub const GAS_HEARD: f32 = 120.0;
/// How near gas a canary first notices it (m).
pub const CANARY_SENSE: f32 = 6.0;
/// How long a canary lasts in the gas itself (s).
pub const CANARY_LIFE: f32 = 30.0;
/// A pushed cart's speed, and how fast it slows (m/s, m/s²).
pub const CART_PUSH: f32 = 2.2;
pub const CART_DRAG: f32 = 0.3;
/// Hits a timber set takes before it gives.
pub const SHORING_HP: u32 = 2;

pub struct WorkingsPlugin;

impl Plugin for WorkingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Gas>()
            .add_message::<GasIgnited>()
            .add_message::<RoofFell>()
            .add_message::<GasFlash>()
            .add_systems(
                Startup,
                place_site_items
                    .in_set(crate::items::loot::LootSpawnSystems)
                    .after(crate::world::buildings::spawn_buildings),
            )
            .add_systems(
                Update,
                (
                    wire_workings,
                    breathe,
                    ignite,
                    canaries,
                    push_carts,
                    shoot_shoring,
                ),
            )
            .add_systems(FixedUpdate, roll_carts.after(crate::player::move_bodies));
    }
}

pub struct WorkingsVisualsPlugin;

impl Plugin for WorkingsVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                canary_glow,
                canary_lamp,
                timber_looks,
                fall_looks,
                cart_looks,
                gas_flash,
            ),
        );
    }
}

/// Every site's gas pockets: live (still there) or gone (gone up), and
/// sample points through each (site space, a metre over the floor) for
/// what senses it.
#[derive(Resource, Default, Debug)]
pub struct Gas {
    pub live: HashMap<Entity, Vec<bool>>,
    pub points: HashMap<Entity, Vec<(usize, Vec3)>>,
}

/// A pocket's gone up.
#[derive(Message, Clone, Copy, Debug)]
pub struct GasIgnited {
    pub site: Entity,
    pub pocket: usize,
    pub at: Vec3,
}

/// A roof's come down where its shoring was shot out.
#[derive(Message, Clone, Copy, Debug)]
pub struct RoofFell {
    pub site: Entity,
    pub at: Vec3,
}

/// A body breathing firedamp now (for the sound of it and the blur).
#[derive(Component, Debug)]
pub struct InGas;

/// A canary cage's state: how near the gas is (0 nothing, 1 in it) and
/// how much life it has left (0: dead, and dark for good).
#[derive(Component, Debug, Clone, Copy)]
pub struct Canary {
    pub alarm: f32,
    pub life: f32,
}

/// A mine cart: on `rail` of `site`, `s` metres along it, rolling at `v`.
#[derive(Component, Debug)]
pub struct MineCart {
    pub site: Entity,
    pub rail: usize,
    pub s: f32,
    pub v: f32,
    /// Its world position last tick (riders go with it).
    last: Vec3,
}

/// A timber set; `hp` hits left in it.
#[derive(Component, Debug)]
pub struct Shoring {
    pub site: Entity,
    pub hp: u32,
    /// Across the corridor: its width.
    pub width: f32,
}

/// Where a roof came down: rubble filling the corridor.
#[derive(Component, Debug)]
pub struct Fall {
    pub width: f32,
}

/// The cart's bed (what you stand in): half extents, and its top.
const BED: Vec3 = Vec3::new(0.42, 0.36, 0.68);

/// The gas pocket a world point's in, if its site's raster says so and
/// it's between that cell's floor and roof.
fn pocket_at(b: &Building, xf: &Transform, p: Vec3) -> Option<usize> {
    let r = ug::raster(&b.prefab)?;
    let l = xf.compute_affine().inverse().transform_point3(p);
    let (i, j) = r.cell_of([l.x, l.z])?;
    let c = j * r.nx + i;
    let g = r.gas[c]?;
    let f = r.floors[c];
    (l.y > f - 0.3 && l.y < f + r.ceil[c].max(2.0) + 0.3).then_some(g as usize)
}

/// The layouts' own items (the canary on its bench, Brabec's boots), with
/// the Zone's loot: once for a fresh Zone, not again over a restored one
/// (the save has them where they were left).
fn place_site_items(
    mut commands: Commands,
    buildings: Query<(&Building, &Transform)>,
    catalog: Res<crate::items::Catalog>,
    restore: Option<Res<crate::persist::Restore>>,
) {
    if restore.is_some_and(|r| r.zone) {
        return;
    }
    for (b, xf) in &buildings {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        for it in &l.items {
            let (Some(room), Some(kind)) = (
                l.rooms.iter().find(|room| room.id == it.room),
                catalog.kind(&it.id),
            ) else {
                continue;
            };
            let at = Vec3::new(
                room.at[0] + it.at[0],
                l.floor_of(room) + it.y + 0.1,
                room.at[1] + it.at[1],
            );
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(xf.transform_point(at)),
                    condition: 1.0,
                },
            );
        }
    }
}

/// Logic, as sites are built: their pockets live, their carts on their
/// rails, their shoring standing.
fn wire_workings(
    mut commands: Commands,
    buildings: Query<(Entity, &Building), Added<Building>>,
    mut gas: ResMut<Gas>,
) {
    for (site, b) in &buildings {
        let (Some(l), Some(r)) = (ug::layout(&b.prefab), ug::raster(&b.prefab)) else {
            continue;
        };
        if !l.gas.is_empty() {
            gas.live.insert(site, vec![true; l.gas.len()]);
            let mut pts = Vec::new();
            for j in (0..r.nz).step_by(2) {
                for i in (0..r.nx).step_by(2) {
                    let c = j * r.nx + i;
                    if let Some(g) = r.gas[c] {
                        let p = r.centre(i, j);
                        pts.push((g as usize, Vec3::new(p[0], r.floors[c] + 1.0, p[1])));
                    }
                }
            }
            gas.points.insert(site, pts);
        }
        for (k, c) in l.carts.iter().enumerate() {
            let Some(rail) = l.rails.get(c.rail) else {
                continue;
            };
            let p = ug::along(rail, c.at);
            let floor = r.floor_at(p).unwrap_or(l.floor);
            commands.spawn((
                Name::new(format!("Mine cart {k}")),
                MineCart {
                    site,
                    rail: c.rail,
                    s: c.at,
                    v: 0.0,
                    last: Vec3::ZERO,
                },
                crate::raid::truck::Pressable::BenchRun,
                crate::interact::Grabbable {
                    points: vec![crate::interact::GrabPoint::free()],
                },
                RigidBody::Kinematic,
                cart_collider(),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                Transform::from_xyz(p[0], floor, p[1])
                    .with_rotation(Quat::from_rotation_y(rail_yaw(rail, c.at))),
                Visibility::default(),
                ChildOf(site),
            ));
        }
        for sh in &l.shoring {
            let Some(pts) = ug::link_points(l, &sh.link.0, &sh.link.1) else {
                continue;
            };
            let s = ug::length(&pts) * sh.along;
            let p = ug::along(&pts, s);
            let width = l
                .links
                .iter()
                .find(|k| {
                    (k.from == sh.link.0 && k.to == sh.link.1)
                        || (k.from == sh.link.1 && k.to == sh.link.0)
                })
                .map_or(1.5, |k| k.width);
            let floor = r.floor_at(p).unwrap_or(l.floor);
            // Two posts at the walls, a cap across under the roof.
            let post = |x: f32| {
                (
                    Vec3::new(x, 1.1, 0.0),
                    Quat::IDENTITY,
                    Collider::cuboid(0.18, 2.2, 0.18),
                )
            };
            commands.spawn((
                Name::new("Shoring"),
                Shoring {
                    site,
                    hp: SHORING_HP,
                    width,
                },
                RigidBody::Static,
                Collider::compound(vec![post(-width * 0.5 + 0.12), post(width * 0.5 - 0.12)]),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                Transform::from_xyz(p[0], floor, p[1])
                    .with_rotation(Quat::from_rotation_y(rail_yaw(&pts, s))),
                Visibility::default(),
                ChildOf(site),
            ));
        }
    }
}

fn cart_collider() -> Collider {
    // The bed you stand on, and its two long sides (the ends open, to
    // step in at).
    Collider::compound(vec![
        (
            Vec3::new(0.0, BED.y * 0.5, 0.0),
            Quat::IDENTITY,
            Collider::cuboid(BED.x * 2.0, BED.y, BED.z * 2.0),
        ),
        (
            Vec3::new(-BED.x - 0.04, 0.55, 0.0),
            Quat::IDENTITY,
            Collider::cuboid(0.06, 0.5, BED.z * 2.0),
        ),
        (
            Vec3::new(BED.x + 0.04, 0.55, 0.0),
            Quat::IDENTITY,
            Collider::cuboid(0.06, 0.5, BED.z * 2.0),
        ),
    ])
}

/// The yaw (as placements turn, radians for `from_rotation_y`) along a
/// polyline at `s`: its long axis (Z) along the leg.
/// Each cart's footprint where it's set down on its rail: its middle
/// (site-local x/z), its half extents across and along, and its yaw; for
/// a plan of the site to route round (`sim::indoor`), the carts being
/// bodies of their own and not the site's pieces.
pub fn cart_footprints(prefab: &str) -> Vec<(Vec2, Vec2, f32)> {
    let Some(l) = ug::layout(prefab) else {
        return Vec::new();
    };
    l.carts
        .iter()
        .filter_map(|c| {
            let rail = l.rails.get(c.rail)?;
            let p = ug::along(rail, c.at);
            Some((
                Vec2::from(p),
                Vec2::new(BED.x + 0.1, BED.z),
                rail_yaw(rail, c.at),
            ))
        })
        .collect()
}

fn rail_yaw(pts: &[[f32; 2]], s: f32) -> f32 {
    let a = ug::along(pts, (s - 0.2).max(0.0));
    let b = ug::along(pts, s + 0.2);
    let d = Vec2::new(b[0] - a[0], b[1] - a[1]);
    if d.length() < 1e-4 {
        return 0.0;
    }
    // Local +Z onto d: yaw = atan2(d.x, d.z).
    d.x.atan2(d.y)
}

/// Breathing it: it hurts, slowly; marked for the hiss and the blur.
fn breathe(
    mut commands: Commands,
    gas: Res<Gas>,
    sites: Query<(Entity, &Building, &Transform)>,
    mut bodies: Query<
        (
            Entity,
            &GlobalTransform,
            &mut crate::health::Health,
            Has<InGas>,
        ),
        With<crate::player::Player>,
    >,
    time: Res<Time>,
) {
    for (e, gt, mut health, marked) in &mut bodies {
        let head = gt.translation() + Vec3::Y * 1.5;
        let inside = sites.iter().any(|(site, b, xf)| {
            pocket_at(b, xf, head)
                .is_some_and(|g| gas.live.get(&site).is_some_and(|v| v.get(g) == Some(&true)))
        });
        if inside {
            health.hp -= GAS_HARM * time.delta_secs();
            if !marked {
                commands.entity(e).insert(InGas);
            }
        } else if marked {
            commands.entity(e).remove::<InGas>();
        }
    }
}

/// A spark in a live pocket: a shot fired from or through it, a grenade
/// going off in it, a torch switched on in it, a burning flare in it. It goes up: everyone in
/// it is hurt, it's heard a long way off, and the air's clear after.
#[allow(clippy::too_many_arguments)]
fn ignite(
    mut shots: MessageReader<crate::weapons::ballistics::Shot>,
    mut grenades: MessageReader<crate::weapons::grenades::GrenadeEvent>,
    torches: Query<(&crate::gadgets::Device, &GlobalTransform), Added<crate::gadgets::Powered>>,
    flares: Query<&GlobalTransform, With<super::glow::Burning>>,
    sites: Query<(Entity, &Building, &Transform)>,
    parts: Query<(Entity, &GlobalTransform), With<crate::robots::RobotPart>>,
    bodies: Query<(Entity, &GlobalTransform), With<crate::player::BodyCollider>>,
    mut gas: ResMut<Gas>,
    mut hits: MessageWriter<crate::weapons::ballistics::Hit>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut out: MessageWriter<GasFlash>,
    mut ignited: MessageWriter<GasIgnited>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    if gas.live.is_empty() {
        shots.clear();
        grenades.clear();
        return;
    }
    let mut sparks: Vec<Vec3> = Vec::new();
    for s in shots.read() {
        let dir = s.direction.normalize_or_zero();
        sparks.extend((0..60).map(|k| s.origin + dir * (k as f32 * 0.5)));
    }
    for g in grenades.read() {
        if g.event == extraction_core::grenade::Event::Exploded {
            sparks.push(g.at);
        }
    }
    for (d, gt) in &torches {
        if d.kind == crate::gadgets::DeviceKind::Light {
            sparks.push(gt.translation());
        }
    }
    // A burning flare: an open flame, carried or thrown into it.
    sparks.extend(flares.iter().map(|gt| gt.translation()));
    for p in sparks {
        for (site, b, xf) in &sites {
            let Some(g) = pocket_at(b, xf, p) else {
                continue;
            };
            let Some(live) = gas.live.get_mut(&site).and_then(|v| v.get_mut(g)) else {
                continue;
            };
            if !*live {
                continue;
            }
            *live = false;
            for (target, tgt) in parts.iter().chain(bodies.iter()) {
                let t = tgt.translation();
                if pocket_at(b, xf, t) == Some(g) {
                    hits.write(crate::weapons::ballistics::Hit {
                        entity: target,
                        point: t,
                        normal: (p - t).normalize_or(Vec3::Y),
                        velocity: (t - p).normalize_or(Vec3::Y) * 30.0,
                        damage: GAS_BLAST,
                        shooter: None,
                        mass_g: 0.0,
                        load: Default::default(),
                        blow: None,
                    });
                }
            }
            noise.write(crate::robots::Noise {
                at: p,
                radius: GAS_HEARD,
                source: None,
            });
            out.write(GasFlash { at: p });
            ignited.write(GasIgnited {
                site,
                pocket: g,
                at: p,
            });
            raid.say("The firedamp goes up with a thump that shakes the timbers.");
        }
    }
}

/// A flash where a pocket went up (for the client's light and sound).
#[derive(Message, Clone, Copy, Debug)]
pub struct GasFlash {
    pub at: Vec3,
}

/// Canaries: how near live gas is, and their life ebbing in it.
fn canaries(
    mut commands: Commands,
    catalog: Res<crate::items::Catalog>,
    mut items: Query<(
        Entity,
        &crate::items::Item,
        &GlobalTransform,
        Option<&mut Canary>,
    )>,
    gas: Res<Gas>,
    sites: Query<(Entity, &Transform), With<Building>>,
    time: Res<Time>,
) {
    let Some(kind) = catalog.kind("canary") else {
        return;
    };
    for (e, item, gt, canary) in &mut items {
        if item.kind() != kind {
            continue;
        }
        let p = gt.translation();
        let mut nearest = f32::INFINITY;
        for (site, xf) in &sites {
            let (Some(live), Some(pts)) = (gas.live.get(&site), gas.points.get(&site)) else {
                continue;
            };
            for (g, q) in pts {
                if live.get(*g) == Some(&true) {
                    nearest = nearest.min(xf.transform_point(*q).distance(p));
                }
            }
        }
        let alarm = (1.0 - nearest / CANARY_SENSE).clamp(0.0, 1.0);
        match canary {
            None => {
                commands.entity(e).insert(Canary { alarm, life: 1.0 });
            }
            Some(mut c) => {
                if c.life <= 0.0 {
                    c.alarm = 0.0;
                    continue;
                }
                c.alarm = alarm;
                if nearest < 1.0 {
                    c.life -= time.delta_secs() / CANARY_LIFE;
                }
            }
        }
    }
}

/// Pressing a cart sets it rolling: away from you along its rail, or if
/// you're in it, the way you face.
fn push_carts(
    mut events: MessageReader<crate::interact::InteractEvent>,
    mut carts: Query<(&mut MineCart, &GlobalTransform)>,
    players: Query<&GlobalTransform, With<crate::player::Player>>,
    mut noise: MessageWriter<crate::robots::Noise>,
) {
    for ev in events.read() {
        let crate::interact::InteractEvent::Pressed { item, player } = ev else {
            continue;
        };
        let Ok((mut cart, at)) = carts.get_mut(*item) else {
            continue;
        };
        let along = at.rotation() * Vec3::Z;
        let who = players.get(*player).map(|g| *g).unwrap_or(*at);
        let local = at.affine().inverse().transform_point3(who.translation());
        let riding = local.x.abs() < BED.x && local.z.abs() < BED.z;
        let way = if riding {
            who.forward().as_vec3().dot(along)
        } else {
            (at.translation() - who.translation()).dot(along)
        };
        cart.v = CART_PUSH * way.signum();
        noise.write(crate::robots::Noise {
            at: at.translation(),
            radius: 15.0,
            source: None,
        });
    }
}

/// Carts roll along their rails, slowing, stopping at the ends; whoever
/// stands in one goes with it.
#[allow(clippy::type_complexity)]
fn roll_carts(
    mut carts: Query<(&mut MineCart, &mut Transform, &GlobalTransform, &ChildOf)>,
    sites: Query<(&Building, &Transform), Without<MineCart>>,
    mut riders: Query<
        &mut Transform,
        (
            With<crate::player::Player>,
            Without<MineCart>,
            Without<Building>,
        ),
    >,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut cart, mut t, gt, parent) in &mut carts {
        let Ok((b, xf)) = sites.get(parent.parent()) else {
            continue;
        };
        let (Some(l), Some(r)) = (ug::layout(&b.prefab), ug::raster(&b.prefab)) else {
            continue;
        };
        let Some(rail) = l.rails.get(cart.rail) else {
            continue;
        };
        let before = xf.transform_point(t.translation);
        if cart.last == Vec3::ZERO {
            cart.last = before;
        }
        if cart.v != 0.0 {
            let len = ug::length(rail);
            let ends = (BED.z, len - BED.z);
            let slow = (cart.v.abs() - CART_DRAG * dt).max(0.0);
            cart.v = slow * cart.v.signum();
            cart.s += cart.v * dt;
            if cart.s <= ends.0 || cart.s >= ends.1 {
                cart.s = cart.s.clamp(ends.0, ends.1);
                cart.v = 0.0;
            }
            let p = ug::along(rail, cart.s);
            let floor = r.floor_at(p).unwrap_or(l.floor);
            t.translation = Vec3::new(p[0], floor, p[1]);
            t.rotation = Quat::from_rotation_y(rail_yaw(rail, cart.s));
        }
        let now = xf.transform_point(t.translation);
        let moved = now - cart.last;
        cart.last = now;
        if moved.length_squared() < 1e-8 {
            continue;
        }
        // Riders: feet over the bed, near its top, as it was last tick.
        let was = gt.affine().inverse();
        for mut rt in &mut riders {
            let l = was.transform_point3(rt.translation);
            if l.x.abs() < BED.x + 0.05
                && l.z.abs() < BED.z + 0.05
                && l.y > BED.y - 0.25
                && l.y < BED.y + 0.6
            {
                rt.translation += moved;
            }
        }
    }
}

/// Shooting the shoring: a timber set takes its hits and gives; the roof
/// comes down there, for good.
fn shoot_shoring(
    mut commands: Commands,
    mut hits: MessageReader<crate::weapons::ballistics::Hit>,
    mut timbers: Query<(Entity, &mut Shoring, &Transform, &GlobalTransform)>,
    mut fell: MessageWriter<RoofFell>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for h in hits.read() {
        let Ok((e, mut sh, t, gt)) = timbers.get_mut(h.entity) else {
            continue;
        };
        sh.hp = sh.hp.saturating_sub(1);
        if sh.hp > 0 {
            raid.say("The prop splinters. Dust sifts down.");
            continue;
        }
        let at = gt.translation();
        commands.entity(e).despawn();
        commands.spawn((
            Name::new("Roof fall"),
            Fall { width: sh.width },
            RigidBody::Static,
            Collider::cuboid(sh.width + 0.2, 2.6, 2.0),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
            Transform::from_translation(t.translation + Vec3::Y * 1.3).with_rotation(t.rotation),
            Visibility::default(),
            ChildOf(sh.site),
        ));
        fell.write(RoofFell { site: sh.site, at });
        noise.write(crate::robots::Noise {
            at,
            radius: 60.0,
            source: None,
        });
        raid.say("The prop goes and the roof comes down behind it in a roar of stone.");
    }
}

/// Client: a canary's glow, green to amber to red as gas nears; dark,
/// dead.
fn canary_glow(
    mut commands: Commands,
    canaries: Query<(Entity, &Canary, Option<&Children>), Changed<Canary>>,
    mut lights: Query<&mut PointLight>,
) {
    for (e, c, children) in &canaries {
        let colour = Color::from(lamp_colour(c.alarm));
        let intensity = if c.life > 0.0 { 900.0 } else { 0.0 };
        let mut found = false;
        for k in children.into_iter().flatten() {
            if let Ok(mut l) = lights.get_mut(*k) {
                l.color = colour;
                l.intensity = intensity;
                found = true;
            }
        }
        if !found {
            commands.spawn((
                PointLight {
                    color: colour,
                    intensity,
                    range: 3.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                // At the bird's breast (Kit's `glow`).
                Transform::from_xyz(0.0, -0.04, -0.022),
                ChildOf(e),
            ));
        }
    }
}

/// The bird lamp's flame: a calm warm white with no gas about, amber as
/// it gathers, red near the pocket's heart.
fn lamp_colour(alarm: f32) -> LinearRgba {
    let calm = LinearRgba::rgb(1.0, 0.78, 0.45);
    let amber = LinearRgba::rgb(1.0, 0.45, 0.08);
    let red = LinearRgba::rgb(1.0, 0.08, 0.03);
    let a = alarm.clamp(0.0, 1.0);
    if a < 0.5 {
        calm.mix(&amber, a * 2.0)
    } else {
        amber.mix(&red, (a - 0.5) * 2.0)
    }
}

/// A canary's lamp window, its own material (Kit's `glow` node shares the
/// library's `Lamp`).
#[derive(Component)]
struct CanaryLamp(Handle<StandardMaterial>);

/// Client: the bird lamp's window glows with the canary's light, amber to
/// red as the gas gathers, and goes dark when it's dead.
fn canary_lamp(
    mut commands: Commands,
    canaries: Query<(Entity, Ref<Canary>, Option<&CanaryLamp>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, c, lamp) in &canaries {
        let Some(lamp) = lamp else {
            // Once its model's in: the `glow` node's meshes, a material
            // of their own.
            let Some(glow) = children
                .iter_descendants(e)
                .find(|d| names.get(*d).is_ok_and(|n| n.as_str() == "glow"))
            else {
                continue;
            };
            let parts: Vec<Entity> = std::iter::once(glow)
                .chain(children.iter_descendants(glow))
                .filter(|d| meshes.contains(*d))
                .collect();
            let Some(first) = parts.first() else {
                continue;
            };
            let base = materials
                .get(&meshes.get(*first).unwrap().0)
                .cloned()
                .unwrap_or_default();
            let own = materials.add(base);
            for d in parts {
                if let Ok(mut m) = meshes.get_mut(d) {
                    m.0 = own.clone();
                }
            }
            commands.entity(e).insert(CanaryLamp(own));
            continue;
        };
        if !c.is_changed() {
            continue;
        }
        let Some(mut m) = materials.get_mut(&lamp.0) else {
            continue;
        };
        let lit = if c.life > 0.0 { 3.0 } else { 0.0 };
        m.emissive = lamp_colour(c.alarm) * lit;
    }
}

/// Client: a timber set drawn (two posts, a cap).
fn timber_looks(
    mut commands: Commands,
    timbers: Query<(Entity, &Shoring), Added<Shoring>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, sh) in &timbers {
        let wood = materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.3, 0.18),
            perceptual_roughness: 0.9,
            ..default()
        });
        let w = sh.width;
        for (size, at) in [
            (
                Vec3::new(0.18, 2.2, 0.18),
                Vec3::new(-w * 0.5 + 0.12, 1.1, 0.0),
            ),
            (
                Vec3::new(0.18, 2.2, 0.18),
                Vec3::new(w * 0.5 - 0.12, 1.1, 0.0),
            ),
            (Vec3::new(w, 0.2, 0.22), Vec3::new(0.0, 2.25, 0.0)),
        ] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(size))),
                MeshMaterial3d(wood.clone()),
                Transform::from_translation(at),
                ChildOf(e),
            ));
        }
    }
}

/// Client: a roof fall drawn, a heap of slabs across the corridor.
fn fall_looks(
    mut commands: Commands,
    falls: Query<(Entity, &Fall), Added<Fall>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, f) in &falls {
        let stone = materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.34, 0.31),
            perceptual_roughness: 1.0,
            ..default()
        });
        for k in 0..7 {
            let a = k as f32 * 2.1;
            let size = Vec3::new(0.6 + (k % 3) as f32 * 0.25, 0.4, 0.5 + (k % 2) as f32 * 0.3);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(size))),
                MeshMaterial3d(stone.clone()),
                Transform::from_xyz(
                    (a.sin() * 0.4) * f.width * 0.5,
                    -1.1 + (k as f32 * 0.32) % 1.8,
                    a.cos() * 0.5,
                )
                .with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    a,
                    0.3 * a.sin(),
                    0.2,
                )),
                ChildOf(e),
            ));
        }
    }
}

/// Client: a cart drawn (Kit's mine tub; a box till it's loaded).
fn cart_looks(
    mut commands: Commands,
    carts: Query<Entity, Added<MineCart>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use crate::render::world_props::{Blockout, SwapIn};
    for e in &carts {
        commands.spawn((
            Blockout,
            Mesh3d(meshes.add(Cuboid::new(BED.x * 2.0 + 0.1, 0.8, BED.z * 2.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.35, 0.22, 0.14),
                perceptual_roughness: 0.8,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.4, 0.0),
            ChildOf(e),
        ));
        commands.entity(e).insert(SwapIn::one("mine_tub"));
    }
}

/// A gas flash's light, fading.
#[derive(Component)]
struct Flash(f32);

/// Client: where a pocket went up, a burst of orange light that fades.
fn gas_flash(
    mut commands: Commands,
    mut flashes: MessageReader<GasFlash>,
    mut lit: Query<(Entity, &mut Flash, &mut PointLight)>,
    time: Res<Time>,
) {
    for f in flashes.read() {
        commands.spawn((
            Flash(0.8),
            PointLight {
                color: Color::srgb(1.0, 0.55, 0.2),
                intensity: 4.0e6,
                range: 30.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(f.at),
        ));
    }
    for (e, mut f, mut l) in &mut lit {
        f.0 -= time.delta_secs();
        if f.0 <= 0.0 {
            commands.entity(e).despawn();
        } else {
            l.intensity = 4.0e6 * f.0 / 0.8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::LaunchOptions;
    use crate::world::Zone;
    use bevy::ecs::system::RunSystemOnce;

    const SITE: &str = "deep_kettle";

    fn app() -> App {
        let mut app = crate::headless_app(
            LaunchOptions {
                robots: false,
                map: "kettle_pass".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        app
    }

    fn site(app: &mut App) -> (Entity, Transform) {
        app.world_mut()
            .query::<(Entity, &Building, &Transform)>()
            .iter(app.world())
            .find(|(_, b, _)| b.prefab == SITE)
            .map(|(e, _, t)| (e, *t))
            .expect("the deep workings placed")
    }

    fn room_at(xf: &Transform, id: &str, dx: f32, dz: f32, up: f32) -> Vec3 {
        let l = ug::layout(SITE).unwrap();
        let room = l.rooms.iter().find(|r| r.id == id).unwrap();
        xf.transform_point(Vec3::new(
            room.at[0] + dx,
            l.floor_of(room) + up,
            room.at[1] + dz,
        ))
    }

    /// Crossing the gas: a bot takes the canary off the landing's bench
    /// and walks toward the old heading, and the canary's alarm rises; in
    /// the gas it hurts; a shot into the pocket from outside sets it off
    /// (the pocket's gone, the bang's heard), and then the bot walks in
    /// and takes the crate.
    #[test]
    fn crossing_the_gas_with_the_canary() {
        use crate::bot::{Bot, Task};
        use crate::rig::{LocalPlayer, Side};
        use crate::sim::indoor::{BuildingPlan, plan_buildings};
        #[derive(Resource, Default)]
        struct Ignited(Vec<GasIgnited>);
        let mut app = app();
        app.init_resource::<Ignited>().add_systems(
            Update,
            |mut m: MessageReader<GasIgnited>, mut d: ResMut<Ignited>| {
                d.0.extend(m.read().copied());
            },
        );
        let zone = app.world().resource::<Zone>().clone();
        let (site, xf) = site(&mut app);
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
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let kind = catalog.kind("canary").expect("the canary in the catalog");
        let (canary, _) = app
            .world_mut()
            .query::<(Entity, &crate::items::Item)>()
            .iter(app.world())
            .find(|(_, i)| i.kind() == kind)
            .expect("the canary on its bench");
        let run = |app: &mut App, tasks: Vec<Task>, what: &str| {
            app.world_mut().entity_mut(player).insert(Bot::new(tasks));
            for _ in 0..90 * 120 {
                app.update();
                if app.world().get::<Bot>(player).unwrap().done() {
                    break;
                }
            }
            let bot = app.world().get::<Bot>(player).unwrap();
            assert_eq!(bot.failed, 0, "{what}: {:?}", bot.log);
        };
        let route = |app: &App, to: Vec3, within: f32| {
            let pos = app.world().get::<Transform>(player).unwrap().translation;
            crate::sim::raider::indoor_route(&plan, pos, plan.to_local(to).xz(), within)
                .unwrap_or_else(|| panic!("no route from {pos:?} to {to:?}"))
        };
        // In at the shaft, to the bench, the canary in hand.
        let entry = *plan
            .entries
            .iter()
            .min_by(|a, b| a.length().total_cmp(&b.length()))
            .unwrap();
        let start = plan.to_world(entry);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = Vec3::new(start.x, zone.ground(start.x, start.z) + 0.1, start.z);
        app.update();
        let bench = app
            .world()
            .get::<GlobalTransform>(canary)
            .unwrap()
            .translation();
        let mut tasks = route(&app, bench, 0.5);
        tasks.push(Task::Grab {
            item: canary,
            side: Side::Left,
        });
        tasks.push(Task::Wait(0.5));
        run(&mut app, tasks, "to the canary");
        let alarm = |app: &App| app.world().get::<Canary>(canary).map_or(-1.0, |c| c.alarm);
        assert!(alarm(&app) < 0.05, "quiet on the landing: {}", alarm(&app));
        // To the old heading's mouth (the haulage side): it smells it.
        let mouth = room_at(&xf, "haulage", -1.0, 0.0, 0.0);
        let tasks = route(&app, mouth, 0.5);
        run(&mut app, tasks, "toward the heading");
        for _ in 0..10 {
            app.update();
        }
        assert!(
            alarm(&app) > 0.3,
            "the canary goes amber near the gas: {}",
            alarm(&app)
        );
        // In the gas, it hurts.
        let hp = |app: &App| app.world().get::<crate::health::Health>(player).unwrap().hp;
        let before = hp(&app);
        let inside = room_at(&xf, "old_heading", 0.0, 0.0, 0.05);
        app.world_mut().entity_mut(player).remove::<Bot>();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = inside;
        for _ in 0..90 * 2 {
            app.update();
        }
        assert!(app.world().get::<InGas>(player).is_some(), "breathing it");
        assert!(
            hp(&app) < before - 1.0,
            "it hurts: {before} -> {}",
            hp(&app)
        );
        // Back out to the haulage, and a shot into the heading.
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = room_at(&xf, "haulage", 0.0, 0.0, 0.05);
        for _ in 0..30 {
            app.update();
        }
        assert!(app.world().get::<InGas>(player).is_none(), "out of it");
        let from = room_at(&xf, "haulage", 0.0, 0.0, 1.5);
        let to = room_at(&xf, "old_heading", 0.0, 0.0, 1.5);
        let gun = app.world_mut().spawn_empty().id();
        app.world_mut()
            .write_message(crate::weapons::ballistics::Shot {
                gun,
                shooter: Some(player),
                origin: from,
                direction: (to - from).normalize(),
                speed: 400.0,
                damage: 30.0,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: 1,
            });
        for _ in 0..5 {
            app.update();
        }
        let ignited = &app.world().resource::<Ignited>().0;
        assert_eq!(ignited.len(), 1, "it went up: {ignited:?}");
        assert_eq!(
            app.world().resource::<Gas>().live.get(&site).map(|v| v[0]),
            Some(false),
            "the heading's air is clear"
        );
        for _ in 0..10 {
            app.update();
        }
        assert!(alarm(&app) < 0.05, "the canary quietens: {}", alarm(&app));
        // In, and the crate.
        let l = ug::layout(SITE).unwrap();
        let heading = l.rooms.iter().find(|r| r.id == "old_heading").unwrap();
        let crate_at = room_at(
            &xf,
            "old_heading",
            heading.props[0].at[0],
            heading.props[0].at[1],
            0.0,
        );
        let tasks = route(&app, crate_at, 0.6);
        run(&mut app, tasks, "into the heading");
        let at = app.world().get::<Transform>(player).unwrap().translation;
        assert!(
            at.xz().distance(crate_at.xz()) < 1.6,
            "at the crate: {at:?}"
        );
        assert!(app.world().get::<InGas>(player).is_none(), "and breathing");
    }

    /// The cart rolls along its rail when pushed, and carries whoever
    /// stands in it; two hits on the shoring and the roof comes down,
    /// shutting the corridor.
    #[test]
    fn the_cart_rolls_and_the_roof_falls() {
        use crate::rig::LocalPlayer;
        #[derive(Resource, Default)]
        struct Fell(usize);
        let mut app = app();
        app.init_resource::<Fell>().add_systems(
            Update,
            |mut m: MessageReader<RoofFell>, mut d: ResMut<Fell>| {
                d.0 += m.read().count();
            },
        );
        let (_, xf) = site(&mut app);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let (cart, at) = app
            .world_mut()
            .query::<(Entity, &MineCart, &GlobalTransform)>()
            .iter(app.world())
            .next()
            .map(|(e, _, g)| (e, *g))
            .expect("a cart");
        // Stand in the bed, facing down the rail, and push off.
        let bed = at.transform_point(Vec3::new(0.0, BED.y + 0.02, 0.0));
        let along = at.rotation() * Vec3::Z;
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = bed;
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .rotation = Transform::default().looking_to(along, Vec3::Y).rotation;
        for _ in 0..20 {
            app.update();
        }
        let s0 = app.world().get::<MineCart>(cart).unwrap().s;
        let feet0 = app.world().get::<Transform>(player).unwrap().translation;
        app.world_mut()
            .write_message(crate::interact::InteractEvent::Pressed { item: cart, player });
        for _ in 0..90 * 3 {
            app.update();
        }
        let c = app.world().get::<MineCart>(cart).unwrap();
        assert!(c.s > s0 + 3.0, "it rolled: {s0} -> {}", c.s);
        let feet = app.world().get::<Transform>(player).unwrap().translation;
        assert!(
            feet.distance(feet0) > 3.0,
            "and carried you: {feet0:?} -> {feet:?}"
        );
        // The shoring: two hits.
        let (timber, t) = app
            .world_mut()
            .query::<(Entity, &Shoring, &GlobalTransform)>()
            .iter(app.world())
            .next()
            .map(|(e, _, g)| (e, g.translation()))
            .expect("shoring");
        for _ in 0..2 {
            app.world_mut()
                .write_message(crate::weapons::ballistics::Hit {
                    entity: timber,
                    point: t + Vec3::Y,
                    normal: Vec3::Z,
                    velocity: Vec3::NEG_Z * 300.0,
                    damage: 40.0,
                    shooter: Some(player),
                    mass_g: 8.0,
                    load: Default::default(),
                    blow: None,
                });
            app.update();
        }
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(app.world().resource::<Fell>().0, 1, "the roof came down");
        assert!(app.world().get_entity(timber).is_err(), "the prop's gone");
        // The corridor's shut: a ray down it meets the fall.
        let heading = room_at(&xf, "old_heading", 0.0, 0.0, 1.2);
        let refuge = room_at(&xf, "refuge", 0.0, 0.0, 1.2);
        let mut q = bevy::ecs::system::SystemState::<SpatialQuery>::new(app.world_mut());
        let spatial = q.get(app.world()).unwrap();
        let dir = Dir3::new(refuge - heading).unwrap();
        let hit = spatial.cast_ray(
            heading,
            dir,
            heading.distance(refuge),
            true,
            &SpatialQueryFilter::default(),
        );
        let falls: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<Fall>>()
            .iter(app.world())
            .collect();
        assert!(
            hit.is_some_and(|h| falls.contains(&h.entity)),
            "the fall blocks the way: {hit:?}"
        );
    }
}
