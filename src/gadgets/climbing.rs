//! The climbing kit: a grapnel on a rope. Use it to shake out the line,
//! then throw the hook onto a roof or a ledge. If it catches, the rope
//! hangs from the nearest edge on your side, and you climb it hand over
//! hand: grab the rope and pull down to go up (or push the stick: flat
//! mode and comfort). Reach the top and you haul yourself over. Let go of
//! the rope and you fall. Pick the hook back up to coil the line.
//!
//! The zipline kit is the same rope, run downhill: stand somewhere high,
//! look where you want to go and use it. If the cable finds something to
//! bite on (8 to 60 m off, and below you), it runs from a post at your
//! feet to there. Grab it and you ride it down, faster and faster; you
//! drop off the end. Pick the kit back up from its post to reel it in.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::interact::body::InBackpack;
use crate::interact::{Hand, Held, InSocket};
use crate::items::usage::UseItem;
use crate::player::{Body, Player};
use crate::rig::{RigHand, RigInput};
use crate::world::{Layer, Zone};

/// Climbing speed with the stick, m/s.
pub const CLIMB_SPEED: f32 = 1.4;
/// Fastest you can haul yourself, m/s.
const MAX_PULL: f32 = 4.0;
/// The hook only catches this high above the ground.
const MIN_HEIGHT: f32 = 2.2;
/// Top speed on a zipline, m/s.
pub const ZIP_SPEED: f32 = 9.0;
/// How far a zipline cable can reach, and how near is worth it.
pub const ZIP_RANGE: (f32, f32) = (8.0, 60.0);
/// Farthest the hook looks for an edge to hang the rope from.
const LEDGE_SEARCH: f32 = 10.0;

/// The kit's hook.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum Grapnel {
    Coiled,
    /// Line out, ready to throw. `thrown` once it has left the hand.
    Ready {
        by: Entity,
        thrown: bool,
    },
    Hooked {
        rope: Entity,
    },
}

/// A rope hung from a hooked grapnel. `top` is at the edge it hangs over;
/// `anchor` is where the hook bit.
#[derive(Component, Debug, Clone, Copy)]
pub struct Rope {
    pub anchor: Vec3,
    pub top: Vec3,
    pub bottom: Vec3,
    pub kit: Entity,
    /// A zipline: you ride it down rather than climb it.
    pub zip: bool,
}

impl Rope {
    /// The nearest point on the hanging part of the rope.
    pub fn nearest(&self, p: Vec3) -> Vec3 {
        let ab = self.bottom - self.top;
        let t = ((p - self.top).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        self.top + ab * t
    }
}

/// A hand holding a rope at a point (world). The newest grip carries you.
#[derive(Component, Debug, Clone, Copy)]
pub struct RopeGrip {
    pub rope: Entity,
    pub at: Vec3,
    pub since: f32,
    /// Riding a zipline: how fast, m/s.
    pub speed: f32,
}

/// The zipline kit: packed, or rigged with its cable.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum Zipline {
    Packed,
    /// `set` once you've let go of it and it's sat on its post.
    Rigged {
        cable: Entity,
        set: bool,
    },
}

/// A body hanging on a rope: no walking, no gravity.
#[derive(Component, Debug, Clone, Copy)]
pub struct Climbing;

pub struct ClimbingPlugin;

impl Plugin for ClimbingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                ready_hooks,
                hooks.after(crate::interact::InteractSystems::Follow),
                (rig_ziplines, pack_ziplines)
                    .chain()
                    .after(crate::interact::InteractSystems::Follow),
                climb
                    .after(crate::interact::InteractSystems::Grab)
                    .before(crate::player::move_bodies),
            ),
        );
    }
}

