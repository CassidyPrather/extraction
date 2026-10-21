//! The furnace cellars' mechanics, for any site whose layout has them
//! (`cellars_cinder` first): heat that burns, vents that cool it, and the
//! charging crane whose hook carries whoever holds its chain.
//!
//! Logic in [`FurnacePlugin`]; the hot rooms' glow (the cellars' only
//! light) and the crane drawn in [`FurnaceVisualsPlugin`].

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::underground as ug;
use std::collections::HashMap;

use super::Layer;
use super::buildings::Building;

/// Heat's harm at level 1, hit points a second (scorching: more).
pub const HEAT_HARM: f32 = 6.0;
pub const SCORCH_HARM: f32 = 25.0;
/// A vented zone's heat falls to this share of its own...
pub const VENTED: f32 = 0.1;
/// ...at this much a second.
pub const COOLING: f32 = 0.035;
/// The crane's speed along its run, m/s.
pub const CRANE_SPEED: f32 = 1.2;

pub struct FurnacePlugin;

impl Plugin for FurnacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Heat>()
            .add_message::<Vented>()
            .add_message::<CraneRidden>()
            .add_systems(Update, (wire_furnace, open_vents, cool, scorch))
            .add_systems(
                FixedUpdate,
                (send_crane, run_crane)
                    .chain()
                    .after(crate::interact::InteractSystems::Grab)
                    .before(crate::player::move_bodies),
            );
    }
}

pub struct FurnaceVisualsPlugin;

impl Plugin for FurnaceVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                glow,
                crane_looks,
                crane_models.after(crane_looks),
                lever_looks,
                lever_models.after(lever_looks),
                swing_levers,
            ),
        );
    }
}

/// Every site's heat zones: the share of its heat each still has (1 at
/// the start; vented, it falls to [`VENTED`]).
#[derive(Resource, Default, Debug)]
pub struct Heat(pub HashMap<Entity, Vec<f32>>);

impl Heat {
    /// How hot a world point is now (0 to the zone's level), if it's in a
    /// site's heat zone.
    pub fn at(&self, sites: &Query<(Entity, &Building, &Transform)>, p: Vec3) -> f32 {
        for (site, b, xf) in sites.iter() {
            let (Some(l), Some(r)) = (ug::layout(&b.prefab), ug::raster(&b.prefab)) else {
                continue;
            };
            let q = xf.compute_affine().inverse().transform_point3(p);
            let Some(z) = r.heat_at([q.x, q.z]) else {
                continue;
            };
            let Some(f) = r.floor_at([q.x, q.z]) else {
                continue;
            };
            if q.y < f - 0.5 || q.y > f + 3.0 {
                continue;
            }
            let share = self
                .0
                .get(&site)
                .and_then(|v| v.get(z as usize))
                .copied()
                .unwrap_or(1.0);
            return l.heat[z as usize].level * share;
        }
        0.0
    }
}

/// A vent's lever: pull it.
#[derive(Component, Debug)]
pub struct VentLever {
    pub site: Entity,
    pub vent: usize,
    pub open: bool,
}

/// A vent's opened.
#[derive(Message, Clone, Copy, Debug)]
pub struct Vented {
    pub site: Entity,
    pub vent: usize,
}

/// Someone rode the crane end to end, holding its chain.
#[derive(Message, Clone, Copy, Debug)]
pub struct CraneRidden {
    pub player: Entity,
    pub site: Entity,
}

/// How hot it is where a body is (for the shimmer and the sound).
#[derive(Component, Debug)]
pub struct InHeat(pub f32);

/// The crane: `t` along its run (0 `from`, 1 `to`), going `dir`.
#[derive(Component, Debug)]
pub struct Crane {
    pub site: Entity,
    pub t: f32,
    pub dir: f32,
    /// Its hook's chain (a rope you hold to ride).
    pub chain: Entity,
}

/// A crane's lever: sends it the other way.
#[derive(Component, Debug)]
pub struct CraneLever {
    pub crane: Entity,
}

