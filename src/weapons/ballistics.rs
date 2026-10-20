//! Bullets as fast, simulated projectiles: gravity, drag, swept against the
//! world every tick. Hits become [`Hit`] messages (robots read them for
//! damage), impulses on whatever they strike, and impact effects.
//!
//! Recoil is in [`super::feel`]: the shot kicks the gun in the hands that
//! hold it, and they bring it back.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::noise::Rng;

use crate::world::Layer;

/// A trigger pull that let a shot go. Shotguns fire many projectiles.
#[derive(Message, Clone, Debug)]
pub struct Shot {
    pub gun: Entity,
    pub shooter: Option<Entity>,
    pub origin: Vec3,
    pub direction: Vec3,
    pub speed: f32,
    pub damage: f32,
    pub projectiles: u32,
    pub spread: f32,
    pub recoil: f32,
    pub seed: u64,
}

#[derive(Component)]
pub struct Projectile {
    pub velocity: Vec3,
    pub damage: f32,
    pub shooter: Option<Entity>,
    pub gun: Entity,
    pub life: f32,
    pub mods: ShotMods,
}

/// What an artifact in the gun's adapter does to its shots.
#[derive(Clone, Copy, Debug)]
pub struct ShotMods {
    /// Gravity and drag scale (featherstone: 0, the round flies flat).
    pub fall: f32,
    /// Hearth: hotter rounds.
    pub damage: f32,
    /// Aegis: robots hit lose their senses for this long.
    pub stun: f32,
    /// Glass eye: the round bends toward robots ahead of it.
    pub seek: bool,
}

impl Default for ShotMods {
    fn default() -> Self {
        Self {
            fall: 1.0,
            damage: 1.0,
            stun: 0.0,
            seek: false,
        }
    }
}

fn shot_mods(id: &str) -> ShotMods {
    let mut m = ShotMods::default();
    match id {
        "art_featherstone" => m.fall = 0.0,
        "art_hearth" => m.damage = 1.5,
        "art_aegis" => m.stun = 2.0,
        "art_glasseye" => m.seek = true,
        _ => {}
    }
    m
}

/// A projectile struck something.
#[derive(Message, Clone, Debug)]
pub struct Hit {
    pub entity: Entity,
    pub point: Vec3,
    pub normal: Vec3,
    pub velocity: Vec3,
    pub damage: f32,
    pub shooter: Option<Entity>,
}

/// A muzzle flash's light, fading out.
#[derive(Component)]
pub struct Flash {
    pub life: f32,
}

/// Shots fired and projectiles moved: hit messages are out after this.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BallisticsSystems;

/// Air drag, 1/s: a rough fit so a rifle round loses ~20% by 200 m.
const DRAG: f32 = 0.12;
const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

pub struct BallisticsPlugin;

impl Plugin for BallisticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Shot>()
            .add_message::<Hit>()
            .add_systems(
                FixedUpdate,
                (fire_shots, fly_projectiles, push_hit_bodies)
                    .chain()
                    .in_set(BallisticsSystems)
                    .after(super::triggers),
            )
            .add_systems(Update, fade_flashes);
    }
}