fn ready_hooks(
    mut uses: MessageReader<UseItem>,
    mut kits: Query<&mut Grapnel>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for u in uses.read() {
        let Ok(mut g) = kits.get_mut(u.item) else {
            continue;
        };
        match *g {
            Grapnel::Coiled => {
                *g = Grapnel::Ready {
                    by: u.player,
                    thrown: false,
                };
                raid.say("You shake out the line and swing the hook. Throw it high.");
            }
            Grapnel::Ready { .. } => {
                *g = Grapnel::Coiled;
                raid.say("You coil the line back up.");
            }
            Grapnel::Hooked { .. } => {}
        }
    }
}

/// A thrown hook catches on whatever it lands on, if that's high enough,
/// and the rope drops from the nearest edge toward whoever threw it.
#[allow(clippy::too_many_arguments)]
fn hooks(
    mut commands: Commands,
    mut kits: Query<(
        Entity,
        &mut Grapnel,
        &Position,
        &LinearVelocity,
        Has<Held>,
        Has<InSocket>,
        Has<InBackpack>,
    )>,
    ropes: Query<&Rope>,
    bodies: Query<&GlobalTransform>,
    spatial: SpatialQuery,
    zone: Res<Zone>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    for (e, mut g, pos, vel, held, socketed, stowed) in &mut kits {
        match *g {
            Grapnel::Coiled => {}
            Grapnel::Hooked { rope } => {
                // Picked back up (or the rope is gone): coil it.
                if held || socketed || stowed || !ropes.contains(rope) {
                    commands.entity(rope).try_despawn();
                    commands
                        .entity(e)
                        .remove::<(RigidBodyDisabled, ColliderDisabled)>();
                    *g = Grapnel::Coiled;
                }
            }
            Grapnel::Ready { by, thrown } => {
                if socketed || stowed {
                    *g = Grapnel::Coiled;
                    continue;
                }
                if held {
                    continue;
                }
                if !thrown {
                    *g = Grapnel::Ready { by, thrown: true };
                }
                let p = pos.0;
                if p.y - zone.ground(p.x, p.z) < MIN_HEIGHT || vel.0.y > 0.5 {
                    // Still rising, or too low: if it has come to rest down
                    // here, it didn't catch.
                    if vel.0.length() < 0.05 && thrown {
                        *g = Grapnel::Coiled;
                        raid.say("The hook doesn't catch. Coil it and try again.");
                    }
                    continue;
                }
                let Some(hit) = spatial.cast_ray(p, Dir3::NEG_Y, 0.35, true, &world) else {
                    continue;
                };
                let bite = p - Vec3::Y * hit.distance;
                let toward = bodies
                    .get(by)
                    .map(|b| b.translation() - bite)
                    .unwrap_or(Vec3::NEG_Z);
                let Some((top, bottom)) = find_ledge(&spatial, &zone, bite, toward) else {
                    *g = Grapnel::Coiled;
                    raid.say("The hook catches, but there's no edge to hang the rope from.");
                    continue;
                };
                let rope = commands
                    .spawn((
                        Name::new("Rope"),
                        Rope {
                            anchor: bite,
                            top,
                            bottom,
                            kit: e,
                            zip: false,
                        },
                        crate::interact::Grabbable {
                            points: vec![crate::interact::GrabPoint::free()],
                        },
                        Transform::from_translation((top + bottom) * 0.5),
                        Visibility::default(),
                    ))
                    .with_child((
                        // For the hands to find: a thin column along the rope.
                        Collider::capsule(0.05, (top.y - bottom.y - 0.1).max(0.1)),
                        CollisionLayers::new(Layer::Item, [Layer::Hand]),
                        Sensor,
                        Transform::from_rotation(Quat::from_rotation_arc(
                            Vec3::Y,
                            (top - bottom).normalize_or(Vec3::Y),
                        )),
                    ))
                    .id();
                // The rope's collider wants a body-less static parent.
                commands.entity(rope).insert(RigidBody::Static);
                commands
                    .entity(e)
                    .insert((RigidBodyDisabled, Position(bite + Vec3::Y * 0.05)));
                *g = Grapnel::Hooked { rope };
                raid.say("The hook bites. The rope hangs ready.");
            }
        }
    }
}

