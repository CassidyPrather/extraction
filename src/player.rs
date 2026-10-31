//! The player's body: a kinematic capsule that walks the rig around.
//!
//! The root entity sits at the feet and is the playspace origin. Its yaw is
//! the body's facing; locomotion is relative to the head's yaw, so in VR
//! you walk where you look. The capsule is a child collider so robots and
//! bullets can hit it; movement is move-and-slide with gravity and a
//! simple step-up for kerbs and stairs.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::inventory::Encumbrance;

use crate::launch::LaunchOptions;
use crate::rig::{LocalPlayer, RigHand, RigHead, RigInput, Side};
use crate::world::{Layer, Zone};
use extraction_core::map::MarkerKind;

/// Walking pace, m/s. VR-comfortable rather than shooter-fast.
pub const WALK_SPEED: f32 = 2.4;
pub const SPRINT_SPEED: f32 = 4.6;
pub const CROUCH_SPEED: f32 = 1.3;
const GRAVITY: f32 = 9.81;
const JUMP_SPEED: f32 = 3.6;
const CAPSULE_RADIUS: f32 = 0.28;
const CAPSULE_HEIGHT: f32 = 1.75;
/// The lowest the body goes: crouched as low as a kneel.
const MIN_HEIGHT: f32 = 0.9;
const STEP_HEIGHT: f32 = 0.4;
/// Deeper water than this and you float, head and shoulders out.
pub const FLOAT_DEPTH: f32 = 1.3;
/// Swimming pace with a kit on your back: a slow paddle.
const SWIM_SPEED: f32 = 0.8;

#[derive(Component)]
pub struct Player;

/// Kinematic state the controller carries between ticks.
#[derive(Component, Default, Debug)]
pub struct Body {
    pub velocity: Vec3,
    pub grounded: bool,
    /// Seconds since last on the ground (for coyote-time jumps).
    pub airborne: f32,
    /// 0..=1 sprint stamina.
    pub stamina: f32,
    /// Carried mass, kg, set by the inventory system.
    pub carried_kg: f32,
    /// Distance walked since the last footstep, for footstep sounds.
    pub stride: f32,
    /// Downward speed at the last landing, m/s (health reads and clears it).
    pub impact: f32,
    /// How deep the water is around the feet, m (0 on dry land).
    pub wading: f32,
    /// Afloat in deep water.
    pub swimming: bool,
    /// Where the feet stand in the playspace (x, z). Room-scale: they
    /// follow the head when it walks off, not when it leans; see
    /// `follow_feet`.
    pub feet: Option<Vec2>,
    /// The head's standing height in the playspace, held at its highest
    /// and slowly let go (so a bend reads as a bend).
    pub standing: f32,
    /// How tall the body is right now (m; 0 before the first tick): it
    /// follows the head down at once, and back up only where there's room
    /// above. What moves you and what gets shot are both this tall, so you
    /// can duck under a beam, a fallen trunk or a low doorway for real.
    pub height: f32,
    /// How much slower a bulky thing in the hands makes you (0: not at
    /// all; `interact::carry`). You can't run with one.
    pub hauling: f32,
    /// How much slower a broken leg makes you (0: not at all;
    /// `interact::wounds`). You can't run on one.
    pub limping: f32,
    /// How much slower a frost's chill makes you (0: not at all;
    /// `anomaly::frost`). Stiff with it, you can't run.
    pub chilled: f32,
}

/// The capsule collider entity (child of the root).
#[derive(Component)]
pub struct BodyCollider;

/// Fired every stride while walking on the ground.
#[derive(Message, Clone, Debug)]
pub struct Footstep {
    pub entity: Entity,
    pub position: Vec3,
    pub speed: f32,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Footstep>()
            .add_systems(Startup, spawn_local_player)
            .add_systems(FixedUpdate, (move_bodies, fit_hitboxes));
    }
}