/// Where a fitting is on a room's wall, site space, facing into the room.
fn wall_pose(l: &ug::Layout, room: &str, at: [f32; 2], rot: f32, up: f32) -> Option<Transform> {
    let room = l.rooms.iter().find(|r| r.id == room)?;
    let q = Quat::from_rotation_y(-rot.to_radians());
    let p = Vec3::new(
        room.at[0] + at[0],
        l.floor_of(room) + up,
        room.at[1] + at[1],
    );
    Some(Transform::from_translation(p + q * Vec3::new(0.0, 0.0, -0.12)).with_rotation(q))
}

fn lever_bundle(name: &'static str, pose: Transform, site: Entity) -> impl Bundle {
    (
        Name::new(name),
        crate::raid::truck::Pressable::BenchRun,
        crate::interact::Grabbable {
            points: vec![crate::interact::GrabPoint::free()],
        },
        Collider::cuboid(0.3, 0.4, 0.2),
        CollisionLayers::new(Layer::Item, LayerMask::NONE),
        pose,
        Visibility::default(),
        ChildOf(site),
    )
}

/// The crane's hook, world space, at `t` along its run.
fn hook_at(c: &ug::CraneDef, xf: &Transform, t: f32) -> Vec3 {
    let p = [
        c.from[0] + (c.to[0] - c.from[0]) * t,
        c.from[1] + (c.to[1] - c.from[1]) * t,
    ];
    xf.transform_point(Vec3::new(p[0], c.y, p[1]))
}

/// Logic, as sites are built: their heat at full, their vent levers, their
/// crane (parked at its far end) with its chain and levers.
fn wire_furnace(
    mut commands: Commands,
    buildings: Query<(Entity, &Building, &Transform), Added<Building>>,
    mut heat: ResMut<Heat>,
) {
    for (site, b, xf) in &buildings {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        if !l.heat.is_empty() {
            heat.0.insert(site, vec![1.0; l.heat.len()]);
        }
        for (k, v) in l.vents.iter().enumerate() {
            let Some(pose) = wall_pose(l, &v.room, v.at, v.rot, 1.2) else {
                continue;
            };
            commands.spawn((
                lever_bundle("Vent lever", pose, site),
                VentLever {
                    site,
                    vent: k,
                    open: false,
                },
            ));
        }
        let Some(c) = &l.crane else {
            continue;
        };
        // Parked at its far end: you call it over.
        let t = 1.0;
        let top = hook_at(c, xf, t);
        let chain = commands
            .spawn((
                Name::new("Crane chain"),
                crate::gadgets::climbing::Rope {
                    anchor: top,
                    top,
                    bottom: top - Vec3::Y * c.drop,
                    kit: Entity::PLACEHOLDER,
                    zip: false,
                },
                crate::interact::Grabbable {
                    points: vec![crate::interact::GrabPoint::free()],
                },
                RigidBody::Kinematic,
                Transform::from_translation(top - Vec3::Y * c.drop * 0.5),
                Visibility::default(),
            ))
            .with_child((
                Collider::capsule(0.06, (c.drop - 0.1).max(0.1)),
                CollisionLayers::new(Layer::Item, [Layer::Hand]),
                Sensor,
                Transform::default(),
            ))
            .id();
        let crane = commands
            .spawn((
                Name::new("Crane"),
                Crane {
                    site,
                    t,
                    dir: 0.0,
                    chain,
                },
                Transform::from_translation(top),
                Visibility::default(),
            ))
            .id();
        for (room, at, rot) in &c.levers {
            let Some(pose) = wall_pose(l, room, *at, *rot, 1.2) else {
                continue;
            };
            commands.spawn((
                lever_bundle("Crane lever", pose, site),
                CraneLever { crane },
            ));
        }
    }
}

/// A vent lever pulled: its zones start to cool (a clang, heard).
fn open_vents(
    mut events: MessageReader<crate::interact::InteractEvent>,
    mut levers: Query<(&mut VentLever, &GlobalTransform)>,
    mut vented: MessageWriter<Vented>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for ev in events.read() {
        let crate::interact::InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok((mut lever, at)) = levers.get_mut(*item) else {
            continue;
        };
        if lever.open {
            continue;
        }
        lever.open = true;
        vented.write(Vented {
            site: lever.site,
            vent: lever.vent,
        });
        noise.write(crate::robots::Noise {
            at: at.translation(),
            radius: 25.0,
            source: None,
        });
        raid.say("The damper clangs open. A long breath of hot air goes up the flue.");
    }
}