/// Use a held zipline kit: the cable shoots where you're looking and, if
/// it bites somewhere sensible, runs there from a post at your feet.
#[allow(clippy::too_many_arguments)]
fn rig_ziplines(
    mut commands: Commands,
    mut uses: MessageReader<UseItem>,
    mut kits: Query<&mut Zipline>,
    heads: Query<(&GlobalTransform, &ChildOf), With<crate::rig::RigHead>>,
    feet: Query<&GlobalTransform, With<Player>>,
    spatial: SpatialQuery,
    zone: Res<Zone>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for u in uses.read() {
        let Ok(mut z) = kits.get_mut(u.item) else {
            continue;
        };
        if *z != Zipline::Packed {
            continue;
        }
        let Some((head, _)) = heads.iter().find(|(_, p)| p.parent() == u.player) else {
            continue;
        };
        let Ok(body) = feet.get(u.player) else {
            continue;
        };
        let own = SpatialQueryFilter::from_mask([Layer::World]);
        let Some(hit) =
            spatial.cast_ray(head.translation(), head.forward(), ZIP_RANGE.1, true, &own)
        else {
            raid.say("The cable falls short. Nothing out there to bite on.");
            continue;
        };
        let end = head.translation() + head.forward().as_vec3() * (hit.distance - 0.2);
        let base = body.translation();
        let start = Vec3::new(
            base.x,
            base.y.max(zone.ground(base.x, base.z)) + 2.0,
            base.z,
        );
        let run = end - start;
        let flat = Vec2::new(run.x, run.z).length();
        if flat < ZIP_RANGE.0 {
            raid.say(format!("Only {flat:.0} m: too close to be worth a cable."));
            debug!("zipline: start {start:.1}, end {end:.1}");
            continue;
        }
        if run.y > -1.0 || -run.y > flat {
            raid.say("The cable has to run downhill, and not too steeply.");
            continue;
        }
        let cable = commands
            .spawn((
                Name::new("Zipline"),
                Rope {
                    anchor: start,
                    top: start,
                    bottom: end,
                    kit: u.item,
                    zip: true,
                },
                crate::interact::Grabbable {
                    points: vec![crate::interact::GrabPoint::free()],
                },
                Transform::from_translation((start + end) * 0.5),
                Visibility::default(),
                RigidBody::Static,
            ))
            .with_child((
                Collider::capsule(0.06, (run.length() - 0.2).max(0.1)),
                CollisionLayers::new(Layer::Item, [Layer::Hand]),
                Sensor,
                Transform::from_rotation(Quat::from_rotation_arc(Vec3::Y, run.normalize())),
            ))
            .id();
        // Let go of the kit and it sits on its post at the top.
        commands.entity(u.item).insert(ZipPost(start));
        *z = Zipline::Rigged { cable, set: false };
        raid.say(format!(
            "The cable whips out and bites, {:.0} m off. Grab it and ride.",
            run.length()
        ));
    }
}

/// A rigged kit's post: it sits there until someone takes it.
#[derive(Component, Debug, Clone, Copy)]
pub struct ZipPost(pub Vec3);

/// A rigged kit stays on its post; picked up, it reels the cable in.
fn pack_ziplines(
    mut commands: Commands,
    mut kits: Query<(
        Entity,
        &mut Zipline,
        Option<&ZipPost>,
        &mut Position,
        &mut LinearVelocity,
        Has<Held>,
        Has<InSocket>,
        Has<InBackpack>,
    )>,
    ropes: Query<(), With<Rope>>,
) {
    for (e, mut z, post, mut pos, mut vel, held, socketed, stowed) in &mut kits {
        let Zipline::Rigged { cable, set } = *z else {
            continue;
        };
        if (held && set) || socketed || stowed || !ropes.contains(cable) {
            commands.entity(cable).try_despawn();
            commands.entity(e).remove::<ZipPost>();
            *z = Zipline::Packed;
        } else if !held {
            if let Some(p) = post {
                pos.0 = p.0 - Vec3::Y * 0.1;
                vel.0 = Vec3::ZERO;
            }
            *z = Zipline::Rigged { cable, set: true };
        }
    }
}