fn spawn_local_player(mut commands: Commands, zone: Res<Zone>, opts: Res<LaunchOptions>) {
    let (pos, yaw) = match opts.camera {
        Some((p, yaw, _)) => (Vec3::new(p.x, zone.ground(p.x, p.z), p.z), yaw),
        None => {
            let m = zone.marker(&MarkerKind::Insertion);
            let at = m.map(|m| m.at).unwrap_or([0.0, 0.0]);
            (
                zone.on_ground(at[0], at[1]),
                m.map(|m| m.rot).unwrap_or(0.0),
            )
        }
    };
    info!(
        "spawning local player at {pos} (ground {:.2})",
        zone.ground(pos.x, pos.z)
    );
    spawn_player(&mut commands, pos + Vec3::Y * 0.05, yaw, true);
}

/// Spawns a player rig (root, collider, head, hands) and returns the root.
pub fn spawn_player(commands: &mut Commands, feet: Vec3, yaw_deg: f32, local: bool) -> Entity {
    let mut root = commands.spawn((
        Name::new("Player"),
        Player,
        RigInput::default(),
        Body {
            stamina: 1.0,
            ..default()
        },
        crate::weapons::flat::GunMacro::default(),
        Transform::from_translation(feet)
            .with_rotation(Quat::from_rotation_y(-yaw_deg.to_radians())),
        Visibility::default(),
    ));
    if local {
        root.insert(LocalPlayer);
    }
    root.with_children(|c| {
        c.spawn((
            Name::new("Body collider"),
            BodyCollider,
            HitboxHeight(CAPSULE_HEIGHT),
            Transform::from_xyz(0.0, CAPSULE_HEIGHT * 0.5, 0.0),
            RigidBody::Kinematic,
            Collider::capsule(CAPSULE_RADIUS, CAPSULE_HEIGHT - 2.0 * CAPSULE_RADIUS),
            CollisionLayers::new(Layer::Player, [Layer::World, Layer::Robot, Layer::Item]),
        ));
        c.spawn((
            Name::new("Head"),
            RigHead,
            Transform::from_xyz(0.0, 1.68, 0.0),
            Visibility::default(),
        ));
        for side in Side::BOTH {
            c.spawn((
                Name::new(format!("{side:?} hand")),
                RigHand { side },
                crate::interact::Hand::default(),
                Transform::default(),
                Visibility::default(),
            ));
        }
    });
    root.id()
}

fn capsule(height: f32) -> Collider {
    Collider::capsule(CAPSULE_RADIUS, height - 2.0 * CAPSULE_RADIUS)
}

/// How tall the body can be this tick, standing on `base`: as tall as the
/// head wants (a little over its eyes), but rising only into room there is.
fn body_height(
    ms: &MoveAndSlide,
    body: &Body,
    head_y: f32,
    base: Vec3,
    filter: &SpatialQueryFilter,
) -> f32 {
    let want = (head_y + 0.12).clamp(MIN_HEIGHT, CAPSULE_HEIGHT);
    let was = if body.height > 0.0 {
        body.height
    } else {
        return want;
    };
    if want <= was + 1e-3 {
        return want;
    }
    let room = ms
        .spatial_query
        .cast_shape(
            &capsule(was),
            base + Vec3::Y * was * 0.5,
            Quat::IDENTITY,
            Dir3::Y,
            &ShapeCastConfig::from_max_distance(want - was),
            filter,
        )
        .map_or(want - was, |hit| (hit.distance - 0.01).max(0.0));
    was + room
}

