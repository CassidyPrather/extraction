//! Bullets as fast, simulated projectiles: gravity, drag, swept against the
//! world every tick. Hits become [`Hit`] messages (robots read them for
//! damage), impulses on whatever they strike, and impact effects.
//!
//! Recoil is in [`super::feel`]: the shot kicks the gun in the hands that
//! hold it, and they bring it back.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::ammo::Load;
use extraction_core::firearm_terminal::Stuff;
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
    /// The bullet's mass, grams (its energy is what gets it through
    /// things: `extraction_core::firearm_terminal`).
    pub mass_g: f32,
    /// Its own randomness (ricochet rolls), and what it's been through or
    /// off so far.
    pub seed: u64,
    pub passes: u8,
    /// What it's loaded with (AP, a tracer, a slug).
    pub load: Load,
}

/// A round glanced off a hard face (for the whine and a spray of sparks).
#[derive(Message, Clone, Copy, Debug)]
pub struct Ricochet {
    pub point: Vec3,
    pub normal: Vec3,
    /// Its velocity leaving.
    pub velocity: Vec3,
}

/// How many walls and glances a round survives before it's spent.
const MAX_PASSES: u8 = 3;

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
    /// Air drag scale on its own (a grenade's fragments slow fast).
    pub drag: f32,
}