/// From where the hook bit, walk toward the thrower until the surface
/// drops away: that's the edge. Returns the rope's top (on the edge) and
/// bottom (on the ground below it).
fn find_ledge(
    spatial: &SpatialQuery,
    zone: &Zone,
    bite: Vec3,
    toward: Vec3,
) -> Option<(Vec3, Vec3)> {
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    let d = Vec3::new(toward.x, 0.0, toward.z).normalize_or(Vec3::NEG_Z);
    let mut surface = bite;
    let mut s = 0.2;
    while s < LEDGE_SEARCH {
        let q = Vec3::new(bite.x, surface.y, bite.z) + d * s;
        // Anything in the way at shoulder height ends the walk (a wall
        // above the roof line: no edge this way).
        let probe = q + Vec3::Y * 0.4;
        match spatial.cast_ray(probe, Dir3::NEG_Y, 1.4, true, &world) {
            Some(hit) if hit.distance > 0.05 => {
                surface = probe - Vec3::Y * hit.distance;
            }
            Some(_) => return None,
            None => {
                // The edge: hang the rope just past it.
                let top = Vec3::new(q.x, surface.y, q.z);
                let over = top + d * 0.15;
                let ground = zone.ground(over.x, over.z);
                let floor = spatial
                    .cast_ray(over - Vec3::Y * 0.2, Dir3::NEG_Y, 40.0, true, &world)
                    .map(|h| over.y - 0.2 - h.distance)
                    .unwrap_or(ground)
                    .max(ground);
                if top.y - floor < 1.5 {
                    return None;
                }
                return Some((over, Vec3::new(over.x, floor, over.z)));
            }
        }
        s += 0.2;
    }
    None
}

