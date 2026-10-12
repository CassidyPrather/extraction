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
const STEP_HEIGHT: f32 = 0.4;

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
            .add_systems(FixedUpdate, move_bodies);
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

fn capsule() -> Collider {
    Collider::capsule(CAPSULE_RADIUS, CAPSULE_HEIGHT - 2.0 * CAPSULE_RADIUS)
}

fn move_bodies(
    mut bodies: Query<(Entity, &RigInput, &mut Body, &mut Transform, &Children), With<Player>>,
    colliders: Query<Entity, With<BodyCollider>>,
    move_and_slide: MoveAndSlide,
    time: Res<Time>,
    mut steps: MessageWriter<Footstep>,
) {
    let dt = time.delta_secs();
    let shape = capsule();
    let config = MoveAndSlideConfig::default();
    for (entity, input, mut body, mut root, children) in &mut bodies {
        let own: Vec<Entity> = children.iter().filter(|c| colliders.contains(*c)).collect();
        let filter = SpatialQueryFilter::from_mask([Layer::World, Layer::Robot])
            .with_excluded_entities(own.iter().copied().chain([entity]));
        let enc = Encumbrance::from_mass(body.carried_kg);

        // Desired horizontal velocity from the head's yaw.
        let head_fwd = (root.rotation * input.head.rotation) * Vec3::NEG_Z;
        let fwd = Vec3::new(head_fwd.x, 0.0, head_fwd.z).normalize_or(Vec3::NEG_Z);
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        let want_sprint =
            input.sprint && enc.can_sprint && body.stamina > 0.05 && input.locomotion.y > 0.3;
        let speed = if input.crouch {
            CROUCH_SPEED
        } else if want_sprint {
            SPRINT_SPEED
        } else {
            WALK_SPEED
        } * enc.speed;
        let target = (fwd * input.locomotion.y + right * input.locomotion.x) * speed;
        // Responsive but not instant: a little weight to starting and stopping.
        let accel = if body.grounded { 12.0 } else { 2.0 };
        let horiz = Vec3::new(body.velocity.x, 0.0, body.velocity.z);
        let horiz = horiz.lerp(target, (accel * dt).min(1.0));
        body.velocity.x = horiz.x;
        body.velocity.z = horiz.z;

        if want_sprint && horiz.length() > WALK_SPEED {
            body.stamina = (body.stamina - dt / 12.0 * enc.stamina_drain).max(0.0);
        } else {
            body.stamina = (body.stamina + dt / 8.0 * enc.stamina_regen).min(1.0);
        }

        if input.jump && (body.grounded || body.airborne < 0.15) {
            body.velocity.y = JUMP_SPEED;
            body.grounded = false;
            body.airborne = 1.0;
        }
        body.velocity.y -= GRAVITY * dt;

        root.rotate_y(input.turn);

        let centre = root.translation + Vec3::Y * CAPSULE_HEIGHT * 0.5;
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
        if !grounded && body.grounded && body.velocity.y <= 0.0 {
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

        if grounded {
            body.airborne = 0.0;
            velocity.y = velocity.y.max(-0.5);
        } else {
            body.airborne += dt;
        }
        body.grounded = grounded;
        body.velocity = velocity;
        let moved = Vec3::new(new_centre.x - centre.x, 0.0, new_centre.z - centre.z).length();
        root.translation = new_centre - Vec3::Y * CAPSULE_HEIGHT * 0.5;

        if grounded {
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
    if ms
        .spatial_query
        .cast_shape(
            shape,
            raised,
            Quat::IDENTITY,
            dir,
            &ShapeCastConfig::from_max_distance(delta.length()),
            filter,
        )
        .is_some()
    {
        return None;
    }
    let moved = raised + delta;
    let hit = ms.spatial_query.cast_shape(
        shape,
        moved,
        Quat::IDENTITY,
        Dir3::NEG_Y,
        &ShapeCastConfig::from_max_distance(STEP_HEIGHT + 0.05),
        filter,
    )?;
    (hit.normal1.y > 0.65).then(|| moved - Vec3::Y * (hit.distance - 0.01).max(0.0))
}