/// Vented zones cool, slowly.
fn cool(
    levers: Query<&VentLever>,
    buildings: Query<&Building>,
    mut heat: ResMut<Heat>,
    time: Res<Time>,
) {
    for lever in levers.iter().filter(|l| l.open) {
        let Some(v) = buildings
            .get(lever.site)
            .ok()
            .and_then(|b| ug::layout(&b.prefab))
            .and_then(|l| l.vents.get(lever.vent))
        else {
            continue;
        };
        let Some(shares) = heat.0.get_mut(&lever.site) else {
            continue;
        };
        for z in &v.zones {
            if let Some(s) = shares.get_mut(*z) {
                *s = (*s - COOLING * time.delta_secs()).max(VENTED);
            }
        }
    }
}

/// Heat hurts where you stand in it; scorching, fast.
fn scorch(
    mut commands: Commands,
    heat: Res<Heat>,
    sites: Query<(Entity, &Building, &Transform)>,
    mut bodies: Query<
        (
            Entity,
            &GlobalTransform,
            &mut crate::health::Health,
            Option<&InHeat>,
        ),
        With<crate::player::Player>,
    >,
    time: Res<Time>,
) {
    if heat.0.is_empty() {
        return;
    }
    for (e, gt, mut health, marked) in &mut bodies {
        let h = heat.at(&sites, gt.translation() + Vec3::Y * 0.2);
        if h > 0.01 {
            let harm = if h >= ug::SCORCHING {
                SCORCH_HARM
            } else {
                HEAT_HARM * h
            };
            health.hp -= harm * time.delta_secs();
            if marked.is_none_or(|m| (m.0 - h).abs() > 0.05) {
                commands.entity(e).insert(InHeat(h));
            }
        } else if marked.is_some() {
            commands.entity(e).remove::<InHeat>();
        }
    }
}

/// A crane lever sends it to the other end.
fn send_crane(
    mut events: MessageReader<crate::interact::InteractEvent>,
    levers: Query<&CraneLever>,
    mut cranes: Query<&mut Crane>,
    mut noise: MessageWriter<crate::robots::Noise>,
    hooks: Query<&GlobalTransform, With<Crane>>,
) {
    for ev in events.read() {
        let crate::interact::InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok(lever) = levers.get(*item) else {
            continue;
        };
        let Ok(mut c) = cranes.get_mut(lever.crane) else {
            continue;
        };
        c.dir = if c.t > 0.5 { -1.0 } else { 1.0 };
        if let Ok(at) = hooks.get(lever.crane) {
            noise.write(crate::robots::Noise {
                at: at.translation(),
                radius: 30.0,
                source: None,
            });
        }
    }
}

/// The crane runs; its chain goes with it, and so does a hand on it.
fn run_crane(
    mut cranes: Query<(&mut Crane, &mut Transform)>,
    buildings: Query<(&Building, &Transform), Without<Crane>>,
    mut chains: Query<
        (&mut crate::gadgets::climbing::Rope, &mut Transform),
        (Without<Crane>, Without<Building>),
    >,
    mut grips: Query<(&mut crate::gadgets::climbing::RopeGrip, &ChildOf)>,
    mut ridden: MessageWriter<CraneRidden>,
    time: Res<Time>,
) {
    for (mut c, mut t) in &mut cranes {
        if c.dir == 0.0 {
            continue;
        }
        let Ok((b, xf)) = buildings.get(c.site) else {
            continue;
        };
        let Some(def) = ug::layout(&b.prefab).and_then(|l| l.crane.as_ref()) else {
            continue;
        };
        let len = Vec2::from(def.from).distance(Vec2::from(def.to)).max(0.1);
        c.t = (c.t + c.dir * CRANE_SPEED / len * time.delta_secs()).clamp(0.0, 1.0);
        let arrived = c.t <= 0.0 || c.t >= 1.0;
        if arrived {
            c.dir = 0.0;
        }
        let top = hook_at(def, xf, c.t);
        let moved = top - t.translation;
        t.translation = top;
        let chain = c.chain;
        if let Ok((mut rope, mut rt)) = chains.get_mut(chain) {
            rope.anchor = top;
            rope.top = top;
            rope.bottom = top - Vec3::Y * def.drop;
            rt.translation = top - Vec3::Y * def.drop * 0.5;
        }
        for (mut g, hand) in &mut grips {
            if g.rope == chain {
                g.at += moved;
                if arrived {
                    ridden.write(CraneRidden {
                        player: hand.parent(),
                        site: c.site,
                    });
                }
            }
        }
    }
}