impl Default for ShotMods {
    fn default() -> Self {
        Self {
            fall: 1.0,
            damage: 1.0,
            stun: 0.0,
            seek: false,
            drag: 1.0,
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
    /// The round's mass, grams, if it was a round (armour weighs it); 0
    /// for a blast or a blade.
    pub mass_g: f32,
    /// What the round was loaded with (armour weighs an AP core).
    pub load: Load,
    /// A blow, not a round: a point, an edge or something blunt (wood and
    /// robots weigh them differently). `None` for rounds and blasts.
    pub blow: Option<extraction_core::melee::Blow>,
}

/// A muzzle flash's light, fading out.
#[derive(Component)]
pub struct Flash {
    pub life: f32,
}

/// Shots fired and projectiles moved: hit messages are out after this.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BallisticsSystems;

/// A slug's share of the buckshot load's summed harm, its speed against
/// buckshot's, and its spread at most, radians.
pub const SLUG_SHARE: f32 = 0.8;
pub const SLUG_SPEED: f32 = 0.95;
pub const SLUG_SPREAD: f32 = 0.004;
/// An AP round's harm to a robot's armour plate, against ball's.
pub const AP_PLATE: f32 = 1.6;

/// Air drag, 1/s: a rough fit so a rifle round loses ~20% by 200 m.
const DRAG: f32 = 0.12;
const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

/// A round leaves at `speed` m/s, `angle` radians above the line it's
/// measured from (the line of sight): its height above that line when it
/// has come `range` metres along it, and when (seconds). Integrated as
/// the game flies rounds, at the fixed rate, so a sight zeroed by it is
/// zeroed for the game's rounds. `None` if it never gets there.
pub fn height_at(speed: f32, angle: f32, range: f32) -> Option<(f32, f32)> {
    let dt = 1.0 / 90.0;
    let mut p = Vec2::ZERO;
    let mut v = Vec2::new(angle.cos(), angle.sin()) * speed;
    let mut t = 0.0;
    while t < 6.0 {
        let next = p + v * dt;
        if next.x >= range {
            let f = (range - p.x) / (next.x - p.x).max(1e-6);
            return Some((p.y + (next.y - p.y) * f, t + dt * f));
        }
        p = next;
        v.y += GRAVITY.y * dt;
        v *= 1.0 - (DRAG * dt).min(0.9);
        t += dt;
    }
    None
}

/// The angle (radians, up) a bore `sight_height` metres below the line of
/// sight must hold to it for a round at `speed` to cross it at `range`.
pub fn zero_angle(speed: f32, sight_height: f32, range: f32) -> f32 {
    let mut a = 0.0;
    for _ in 0..4 {
        let Some((h, _)) = height_at(speed, a, range) else {
            return a;
        };
        // Where it is at `range`, measured from the line of sight.
        a += (sight_height - h) / range;
    }
    a
}

pub struct BallisticsPlugin;

impl Plugin for BallisticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Shot>()
            .add_message::<Hit>()
            .add_message::<Ricochet>()
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

pub(crate) fn fire_shots(
    mut commands: Commands,
    mut shots: MessageReader<Shot>,
    mounted: Query<(&crate::gadgets::MountedOn, &crate::items::Item)>,
    catalog: Res<crate::items::Catalog>,
    guns: Query<&super::Gun>,
    launchers: Query<(), With<super::flares::Launcher>>,
) {
    for shot in shots.read() {
        // A flare gun throws a star, not a bullet (`flares`).
        if launchers.contains(shot.gun) {
            continue;
        }
        // Robots' guns have no cartridge: a middling bullet. A gun's shot
        // is the round it just fired.
        let (mass_g, load) = guns.get(shot.gun).map_or((8.0, Load::Standard), |g| {
            let load = g.firearm.last_fired;
            (load.mass_g(&g.firearm.def.cartridge), load)
        });
        // A slug is one ball where buckshot is many: the pellets' harm in
        // one, with a rifle's tight group.
        let (projectiles, damage, spread, speed) = if load == Load::Slug {
            (
                1,
                shot.damage * shot.projectiles.max(1) as f32 * SLUG_SHARE,
                shot.spread.min(SLUG_SPREAD),
                shot.speed * SLUG_SPEED,
            )
        } else if load == Load::Subsonic {
            // Slower than sound, a heavier bullet: harm by the square root
            // of its energy against ball's.
            let speed = load.speed(shot.speed);
            let ball = guns.get(shot.gun).map_or(mass_g, |g| {
                extraction_core::firearm_terminal::bullet_mass_g(&g.firearm.def.cartridge)
            });
            let share =
                (mass_g * speed * speed / (ball * shot.speed * shot.speed).max(1e-3)).sqrt();
            (
                shot.projectiles.max(1),
                shot.damage * share.min(1.0),
                shot.spread,
                speed,
            )
        } else {
            (
                shot.projectiles.max(1),
                shot.damage,
                shot.spread,
                shot.speed,
            )
        };
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
        for _ in 0..projectiles {
            // Uniform-in-disc spread.
            let r = spread * rng.f32().sqrt();
            let a = rng.range(0.0, std::f32::consts::TAU);
            let d = (dir + side * (r * a.cos()) + up * (r * a.sin())).normalize();
            commands.spawn((
                Name::new("Projectile"),
                Projectile {
                    velocity: d * speed,
                    damage,
                    shooter: shot.shooter,
                    gun: shot.gun,
                    life: 3.0,
                    mods,
                    mass_g,
                    seed: rng.next_u64(),
                    passes: 0,
                    load,
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

pub(crate) fn fly_projectiles(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Projectile, &mut Transform)>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    parents: Query<&ChildOf>,
    parts: Query<(&crate::robots::RobotPart, &GlobalTransform)>,
    zone: Option<Res<crate::world::Zone>>,
    mut splashes: MessageWriter<super::impacts::Impact>,
    struck: Struck,
    mut ricochets: MessageWriter<Ricochet>,
) {
    use extraction_core::firearm_terminal::{Outcome, strike_load};
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
                part: None,
                hurt: false,
            });
            commands.entity(e).despawn();
            continue;
        }
        if let Some(hit) = hit {
            let point = from + step.normalize() * hit.distance;
            let speed = p.velocity.length();
            let dir = p.velocity / speed.max(1e-3);
            let cos_in = (-dir).dot(hit.normal).clamp(0.0, 1.0);
            let (stuff, mm, along) = struck.stuff(hit.entity, point, dir, cos_in);
            // A round starting inside something (a double wall) is spent.
            let outcome = if p.passes >= MAX_PASSES || hit.distance < 1e-4 {
                Outcome::Stop
            } else {
                let roll =
                    Rng::new(p.seed ^ (u64::from(p.passes) + 1).wrapping_mul(0x9E37_79B9)).f32();
                strike_load(p.load, stuff, p.mass_g, speed, cos_in, mm, roll)
            };
            let glancing = matches!(outcome, Outcome::Ricochet { .. });
            // An AP core punches a robot's plate; soft things it harms a
            // little less than ball, which breaks up in them.
            let core = if p.load != Load::Ap {
                p.load.soft()
            } else if struck.is_plate(hit.entity) {
                AP_PLATE
            } else {
                p.load.soft()
            };
            hits.write(Hit {
                entity: hit.entity,
                point,
                normal: hit.normal,
                velocity: p.velocity,
                // A glancing blow only grazes.
                damage: p.damage * p.mods.damage * core * if glancing { 0.3 } else { 1.0 },
                shooter: p.shooter,
                mass_g: p.mass_g,
                load: p.load,
                blow: None,
            });
            match outcome {
                Outcome::Through { speed: out } => {
                    // Out the far side, slower and weaker, leaving an exit
                    // mark.
                    let exit = point + dir * (along + 0.01);
                    splashes.write(super::impacts::Impact {
                        kind: super::impacts::kind_of_stuff(stuff),
                        point: point + dir * along,
                        normal: dir,
                        velocity: dir * out,
                        entity: Some(hit.entity),
                        decal: struck.marks(hit.entity),
                        part: None,
                        hurt: false,
                    });
                    let keep = (out / speed.max(1e-3)).powi(2);
                    p.velocity = dir * out;
                    p.damage *= keep;
                    p.passes += 1;
                    t.translation = exit;
                    continue;
                }
                Outcome::Ricochet { speed: out } => {
                    let n = hit.normal;
                    let mut rng = Rng::new(p.seed ^ 0x51CE);
                    let jitter = Vec3::new(
                        rng.range(-0.08, 0.08),
                        rng.range(0.0, 0.1),
                        rng.range(-0.08, 0.08),
                    );
                    let off = (dir - 2.0 * dir.dot(n) * n + jitter).normalize_or(n);
                    p.velocity = off * out;
                    p.damage *= (out / speed.max(1e-3)).powi(2);
                    p.passes += 1;
                    t.translation = point + n * 0.01;
                    ricochets.write(Ricochet {
                        point,
                        normal: n,
                        velocity: p.velocity,
                    });
                    continue;
                }
                Outcome::Stop => {}
            }
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
        let drag = (DRAG * p.mods.drag * dt * fall).min(0.9);
        p.velocity *= 1.0 - drag;
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

/// A sheet-steel box thicker than this (m) is a hollow body, not a skin.
pub const HOLLOW_OVER: f32 = 0.04;

/// What a built piece of `m`, `half` its box's half extents, is to a
/// round: sheet steel made up into something thicker than a skin (a car's
/// door, its body, a locker) is hollow.
pub fn piece_stuff(m: crate::render::materials::Mat, half: Vec3) -> Stuff {
    match super::impacts::stuff_of(m) {
        Stuff::Sheet if half.min_element() * 2.0 > HOLLOW_OVER => Stuff::Shell,
        s => s,
    }
}

/// What a round has struck, as far as getting through it goes.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Struck<'w, 's> {
    built: Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            &'static crate::world::surfaces::Surfaces,
        ),
    >,
    trees: Query<'w, 's, (), With<crate::world::trees::TreeTrunks>>,
    structures: Query<'w, 's, (), With<crate::world::structures::Structure>>,
    items: Query<'w, 's, (), With<crate::items::Item>>,
    bodies: Query<'w, 's, (), With<crate::player::BodyCollider>>,
    robot_parts: Query<'w, 's, &'static crate::robots::RobotPart>,
    robots: Query<'w, 's, &'static crate::robots::Robot>,
    shapes: Query<'w, 's, (&'static Collider, &'static GlobalTransform)>,
    held_items: Query<'w, 's, &'static crate::items::Item>,
    catalog: Option<Res<'w, crate::items::Catalog>>,
}

impl Struck<'_, '_> {
    /// What it is, how thick square to its face (mm), and how far the
    /// round's line runs through it (m).
    /// A robot's armour plate.
    fn is_plate(&self, e: Entity) -> bool {
        self.robot_parts
            .get(e)
            .is_ok_and(|p| matches!(p.role, crate::robots::PartRole::Armour(_)))
    }

    pub(crate) fn stuff(
        &self,
        e: Entity,
        point: Vec3,
        dir: Vec3,
        cos_in: f32,
    ) -> (Stuff, f32, f32) {
        const SOLID: f32 = 10_000.0;
        if self.bodies.contains(e) {
            return (Stuff::Plate, SOLID, 0.0);
        }
        if self.items.contains(e) {
            // A car's door carried off as a shield is the door it was.
            if self.is_car_panel(e) {
                let along = self.exit(e, point, dir);
                return (Stuff::Shell, along * 1000.0 * cos_in.max(0.2), along);
            }
            return (Stuff::Plate, SOLID, 0.0);
        }
        if let Ok(part) = self.robot_parts.get(e) {
            use crate::robots::{PartRole, RobotKind};
            let light = self
                .robots
                .get(part.robot)
                .is_ok_and(|r| r.kind == RobotKind::Drone);
            let thin = light
                || matches!(
                    part.role,
                    PartRole::Sensor | PartRole::Rotor(_) | PartRole::GasCell(_)
                );
            if thin && !matches!(part.role, PartRole::Armour(_)) {
                // A thin housing, hollow: in one wall, across, out the other.
                let along = self.exit(e, point, dir);
                return (Stuff::Housing, along * 1000.0 * cos_in.max(0.2), along);
            }
            return (Stuff::Plate, 30.0, 0.03);
        }
        if self.trees.contains(e) {
            return (Stuff::Logs, 300.0, 0.3 / cos_in.max(0.2));
        }
        if let Some((gt, surf)) = self.built.get(e).ok() {
            if let Some((half, t, m)) = surf.piece_at(gt, point) {
                let inv = gt.affine().inverse();
                let q = t.rotation.inverse() * (inv.transform_point3(point) - t.translation);
                let d = t.rotation.inverse() * inv.transform_vector3(dir).normalize_or_zero();
                // Where the line leaves the piece's box.
                let mut exit = f32::INFINITY;
                for i in 0..3 {
                    if d[i].abs() > 1e-6 {
                        let a = (half[i] - q[i]) / d[i];
                        let b = (-half[i] - q[i]) / d[i];
                        exit = exit.min(a.max(b));
                    }
                }
                let along = exit.clamp(0.0, 5.0);
                let stuff = piece_stuff(m, half);
                return (stuff, along * 1000.0 * cos_in.max(0.2), along);
            }
        }
        if self.structures.contains(e) {
            return (Stuff::Concrete, SOLID, 0.0);
        }
        (Stuff::Soil, SOLID, 0.0)
    }

    /// A car's door, bonnet or boot lid, carried.
    fn is_car_panel(&self, e: Entity) -> bool {
        let (Ok(item), Some(catalog)) = (self.held_items.get(e), self.catalog.as_ref()) else {
            return false;
        };
        ["car_door", "car_bonnet"]
            .iter()
            .any(|id| catalog.kind(id) == Some(item.kind()))
    }

    /// How far the line from `point` along `dir` runs inside `e`'s
    /// collider before it comes out (m): a few centimetres if it can't
    /// tell.
    fn exit(&self, e: Entity, point: Vec3, dir: Vec3) -> f32 {
        let Ok((collider, gt)) = self.shapes.get(e) else {
            return 0.03;
        };
        let (_, rot, at) = gt.to_scale_rotation_translation();
        collider
            .cast_ray(at, rot, point + dir * 1e-3, dir, 5.0, false)
            .map_or(0.03, |(d, _)| (d + 1e-3).clamp(0.005, 5.0))
    }

    /// A static thing that wears an exit mark.
    fn marks(&self, e: Entity) -> bool {
        !(self.bodies.contains(e) || self.items.contains(e) || self.robot_parts.contains(e))
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