pub fn move_bodies(
    mut bodies: Query<
        (
            Entity,
            &RigInput,
            &mut Body,
            &mut Transform,
            &Children,
            Has<crate::gadgets::climbing::Climbing>,
        ),
        With<Player>,
    >,
    mut colliders: Query<&mut Transform, (With<BodyCollider>, Without<Player>)>,
    move_and_slide: MoveAndSlide,
    time: Res<Time>,
    zone: Res<Zone>,
    levels: Option<Res<crate::world::underground::Levels>>,
    mut steps: MessageWriter<Footstep>,
) {
    let dt = time.delta_secs();
    let config = MoveAndSlideConfig::default();
    for (entity, input, mut body, mut root, children, climbing) in &mut bodies {
        let own: Vec<Entity> = children.iter().filter(|c| colliders.contains(*c)).collect();
        let filter = SpatialQueryFilter::from_mask([Layer::World, Layer::Robot])
            .with_excluded_entities(own.iter().copied().chain([entity]));
        let enc = Encumbrance::from_mass(body.carried_kg);
        // Room-scale: you stand where your feet are (under your head,
        // unless you're leaning). The capsule goes there, and moves from
        // there.
        let feet_before = body.feet;
        let feet_local = follow_feet(&mut body, &input.head, dt);
        let under = root.rotation * Vec3::new(feet_local.x, 0.0, feet_local.y);
        for c in &own {
            if let Ok(mut t) = colliders.get_mut(*c) {
                let local = Vec3::new(input.head.translation.x, 0.0, input.head.translation.z);
                if t.translation.xz() != local.xz() {
                    t.translation.x = local.x;
                    t.translation.z = local.z;
                }
            }
        }
        // Water: wading drags at you; deep enough and you float.
        let feet = root.translation + under;
        // (Underground, a site's own floods; the ground's water is above.)
        let water = crate::world::underground::site_water(&zone, levels.as_deref(), feet)
            .unwrap_or_else(|| zone.hm.water_level([feet.x, feet.z]));
        let depth = water.map_or(0.0, |w| (w - feet.y).max(0.0));
        body.wading = depth;
        let swimming = !climbing && depth > FLOAT_DEPTH - 0.05;
        body.swimming = swimming;
        let drag = 1.0 - (depth / FLOAT_DEPTH).clamp(0.0, 1.0) * 0.6;

        // Desired horizontal velocity from the head's yaw.
        let head_fwd = (root.rotation * input.head.rotation) * Vec3::NEG_Z;
        let fwd = Vec3::new(head_fwd.x, 0.0, head_fwd.z).normalize_or(Vec3::NEG_Z);
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        let want_sprint = input.sprint
            && enc.can_sprint
            && body.hauling <= 0.0
            && body.limping < crate::interact::wounds::LIMP
            && body.chilled < crate::anomaly::frost::STIFF
            && body.stamina > 0.05
            && input.locomotion.y > 0.3
            && depth < 0.5;
        let speed = if swimming {
            SWIM_SPEED
        } else if input.crouch {
            CROUCH_SPEED
        } else if want_sprint {
            SPRINT_SPEED
        } else {
            WALK_SPEED
        } * enc.speed
            * (1.0 - body.hauling)
            * (1.0 - body.limping)
            * (1.0 - body.chilled)
            * if swimming { 1.0 } else { drag };
        let target = if climbing {
            // On a rope the stick climbs (see `gadgets::climbing`).
            Vec3::ZERO
        } else {
            (fwd * input.locomotion.y + right * input.locomotion.x) * speed
        };
        // Responsive but not instant: a little weight to starting and stopping.
        let accel = if swimming {
            3.0
        } else if body.grounded {
            12.0
        } else {
            2.0
        };
        let horiz = Vec3::new(body.velocity.x, 0.0, body.velocity.z);
        let horiz = if climbing {
            horiz
        } else {
            horiz.lerp(target, (accel * dt).min(1.0))
        };
        body.velocity.x = horiz.x;
        body.velocity.z = horiz.z;

        if want_sprint && horiz.length() > WALK_SPEED {
            body.stamina = (body.stamina - dt / 12.0 * enc.stamina_drain).max(0.0);
        } else {
            body.stamina = (body.stamina + dt / 8.0 * enc.stamina_regen).min(1.0);
        }

        if climbing {
            // The rope holds you up; `climb` set the velocity.
        } else if let (true, Some(w)) = (swimming, water) {
            // Afloat: bob toward riding with the feet FLOAT_DEPTH under the
            // surface. No jumping off water.
            let want = w - FLOAT_DEPTH;
            body.velocity.y = ((want - feet.y) * 3.0).clamp(-2.0, 2.0);
        } else {
            if input.jump && (body.grounded || body.airborne < 0.15) {
                body.velocity.y = JUMP_SPEED;
                body.grounded = false;
                body.airborne = 1.0;
            }
            body.velocity.y -= GRAVITY * dt;
        }

        root.rotate_y(input.turn);
        let under = root.rotation * Vec3::new(feet_local.x, 0.0, feet_local.y);
        let under_before = feet_before.map_or(under, |f| root.rotation * Vec3::new(f.x, 0.0, f.y));

        // Room-scale: where the feet walked in the room, the capsule walks
        // too (stepping up a step, stopping at a wall) rather than
        // appearing there. (Appearing inside a step's box, it was pushed
        // back out: walking up a house's steps in the room pushed the room
        // away instead.)
        let height = body_height(
            &move_and_slide,
            &body,
            input.head.translation.y,
            root.translation + under_before,
            &filter,
        );
        body.height = height;
        let shape = capsule(height);
        let mut centre = root.translation + under_before + Vec3::Y * height * 0.5;
        let start = centre;
        let mut under = under;
        let walked = under - under_before;
        if !climbing && walked.length_squared() > 1e-8 {
            let out = move_and_slide.move_and_slide(
                &shape,
                centre,
                Quat::IDENTITY,
                walked / dt.max(1e-4),
                time.delta(),
                &config,
                &filter,
                |_| MoveAndSlideHitResponse::Accept,
            );
            centre = match walk_over(&move_and_slide, &shape, centre, walked, &filter) {
                // Ground you can stand on (level, a slope): exactly where
                // the feet went.
                Some(on) => on,
                // A step's edge or a wall: slide, and step up if it's a
                // step.
                None => {
                    let mut reached = out.position;
                    let got = Vec3::new(reached.x - centre.x, 0.0, reached.z - centre.z);
                    if got.length() < walked.length() * 0.5 {
                        let up = centre + Vec3::Y * STEP_HEIGHT;
                        if let Some(s) = try_step(&move_and_slide, &shape, up, walked, &filter) {
                            reached = s;
                        }
                    }
                    reached
                }
            };
            // The feet got as far as the capsule did, no further. Where
            // they were stopped (a bench leant over, a wall), the room stays
            // where it is and the head goes on into it (`render::blink`
            // shuts the eyes inside); pushing the room back to keep the
            // head out moved you for leaning (the 28 October playtest).
            let got = Vec3::new(centre.x - start.x, 0.0, centre.z - start.z);
            if got.distance_squared(walked) > 1e-8 {
                under = under_before + got;
                let local = root.rotation.inverse() * under;
                body.feet = Some(Vec2::new(local.x, local.z));
            }
        }
        let mut grounded = false;
        let out = move_and_slide.move_and_slide(
            &shape,
            centre,
            Quat::IDENTITY,
            body.velocity,
            time.delta(),
            &config,
            &filter,
            |hit| {
                if hit.normal.y > 0.65 {
                    grounded = true;
                }
                MoveAndSlideHitResponse::Accept
            },
        );
        let mut new_centre = out.position;
        let mut velocity = out.projected_velocity;

        // Step up: if we were walking on the ground and got stopped short,
        // try the same move from higher up, then settle down onto the step.
        let wanted = Vec3::new(body.velocity.x, 0.0, body.velocity.z) * dt;
        let got = Vec3::new(new_centre.x - centre.x, 0.0, new_centre.z - centre.z);
        if body.grounded && wanted.length() > 1e-4 && got.length() < wanted.length() * 0.5 {
            let up = centre + Vec3::Y * STEP_HEIGHT;
            if let Some(stepped) = try_step(&move_and_slide, &shape, up, wanted, &filter) {
                new_centre = stepped;
                velocity.y = 0.0;
                grounded = true;
            }
        }

        // Stick to the ground going down slopes and stairs.
        if !climbing && !grounded && body.grounded && body.velocity.y <= 0.0 {
            if let Some(hit) = move_and_slide.spatial_query.cast_shape(
                &shape,
                new_centre,
                Quat::IDENTITY,
                Dir3::NEG_Y,
                &ShapeCastConfig::from_max_distance(STEP_HEIGHT),
                &filter,
            ) {
                if hit.normal1.y > 0.65 {
                    new_centre.y -= (hit.distance - 0.01).max(0.0);
                    grounded = true;
                    velocity.y = 0.0;
                }
            }
        }

        // Static friction: standing still on the ground, stay put. (Pushing
        // the capsule out of a slope moves it along the slope's normal, a
        // little downhill every tick.)
        if grounded && body.grounded && target.length() < 1e-3 && horiz.length() < 0.05 {
            new_centre.x = centre.x;
            new_centre.z = centre.z;
            velocity.x = 0.0;
            velocity.z = 0.0;
        }
        if grounded {
            if !body.grounded {
                body.impact = body.impact.max(-body.velocity.y);
            }
            body.airborne = 0.0;
            velocity.y = velocity.y.max(-0.5);
        } else {
            body.airborne += dt;
        }
        body.grounded = grounded;
        body.velocity = velocity;
        let moved = Vec3::new(new_centre.x - centre.x, 0.0, new_centre.z - centre.z).length();
        root.translation = new_centre - under - Vec3::Y * height * 0.5;

        if swimming {
            // Strokes: the water's own sound, slower than steps.
            body.stride += moved;
            if body.stride > 1.2 {
                body.stride = 0.0;
                steps.write(Footstep {
                    entity,
                    position: root.translation,
                    speed: moved / dt.max(1e-4),
                });
            }
        } else if grounded {
            body.stride += moved;
            let stride_len = if want_sprint { 1.5 } else { 0.85 };
            if body.stride > stride_len {
                body.stride = 0.0;
                steps.write(Footstep {
                    entity,
                    position: root.translation,
                    speed: moved / dt.max(1e-4),
                });
            }
        }
    }
}