/// Hand over hand: the newest grip stays where it took hold of the rope,
/// so pulling a hand down lifts the body. The stick slides the grip along
/// the rope instead. At the top you haul yourself over the edge.
#[allow(clippy::type_complexity)]
fn climb(
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &RigInput,
            &mut Body,
            &mut Transform,
            &Children,
            Has<Climbing>,
        ),
        With<Player>,
    >,
    mut hands: Query<(Entity, &RigHand, &Hand, Option<&mut RopeGrip>)>,
    ropes: Query<&Rope>,
    hand_holds: Query<(), With<crate::interact::hand_climb::HandHold>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (player, input, mut body, mut root, children, climbing) in &mut players {
        let mut best: Option<(Entity, f32)> = None;
        for c in children.iter() {
            if let Ok((e, _, _, Some(grip))) = hands.get(c) {
                if !ropes.contains(grip.rope) {
                    commands.entity(e).remove::<RopeGrip>();
                    continue;
                }
                if best.is_none_or(|(_, s)| grip.since > s) {
                    best = Some((e, grip.since));
                }
            }
        }
        let Some((hand_e, _)) = best else {
            // A ladder or a ledge may hold you instead (`interact::hand_climb`).
            if climbing && !children.iter().any(|c| hand_holds.contains(c)) {
                commands.entity(player).remove::<Climbing>();
            }
            continue;
        };
        if !climbing {
            commands.entity(player).insert(Climbing);
        }
        let Ok((_, _, hand, Some(mut grip))) = hands.get_mut(hand_e) else {
            continue;
        };
        let Ok(rope) = ropes.get(grip.rope) else {
            continue;
        };
        if rope.zip {
            // Downhill under gravity, against a little drag, to the end.
            let run = rope.bottom - rope.top;
            let slope = (-run.y / run.length().max(1e-3)).max(0.0);
            grip.speed = (grip.speed + (9.81 * slope - 0.04 * grip.speed * grip.speed) * dt)
                .clamp(0.0, ZIP_SPEED);
            grip.at = rope.nearest(grip.at + run.normalize_or_zero() * grip.speed * dt);
            if grip.at.distance(rope.bottom) < 1.2 {
                // Off the end.
                for c in children.iter() {
                    commands.entity(c).remove::<RopeGrip>();
                }
                commands.entity(player).remove::<Climbing>();
                continue;
            }
            let pull = (grip.at - hand.world.translation) / dt;
            body.velocity = pull.clamp_length_max(ZIP_SPEED * 1.5);
            body.airborne = 0.0;
            continue;
        }
        let stick = input.locomotion.y;
        if stick.abs() > 0.2 {
            grip.at = rope.nearest(grip.at + Vec3::Y * stick * CLIMB_SPEED * dt);
        }
        // At the top with your chest at the edge: over you go.
        if grip.at.y > rope.top.y - 0.15 && root.translation.y > rope.top.y - 1.5 {
            let inward = Vec3::new(rope.anchor.x - rope.top.x, 0.0, rope.anchor.z - rope.top.z)
                .normalize_or_zero();
            root.translation = rope.top + inward * 0.7 + Vec3::Y * 0.05;
            body.velocity = Vec3::ZERO;
            for c in children.iter() {
                commands.entity(c).remove::<RopeGrip>();
            }
            commands.entity(player).remove::<Climbing>();
            continue;
        }
        let pull = (grip.at - hand.world.translation) / dt;
        body.velocity = pull.clamp_length_max(MAX_PULL);
        body.airborne = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rope_nearest_clamps_to_its_ends() {
        let r = Rope {
            anchor: Vec3::new(0.0, 5.0, -1.0),
            top: Vec3::new(0.0, 5.0, 0.0),
            bottom: Vec3::ZERO,
            kit: Entity::PLACEHOLDER,
            zip: false,
        };
        assert_eq!(
            r.nearest(Vec3::new(1.0, 2.0, 0.0)),
            Vec3::new(0.0, 2.0, 0.0)
        );
        assert_eq!(r.nearest(Vec3::new(0.0, 9.0, 0.0)), r.top);
        assert_eq!(r.nearest(Vec3::new(0.0, -3.0, 0.0)), r.bottom);
    }

    #[test]
    fn hook_a_roof_and_climb_onto_it() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        // The shop: a flat roof well above the street.
        let (centre, rot, prefab) = app
            .world_mut()
            .query::<(&crate::world::buildings::Building, &GlobalTransform)>()
            .iter(app.world())
            .find(|(b, _)| b.prefab == "shop")
            .map(|(b, g)| (g.translation(), g.rotation(), b.prefab.clone()))
            .expect("a shop");
        let foot = extraction_core::map::prefab_footprint(&prefab);
        let roof = {
            let mut q = bevy::ecs::system::SystemState::<SpatialQuery>::new(app.world_mut());
            let spatial = q.get(app.world()).unwrap();
            let from = centre + Vec3::Y * 30.0;
            let hit = spatial
                .cast_ray(
                    from,
                    Dir3::NEG_Y,
                    40.0,
                    true,
                    &SpatialQueryFilter::from_mask([Layer::World]),
                )
                .expect("a roof");
            from - Vec3::Y * hit.distance
        };
        let ground = zone.ground(centre.x, centre.z);
        assert!(roof.y - ground > 2.5, "the shop is tall enough");
        // Stand off the front, out in the street.
        let out = rot * Vec3::NEG_Z;
        let stand = zone.on_ground(
            centre.x + out.x * (foot[2] * 0.5 + 3.0),
            centre.z + out.z * (foot[2] * 0.5 + 3.0),
        );
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = stand;
        app.update();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let kit = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind("climbing_kit").unwrap(),
                    transform: Transform::from_translation(roof + Vec3::Y * 0.4),
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.update();
        *app.world_mut().get_mut::<Grapnel>(kit).unwrap() = Grapnel::Ready {
            by: player,
            thrown: true,
        };
        let mut rope = None;
        for _ in 0..90 * 3 {
            app.update();
            if let Grapnel::Hooked { rope: r } = *app.world().get::<Grapnel>(kit).unwrap() {
                rope = Some(r);
                break;
            }
        }
        let rope = rope.expect("the hook caught");
        let r = *app.world().get::<Rope>(rope).unwrap();
        assert!(r.top.y - r.bottom.y > 2.0, "a rope to climb: {r:?}");
        // Walk to its foot, take hold and climb.
        let foot_at = r.bottom + (stand - r.bottom).with_y(0.0).normalize_or_zero() * 0.35;
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = foot_at;
        app.update();
        app.world_mut()
            .write_message(crate::interact::HandAction::Grab {
                player,
                side: crate::rig::Side::Right,
                item: rope,
                point: None,
            });
        app.world_mut()
            .get_mut::<RigInput>(player)
            .unwrap()
            .locomotion = Vec2::Y;
        let mut climbed = false;
        for _ in 0..90 * 12 {
            app.update();
            app.world_mut()
                .get_mut::<RigInput>(player)
                .unwrap()
                .locomotion = Vec2::Y;
            if app.world().get::<Climbing>(player).is_none()
                && app.world().get::<Transform>(player).unwrap().translation.y > roof.y - 0.3
            {
                climbed = true;
                break;
            }
        }
        let y = app.world().get::<Transform>(player).unwrap().translation.y;
        assert!(
            climbed,
            "over the edge and onto the roof (at {y:.2}, roof {:.2})",
            roof.y
        );
        for _ in 0..90 {
            app.update();
        }
        let y = app.world().get::<Transform>(player).unwrap().translation.y;
        assert!(y > roof.y - 0.5, "and stays up there ({y:.2})");

        // Now rig a zipline off the roof, out over the street, and ride it.
        let zip = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind("zipline_kit").unwrap(),
                    transform: Transform::from_translation(roof + Vec3::Y * 0.3),
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        let start = app.world().get::<Transform>(player).unwrap().translation;
        {
            // Face out the front, looking down at the street 30 m off.
            let yaw = out.x.atan2(out.z) + std::f32::consts::PI;
            let mut t = app.world_mut().get_mut::<Transform>(player).unwrap();
            t.rotation = Quat::from_rotation_y(yaw);
        }
        let drop = start.y + 1.6 - zone.ground(start.x + out.x * 30.0, start.z + out.z * 30.0);
        let pitch = -(drop / 30.0).atan();
        app.world_mut()
            .get_mut::<RigInput>(player)
            .unwrap()
            .locomotion = Vec2::ZERO;
        app.world_mut().get_mut::<RigInput>(player).unwrap().head =
            Transform::from_xyz(0.0, 1.6, 0.0).with_rotation(Quat::from_rotation_x(pitch));
        for _ in 0..3 {
            app.update();
        }
        app.world_mut().write_message(UseItem { player, item: zip });
        for _ in 0..5 {
            app.update();
        }
        let Zipline::Rigged { cable, .. } = *app.world().get::<Zipline>(zip).unwrap() else {
            panic!(
                "the zipline rigged: {:?}",
                app.world().resource::<crate::raid::Raid>().log.last()
            );
        };
        app.world_mut()
            .write_message(crate::interact::HandAction::Grab {
                player,
                side: crate::rig::Side::Right,
                item: cable,
                point: None,
            });
        let mut rode = false;
        for _ in 0..90 * 15 {
            app.update();
            let p = app.world().get::<Transform>(player).unwrap().translation;
            if Vec2::new(p.x - start.x, p.z - start.z).length() > 15.0 {
                rode = true;
            }
            if rode && app.world().get::<Climbing>(player).is_none() {
                break;
            }
        }
        let p = app.world().get::<Transform>(player).unwrap().translation;
        assert!(rode, "rode the line out ({p:.1} from {start:.1})");
        for _ in 0..90 * 2 {
            app.update();
        }
        let health = app.world().get::<crate::health::Health>(player).unwrap();
        assert!(health.hp > 50.0, "and landed in one piece ({})", health.hp);
    }
}