fn fire_shots(
    mut commands: Commands,
    mut shots: MessageReader<Shot>,
    mounted: Query<(&crate::gadgets::MountedOn, &crate::items::Item)>,
    catalog: Res<crate::items::Catalog>,
) {
    for shot in shots.read() {
        // An artifact in an adapter on this gun.
        let mods = mounted
            .iter()
            .filter(|(m, _)| m.0 == shot.gun)
            .map(|(_, it)| shot_mods(&catalog.get(it.kind()).id))
            .find(|m| m.fall != 1.0 || m.damage != 1.0 || m.stun > 0.0 || m.seek)
            .unwrap_or_default();
        let mut rng = Rng::new(shot.seed);
        let dir = shot.direction.normalize_or(Vec3::NEG_Z);
        let side = dir.any_orthonormal_vector();
        let up = dir.cross(side);
        for _ in 0..shot.projectiles.max(1) {
            // Uniform-in-disc spread.
            let r = shot.spread * rng.f32().sqrt();
            let a = rng.range(0.0, std::f32::consts::TAU);
            let d = (dir + side * (r * a.cos()) + up * (r * a.sin())).normalize();
            commands.spawn((
                Name::new("Projectile"),
                Projectile {
                    velocity: d * shot.speed,
                    damage: shot.damage,
                    shooter: shot.shooter,
                    gun: shot.gun,
                    life: 3.0,
                    mods,
                },
                Transform::from_translation(shot.origin),
            ));
        }
        commands.spawn((
            Name::new("Muzzle flash"),
            Flash { life: 0.05 },
            PointLight {
                color: Color::srgb(1.0, 0.75, 0.45),
                intensity: 60_000.0,
                range: 12.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(shot.origin + dir * 0.05),
        ));
    }
}

fn fly_projectiles(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Projectile, &mut Transform)>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    parents: Query<&ChildOf>,
    parts: Query<(&crate::robots::RobotPart, &GlobalTransform)>,
    zone: Option<Res<crate::world::Zone>>,
    mut splashes: MessageWriter<super::impacts::Impact>,
) {
    let dt = time.delta_secs();
    let filter =
        SpatialQueryFilter::from_mask([Layer::World, Layer::Item, Layer::Robot, Layer::Player]);
    for (e, mut p, mut t) in &mut projectiles {
        p.life -= dt;
        let from = t.translation;
        let step = p.velocity * dt;
        let len = step.length();
        let shooter = p.shooter;
        let gun = p.gun;
        // Ignore the shooter (and anything parented to them) and the gun itself.
        let is_own = |hit: Entity| {
            if hit == gun || Some(hit) == shooter {
                return true;
            }
            let mut cur = hit;
            for _ in 0..6 {
                match parents.get(cur) {
                    Ok(c) => {
                        cur = c.parent();
                        if cur == gun || Some(cur) == shooter {
                            return true;
                        }
                    }
                    Err(_) => break,
                }
            }
            false
        };
        let hit = Dir3::new(step).ok().and_then(|dir| {
            spatial.cast_ray_predicate(from, dir, len, true, &filter, &|h| !is_own(h))
        });
        // Into a water surface before anything else: a splash, and the
        // round is spent (it doesn't go on to the bed).
        let end = from + step;
        let water = zone
            .as_ref()
            .and_then(|z| z.hm.water_level([end.x, end.z]))
            .filter(|w| from.y >= *w && end.y < *w)
            .map(|w| len * (from.y - w) / (from.y - end.y).max(1e-6));
        if let Some(d) = water.filter(|d| hit.as_ref().is_none_or(|h| h.distance > *d)) {
            splashes.write(super::impacts::Impact {
                kind: super::impacts::ImpactKind::Water,
                point: from + step.normalize() * d,
                normal: Vec3::Y,
                velocity: p.velocity,
                entity: None,
                decal: false,
            });
            commands.entity(e).despawn();
            continue;
        }
        if let Some(hit) = hit {
            let point = from + step.normalize() * hit.distance;
            hits.write(Hit {
                entity: hit.entity,
                point,
                normal: hit.normal,
                velocity: p.velocity,
                damage: p.damage * p.mods.damage,
                shooter: p.shooter,
            });
            if p.mods.stun > 0.0 {
                if let Ok((part, _)) = parts.get(hit.entity) {
                    commands
                        .entity(part.robot)
                        .insert(crate::robots::Stunned(p.mods.stun));
                }
            }
            commands.entity(e).despawn();
            continue;
        }
        t.translation += step;
        let fall = p.mods.fall;
        p.velocity += GRAVITY * dt * fall;
        p.velocity *= 1.0 - DRAG * dt * fall;
        if p.mods.seek {
            // Bend toward the nearest robot part within a narrow cone.
            let dir = p.velocity.normalize_or_zero();
            let target = parts
                .iter()
                .map(|(_, g)| g.translation() - t.translation)
                .filter(|v| v.length() < 120.0 && v.normalize_or_zero().dot(dir) > 0.985)
                .min_by(|a, b| a.length().total_cmp(&b.length()));
            if let Some(v) = target {
                let speed = p.velocity.length();
                let turned = dir
                    .lerp(v.normalize(), (6.0 * dt).min(1.0))
                    .normalize_or(dir);
                p.velocity = turned * speed;
            }
        }
        if p.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

/// Dynamic things that get shot get shoved.
fn push_hit_bodies(mut hits: MessageReader<Hit>, mut bodies: Query<(&RigidBody, Forces)>) {
    for hit in hits.read() {
        if let Ok((rb, mut forces)) = bodies.get_mut(hit.entity) {
            if *rb == RigidBody::Dynamic {
                // Momentum of a ~8 g bullet, exaggerated a little for feel.
                let impulse = hit.velocity * 0.012;
                forces.apply_linear_impulse_at_point(impulse, hit.point);
            }
        }
    }
}

fn fade_flashes(
    mut commands: Commands,
    mut flashes: Query<(Entity, &mut Flash, &mut PointLight)>,
    time: Res<Time>,
) {
    for (e, mut f, mut light) in &mut flashes {
        f.life -= time.delta_secs();
        light.intensity *= 0.5;
        if f.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}