/// What gets shot is as tall as the body is ([`Body::height`]): duck behind
/// a wall for real and the capsule ducks with you.
fn fit_hitboxes(
    mut commands: Commands,
    players: Query<(&Body, &Children), With<Player>>,
    mut colliders: Query<(Entity, &mut Transform, &mut HitboxHeight), With<BodyCollider>>,
) {
    for (body, children) in &players {
        if body.height <= 0.0 {
            continue;
        }
        let want = body.height;
        for c in children.iter() {
            let Ok((e, mut t, mut h)) = colliders.get_mut(c) else {
                continue;
            };
            if (h.0 - want).abs() < 0.05 {
                continue;
            }
            h.0 = want;
            t.translation.y = want * 0.5;
            commands.entity(e).insert(Collider::capsule(
                CAPSULE_RADIUS,
                want - 2.0 * CAPSULE_RADIUS,
            ));
        }
    }
}

/// The body collider's current height.
#[derive(Component)]
pub struct HitboxHeight(pub f32);

/// How far the head can lean out past the feet before they step after it.
const LEAN: f32 = 0.35;
/// How fast the feet come back under a head that's upright and level, m/s.
const RECENTRE: f32 = 0.25;

/// Room-scale feet: under the head when you stand straight, but staying
/// put when you lean or bend over something (a table, a crate), and
/// stepping after the head when it walks off further than a lean. The
/// movement capsule stands on them. (Stood under the head itself, the
/// capsule leant into a bench with you, caught on its edge, and slid you
/// down the slope you stood on in the first headset session.)
fn follow_feet(body: &mut Body, head: &Transform, dt: f32) -> Vec2 {
    let at = Vec2::new(head.translation.x, head.translation.z);
    body.standing = (body.standing - dt * 0.01).max(head.translation.y);
    let feet = body.feet.get_or_insert(at);
    let off = at - *feet;
    if off.length() > LEAN {
        *feet += off.normalize() * (off.length() - LEAN);
    }
    let level = (head.rotation * Vec3::NEG_Z).y > -0.3;
    let upright = head.translation.y > body.standing - 0.08;
    if level && upright {
        *feet = feet.move_towards(at, RECENTRE * dt);
    }
    *feet
}