/// Client: the hot rooms glow, dull red, fading as they're vented: the
/// cellars' only light.
fn glow(
    mut commands: Commands,
    buildings: Query<(Entity, &Building)>,
    heat: Res<Heat>,
    mut lights: Query<(&HeatGlow, &mut PointLight)>,
    mut done: Local<Vec<Entity>>,
) {
    for (site, b) in &buildings {
        if done.contains(&site) {
            continue;
        }
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        if l.heat.is_empty() {
            continue;
        }
        done.push(site);
        for (z, zone) in l.heat.iter().enumerate() {
            let Some(room) = zone
                .room
                .as_ref()
                .and_then(|id| l.rooms.iter().find(|r| &r.id == id))
            else {
                continue;
            };
            commands.spawn((
                HeatGlow { site, zone: z },
                PointLight {
                    color: Color::srgb(1.0, 0.32, 0.08),
                    intensity: 0.0,
                    range: room.size[0].max(room.size[1]) + 3.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(room.at[0], l.floor_of(room) + 0.4, room.at[1]),
                super::underground::Below,
                ChildOf(site),
            ));
        }
    }
    for (g, mut light) in &mut lights {
        let share = heat
            .0
            .get(&g.site)
            .and_then(|v| v.get(g.zone))
            .copied()
            .unwrap_or(1.0);
        light.intensity = 60_000.0 * share;
    }
}

/// A heat zone's glow.
#[derive(Component)]
struct HeatGlow {
    site: Entity,
    zone: usize,
}

/// Client: the crane's bridge beam and hook block, and its chain.
fn crane_looks(
    mut commands: Commands,
    cranes: Query<(Entity, &Crane), Added<Crane>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, c) in &cranes {
        let iron = materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.15, 0.14),
            perceptual_roughness: 0.7,
            metallic: 0.6,
            ..default()
        });
        commands.spawn((
            crate::render::world_props::Blockout,
            Mesh3d(meshes.add(Cuboid::new(0.6, 0.5, 0.6))),
            MeshMaterial3d(iron.clone()),
            Transform::from_xyz(0.0, 0.2, 0.0),
            ChildOf(e),
        ));
        commands.spawn((
            crate::render::world_props::Blockout,
            Mesh3d(meshes.add(Cylinder::new(0.035, 1.5))),
            MeshMaterial3d(iron.clone()),
            Transform::default(),
            ChildOf(c.chain),
        ));
        commands.spawn((
            crate::render::world_props::Blockout,
            Mesh3d(meshes.add(Cuboid::new(0.3, 0.35, 0.2))),
            MeshMaterial3d(iron),
            Transform::from_xyz(0.0, -0.6, 0.0),
            ChildOf(c.chain),
        ));
    }
}

/// Its models are in (a crane's bridge and hook, a lever's box).
#[derive(Component)]
struct Dressed;

/// A lever's arm, drawn apart so it can swing: down (about its pivot's
/// X) as its vent opens.
#[derive(Component)]
struct LeverArm {
    rest: Quat,
    angle: f32,
}

/// How far a vent's lever swings down, open (radians).
const LEVER_DOWN: f32 = -1.75;

/// Spawns a model's parts (or those named, or all but those) as children
/// of `parent`, each placed at `at` and its own node's transform.
fn spawn_parts(
    commands: &mut Commands,
    props: &mut crate::render::world_props::PropModels,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
    parts: &[crate::render::world_props::Part],
    at: Transform,
    keep: impl Fn(&str) -> bool,
) {
    for part in parts.iter().filter(|p| keep(&p.name)) {
        for (mesh, t, mat) in &part.prims {
            let material = props.material(mat, materials);
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                at * part.at * *t,
                ChildOf(parent),
            ));
        }
    }
}