/// Where the head stands over the playspace floor, from the root, in the
/// world's axes (zero for flat play and bots, whose heads sit over the
/// root; the walked-off distance in room-scale VR).
pub fn head_offset(root: &Transform, input: &RigInput) -> Vec3 {
    root.rotation * Vec3::new(input.head.translation.x, 0.0, input.head.translation.z)
}

/// Where a player is standing: under their head.
pub fn standing_at(root: &GlobalTransform, input: &RigInput) -> Vec3 {
    root.transform_point(Vec3::new(
        input.head.translation.x,
        0.0,
        input.head.translation.z,
    ))
}

/// Room-scale walking: up a step's height, across by exactly what the feet
/// walked, and down onto ground you can stand on (level or a slope). None
/// when something is in the way or it lands on an edge (a step's: the
/// caller steps up onto it) or the ground falls away further than a step.
fn walk_over(
    ms: &MoveAndSlide,
    shape: &Collider,
    centre: Vec3,
    delta: Vec3,
    filter: &SpatialQueryFilter,
) -> Option<Vec3> {
    let dir = Dir3::new(delta).ok()?;
    let cast = |from: Vec3, dir: Dir3, d: f32| {
        ms.spatial_query.cast_shape(
            shape,
            from,
            Quat::IDENTITY,
            dir,
            &ShapeCastConfig::from_max_distance(d),
            filter,
        )
    };
    if cast(centre, Dir3::Y, STEP_HEIGHT).is_some() {
        return None;
    }
    let raised = centre + Vec3::Y * STEP_HEIGHT;
    if cast(raised, dir, delta.length()).is_some() {
        return None;
    }
    let across = raised + delta;
    let hit = cast(across, Dir3::NEG_Y, STEP_HEIGHT * 2.0)?;
    (hit.normal1.y > 0.65).then(|| across - Vec3::Y * (hit.distance - 0.005).max(0.0))
}

fn try_step(
    ms: &MoveAndSlide,
    shape: &Collider,
    raised: Vec3,
    delta: Vec3,
    filter: &SpatialQueryFilter,
) -> Option<Vec3> {
    // Room above?
    if ms
        .spatial_query
        .cast_shape(
            shape,
            raised - Vec3::Y * STEP_HEIGHT,
            Quat::IDENTITY,
            Dir3::Y,
            &ShapeCastConfig::from_max_distance(STEP_HEIGHT),
            filter,
        )
        .is_some()
    {
        return None;
    }
    let dir = Dir3::new(delta).ok()?;
    let clear = |d: f32| {
        ms.spatial_query
            .cast_shape(
                shape,
                raised,
                Quat::IDENTITY,
                dir,
                &ShapeCastConfig::from_max_distance(d),
                filter,
            )
            .is_none()
    };
    if !clear(delta.length()) {
        return None;
    }
    let down = |from: Vec3| {
        ms.spatial_query.cast_shape(
            shape,
            from,
            Quat::IDENTITY,
            Dir3::NEG_Y,
            &ShapeCastConfig::from_max_distance(STEP_HEIGHT + 0.05),
            filter,
        )
    };
    // Up against a step, a tick's stride is a few millimetres (the blocked
    // move bled the speed off): never enough to carry the capsule over the
    // edge. So look a capsule radius ahead: level ground there, higher
    // than here, is a step, and the capsule steps up onto it.
    let reach = delta.length().max(CAPSULE_RADIUS);
    if clear(reach) {
        let ahead = raised + dir * reach;
        if let Some(top) = down(ahead) {
            let top_y = ahead.y - (top.distance - 0.01).max(0.0);
            if top.normal1.y > 0.65 && top_y > raised.y - STEP_HEIGHT + 0.03 {
                return Some(Vec3::new(ahead.x, top_y, ahead.z));
            }
        }
    }
    let moved = raised + delta;
    let hit = down(moved)?;
    (hit.normal1.y > 0.65).then(|| moved - Vec3::Y * (hit.distance - 0.01).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Room-scale: step off the middle of the playspace and your body
    /// (what bullets hit, what bumps into walls) comes with your head.
    #[test]
    fn the_body_stands_under_the_head() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        // Open ground, away from the truck.
        let at = zone.on_ground(0.0, 200.0);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = at + Vec3::Y * 0.05;
        for _ in 0..30 {
            app.update();
        }
        let before = app.world().get::<Transform>(player).unwrap().translation;
        // The headset reports the head 0.8 m to the right of the middle;
        // keep it there against the flat driver.
        for _ in 0..30 {
            app.world_mut()
                .get_mut::<RigInput>(player)
                .unwrap()
                .head
                .translation = Vec3::new(0.8, 1.6, 0.0);
            app.world_mut().run_schedule(FixedUpdate);
        }
        let collider = app
            .world_mut()
            .query_filtered::<&Transform, With<BodyCollider>>()
            .iter(app.world())
            .next()
            .unwrap()
            .translation;
        assert!(
            (collider.x - 0.8).abs() < 1e-3 && collider.z.abs() < 1e-3,
            "{collider}"
        );
        let after = app.world().get::<Transform>(player).unwrap().translation;
        assert!(
            Vec2::new(after.x - before.x, after.z - before.z).length() < 0.05,
            "the playspace stays put ({before} -> {after})"
        );
        // Crouch for real: what gets shot crouches too.
        for _ in 0..5 {
            app.world_mut()
                .get_mut::<RigInput>(player)
                .unwrap()
                .head
                .translation = Vec3::new(0.8, 1.0, 0.0);
            app.world_mut().run_schedule(FixedUpdate);
        }
        let h = app
            .world_mut()
            .query_filtered::<&HitboxHeight, With<BodyCollider>>()
            .iter(app.world())
            .next()
            .unwrap()
            .0;
        assert!((h - 1.12).abs() < 0.01, "crouched hitbox {h}");
    }

    /// Standing still on a hillside, you stay where you stand.
    #[test]
    fn standing_still_on_a_slope_stays_put() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        // The steepest walkable ground near the south gate.
        let mut at = Vec2::new(40.0, 280.0);
        let mut steepest = 0.0;
        for i in -20..=20 {
            for j in -20..=20 {
                let p = Vec2::new(40.0 + i as f32 * 2.0, 280.0 + j as f32 * 2.0);
                let s = zone.hm.slope_deg([p.x, p.y]);
                if s > steepest && s < 30.0 && zone.hm.road_at([p.x, p.y]).weight == 0 {
                    steepest = s;
                    at = p;
                }
            }
        }
        assert!(steepest > 8.0, "a slope to stand on ({steepest} deg)");
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = zone.on_ground(at.x, at.y) + Vec3::Y * 0.05;
        for _ in 0..20 {
            app.update();
        }
        let before = app.world().get::<Transform>(player).unwrap().translation;
        for _ in 0..120 {
            app.update();
        }
        let after = app.world().get::<Transform>(player).unwrap().translation;
        assert!(
            Vec2::new(after.x - before.x, after.z - before.z).length() < 0.02,
            "standing on a {steepest:.0} deg slope: {before} -> {after}"
        );
    }

    /// Dropped into the canal, you come up and float with your head out;
    /// walked out onto the bank, you're on your feet again.
    #[test]
    fn deep_water_floats_you() {
        let opts = crate::LaunchOptions {
            robots: false,
            map: "cinder_works".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        // The deepest spot across the canal at x = -70.
        let (at, w) = (25..60)
            .filter_map(|z| {
                let p = [-70.0, z as f32];
                zone.hm
                    .water_level(p)
                    .map(|w| (p, w, w - zone.hm.height(p)))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(p, w, _)| (p, w))
            .expect("the canal");
        assert!(w - zone.hm.height(at) > FLOAT_DEPTH, "deep enough to swim");
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = Vec3::new(at[0], zone.hm.height(at) + 0.05, at[1]);
        for _ in 0..90 * 3 {
            app.update();
        }
        let feet = app.world().get::<Transform>(player).unwrap().translation;
        let body = app.world().get::<Body>(player).unwrap();
        assert!(body.swimming, "afloat");
        assert!(
            (feet.y - (w - FLOAT_DEPTH)).abs() < 0.25,
            "riding with the head out: feet {:.2}, water {w:.2}",
            feet.y
        );
        // Out on the bank, on foot.
        let bank = [-100.0, 100.0];
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = Vec3::new(bank[0], zone.hm.height(bank) + 0.05, bank[1]);
        for _ in 0..30 {
            app.update();
        }
        let body = app.world().get::<Body>(player).unwrap();
        assert!(!body.swimming && body.wading < 0.01, "dry land");
    }
}