/// Client: the crane's models in place of its blockout once they're in:
/// the bridge across the pit (from wall to wall, under the roof), the
/// hook block at the chain's foot with the chain up to the bridge.
fn crane_models(
    mut commands: Commands,
    cranes: Query<(Entity, &Crane, &Children), Without<Dressed>>,
    chains: Query<&Children>,
    blockouts: Query<(), With<crate::render::world_props::Blockout>>,
    buildings: Query<(&Building, &Transform)>,
    mut props: crate::render::world_props::PropModels,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, c, kids) in &cranes {
        let Ok((b, xf)) = buildings.get(c.site) else {
            continue;
        };
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        let Some(def) = &l.crane else {
            continue;
        };
        let (Some(bridge), Some(hook)) = (
            props.parts("cellar_crane_bridge"),
            props.parts("cellar_crane_hook"),
        ) else {
            continue;
        };
        commands.entity(e).insert(Dressed);
        for k in kids.iter().filter(|k| blockouts.contains(*k)) {
            commands.entity(k).despawn();
        }
        for k in chains
            .get(c.chain)
            .into_iter()
            .flatten()
            .copied()
            .filter(|k| blockouts.contains(*k))
        {
            commands.entity(k).despawn();
        }
        // Across the run, centred over the pit.
        let (from, to) = (Vec2::from(def.from), Vec2::from(def.to));
        let run = (to - from).normalize_or(Vec2::Y);
        let across = Vec2::new(run.y, -run.x);
        let pit = l
            .rooms
            .iter()
            .find(|r| r.pit)
            .map_or(0.0, |r| (Vec2::from(r.at) - from).dot(across));
        let turn = xf.rotation * Quat::from_rotation_y((-across.y).atan2(across.x));
        // Its top just under the roof (the hook's top 0.6 under that).
        let bridge_at = Transform::from_translation(
            xf.rotation * Vec3::new(across.x * pit, 0.55, across.y * pit),
        )
        .with_rotation(turn);
        spawn_parts(
            &mut commands,
            &mut props,
            &mut materials,
            e,
            &bridge,
            bridge_at,
            |_| true,
        );
        // The block at the chain's foot; the chain up to the beam's
        // underside, half a metre over the hook's top.
        let foot = Transform::from_xyz(0.0, -def.drop * 0.5, 0.0).with_rotation(turn);
        spawn_parts(
            &mut commands,
            &mut props,
            &mut materials,
            c.chain,
            &hook,
            foot,
            |n| n != "chain",
        );
        let reach = (def.drop - 0.05).max(0.1);
        spawn_parts(
            &mut commands,
            &mut props,
            &mut materials,
            c.chain,
            &hook,
            foot * Transform::from_scale(Vec3::new(1.0, reach, 1.0)),
            |n| n == "chain",
        );
    }
}

/// Client: a box on the wall for each vent and crane lever until its
/// model's in.
fn lever_looks(
    mut commands: Commands,
    levers: Query<Entity, Or<(Added<VentLever>, Added<CraneLever>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for e in &levers {
        commands.spawn((
            crate::render::world_props::Blockout,
            Mesh3d(meshes.add(Cuboid::new(0.5, 0.8, 0.25))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.12, 0.11, 0.1),
                perceptual_roughness: 0.7,
                metallic: 0.5,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.0, 0.12 - 0.125),
            ChildOf(e),
        ));
    }
}

/// Client: the cast-iron lever box (Kit's `cellar_vent`) on each vent's
/// and crane's lever, its back on the wall, its arm apart to swing.
fn lever_models(
    mut commands: Commands,
    levers: Query<(Entity, &Children), (Or<(With<VentLever>, With<CraneLever>)>, Without<Dressed>)>,
    blockouts: Query<(), With<crate::render::world_props::Blockout>>,
    mut props: crate::render::world_props::PropModels,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, kids) in &levers {
        let Some(parts) = props.parts("cellar_vent") else {
            continue;
        };
        commands.entity(e).insert(Dressed);
        for k in kids.iter().filter(|k| blockouts.contains(*k)) {
            commands.entity(k).despawn();
        }
        // (The pose stands 0.12 off the wall; the model's origin is on it.)
        let wall = Transform::from_xyz(0.0, 0.0, 0.12);
        spawn_parts(
            &mut commands,
            &mut props,
            &mut materials,
            e,
            &parts,
            wall,
            |n| n != "lever",
        );
        if let Some(arm) = parts.iter().find(|p| p.name == "lever") {
            let pivot = wall * arm.at;
            let a = commands
                .spawn((
                    LeverArm {
                        rest: pivot.rotation,
                        angle: 0.0,
                    },
                    pivot,
                    Visibility::default(),
                    ChildOf(e),
                ))
                .id();
            for (mesh, t, mat) in &arm.prims {
                let material = props.material(mat, &mut materials);
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    *t,
                    ChildOf(a),
                ));
            }
        }
    }
}

/// Client: a vent's lever swings down as it's opened.
fn swing_levers(
    vents: Query<&VentLever>,
    mut arms: Query<(&mut LeverArm, &mut Transform, &ChildOf)>,
    time: Res<Time>,
) {
    for (mut arm, mut t, parent) in &mut arms {
        let want = if vents.get(parent.parent()).is_ok_and(|v| v.open) {
            LEVER_DOWN
        } else {
            0.0
        };
        if (arm.angle - want).abs() < 1e-3 {
            continue;
        }
        let step = 4.0 * time.delta_secs();
        arm.angle += (want - arm.angle).clamp(-step, step);
        t.rotation = arm.rest * Quat::from_rotation_x(arm.angle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::LaunchOptions;
    use bevy::ecs::system::RunSystemOnce;

    const SITE: &str = "cellars_cinder";

    /// Venting the cellar and riding the crane: the flue's scorching till
    /// a bot pulls its vent lever, then hardly warm; on the near ledge a
    /// lever calls the crane over; the bot takes hold of its chain, the
    /// lever sends it across, and over the ash pit it goes, to let go on
    /// the far ledge unburned.
    #[test]
    fn venting_the_cellar_and_riding_the_crane() {
        use crate::bot::{Bot, Task};
        use crate::rig::{LocalPlayer, Side};
        use crate::sim::indoor::{BuildingPlan, plan_buildings};
        let mut app = crate::headless_app(
            LaunchOptions {
                robots: false,
                map: "cinder_works".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        #[derive(Resource, Default)]
        struct Rode(Vec<Entity>);
        app.init_resource::<Rode>().add_systems(
            Update,
            |mut m: MessageReader<CraneRidden>, mut d: ResMut<Rode>| {
                d.0.extend(m.read().map(|m| m.player));
            },
        );
        for _ in 0..3 {
            app.update();
        }
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let (site, xf) = app
            .world_mut()
            .query::<(Entity, &Building, &Transform)>()
            .iter(app.world())
            .find(|(_, b, _)| b.prefab == SITE)
            .map(|(e, _, t)| (e, *t))
            .expect("the cellars placed");
        let l = ug::layout(SITE).unwrap();
        let at = |id: &str, dx: f32, dz: f32, up: f32| {
            let room = l.rooms.iter().find(|r| r.id == id).unwrap();
            xf.transform_point(Vec3::new(
                room.at[0] + dx,
                l.floor_of(room) + up,
                room.at[1] + dz,
            ))
        };
        let heat_at = |app: &mut App, p: Vec3| {
            app.world_mut()
                .run_system_once(
                    move |heat: Res<Heat>, sites: Query<(Entity, &Building, &Transform)>| {
                        heat.at(&sites, p)
                    },
                )
                .unwrap()
        };
        let flue = at("flue_passage", 0.0, 0.0, 0.2);
        assert!(
            heat_at(&mut app, flue) >= ug::SCORCHING,
            "the flue's scorching"
        );
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
        // In, and to the vent room's lever.
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
        let (lever, lever_at) = app
            .world_mut()
            .query::<(Entity, &VentLever, &GlobalTransform)>()
            .iter(app.world())
            .find(|(_, v, _)| v.site == site && v.vent == 0)
            .map(|(e, _, g)| (e, g.translation()))
            .expect("the flue's vent lever");
        let mut tasks = route(&app, lever_at, 0.6);
        tasks.push(Task::Press(lever));
        tasks.push(Task::Wait(1.0));
        run(&mut app, tasks, "to the vent");
        for _ in 0..90 * 30 {
            app.update();
        }
        let h = heat_at(&mut app, flue);
        assert!(h < 0.2, "the flue's vented: {h}");
        // Through the flue, hardly warm.
        let hp = |app: &App| app.world().get::<crate::health::Health>(player).unwrap().hp;
        app.world_mut().entity_mut(player).remove::<Bot>();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = flue;
        let before = hp(&app);
        for _ in 0..90 * 2 {
            app.update();
        }
        assert!(
            before - hp(&app) < 2.5,
            "warm, not burning: {before} -> {}",
            hp(&app)
        );
        // On the near ledge, call the crane over.
        let ledge = at("near_ledge", 1.0, 0.0, 0.05);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = ledge;
        for _ in 0..30 {
            app.update();
        }
        let (crane, chain) = app
            .world_mut()
            .query::<(Entity, &Crane)>()
            .iter(app.world())
            .next()
            .map(|(e, c)| (e, c.chain))
            .expect("the crane");
        let levers: Vec<(Entity, Vec3)> = app
            .world_mut()
            .query::<(Entity, &CraneLever, &GlobalTransform)>()
            .iter(app.world())
            .map(|(e, _, g)| (e, g.translation()))
            .collect();
        let near_lever = levers
            .iter()
            .min_by(|a, b| a.1.distance(ledge).total_cmp(&b.1.distance(ledge)))
            .unwrap()
            .0;
        app.world_mut()
            .write_message(crate::interact::InteractEvent::Pressed {
                item: near_lever,
                player,
            });
        for _ in 0..90 * 20 {
            app.update();
            if app.world().get::<Crane>(crane).unwrap().t <= 0.0 {
                break;
            }
        }
        assert!(
            app.world().get::<Crane>(crane).unwrap().t <= 0.0,
            "the crane came over"
        );
        // Hold the chain; send it across.
        // (A rope's taken hold of, not held as an item is: walk under it,
        // and the hand takes it.)
        let under = app
            .world()
            .get::<crate::gadgets::climbing::Rope>(chain)
            .unwrap()
            .bottom;
        let tasks = route(&app, under, 0.4);
        run(&mut app, tasks, "under the chain");
        app.world_mut()
            .write_message(crate::interact::HandAction::Grab {
                player,
                side: Side::Right,
                item: chain,
                point: None,
            });
        for _ in 0..30 {
            app.update();
        }
        let gripping = app
            .world_mut()
            .query::<&crate::gadgets::climbing::RopeGrip>()
            .iter(app.world())
            .any(|g| g.rope == chain);
        assert!(gripping, "holding the chain");
        app.world_mut().entity_mut(player).remove::<Bot>();
        app.world_mut()
            .write_message(crate::interact::InteractEvent::Pressed {
                item: near_lever,
                player,
            });
        let mut over_pit = false;
        for _ in 0..90 * 25 {
            app.update();
            let p = app.world().get::<Transform>(player).unwrap().translation;
            let local = xf.compute_affine().inverse().transform_point3(p);
            if (local.z - 46.0).abs() < 2.0 {
                over_pit = true;
            }
            if app.world().get::<Crane>(crane).unwrap().t >= 1.0 {
                break;
            }
        }
        assert!(over_pit, "carried over the pit");
        assert!(
            app.world().get::<Crane>(crane).unwrap().t >= 1.0,
            "the crane got across"
        );
        app.update();
        assert!(
            app.world().resource::<Rode>().0.contains(&player),
            "the ride's told"
        );
        // Let go.
        let hands: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<crate::gadgets::climbing::RopeGrip>>()
            .iter(app.world())
            .collect();
        for h in hands {
            app.world_mut()
                .entity_mut(h)
                .remove::<crate::gadgets::climbing::RopeGrip>();
        }
        let before = hp(&app);
        for _ in 0..90 * 2 {
            app.update();
        }
        let p = app.world().get::<Transform>(player).unwrap().translation;
        let far = at("far_ledge", 0.0, 0.0, 0.0);
        let local = xf.compute_affine().inverse().transform_point3(p);
        let room = l.rooms.iter().find(|r| r.id == "far_ledge").unwrap();
        assert!(
            (local.x - room.at[0]).abs() < room.size[0] * 0.5
                && (local.z - room.at[1]).abs() < room.size[1] * 0.5
                && (p.y - far.y).abs() < 0.3,
            "on the far ledge: {p:?} (its floor {:.2})",
            far.y
        );
        assert!(
            before - hp(&app) < 0.5,
            "unburned: {before} -> {}",
            hp(&app)
        );
    }
}
