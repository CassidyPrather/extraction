//! Signal flares (`docs/ARMOURY.md`): the SPSh-44's 26 mm stars. A star
//! climbs, burns for [`BURN`] seconds and sinks slowly once past the top
//! of its arc; it lies burning where it comes down. A white star lights
//! the ground under it (you as much as them: robots see you by it); red
//! and green are signals. Robots look at a burning star and go to where
//! it is, and one that takes a star to the face is dazzled.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::ammo::Load;

use super::Gun;
use super::ballistics::{Hit, Shot};
use crate::world::Layer;

/// Seconds a star burns.
pub const BURN: f32 = 8.0;
/// Seconds before it lights (the star clears the muzzle first).
pub const IGNITE: f32 = 0.2;
/// Air drag on the climb (quadratic, per metre): a 75 m/s star straight
/// up tops out about 85 m.
const DRAG_UP: f32 = 0.012;
/// ...and once it's coming down: it sinks at about 6 m/s.
const DRAG_DOWN: f32 = 0.27;
/// How far a white star lights the ground, metres.
pub const WHITE_REACH: f32 = 60.0;
/// How far robots see a burning star from (and go to it): a white one,
/// and a coloured signal, metres.
pub const SEEN_WHITE: f32 = 160.0;
pub const SEEN_SIGNAL: f32 = 220.0;
/// Seconds between the looks a burning star draws.
const DRAW_EVERY: f32 = 1.0;
/// Seconds a robot hit in the face by a star is dazzled.
pub const DAZZLE: f32 = 3.0;
/// Stars alive at once (the oldest goes).
pub const STARS_CAP: usize = 6;

/// A star's colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Star {
    White,
    Red,
    Green,
}

impl Star {
    /// The star a 26 mm round of `load` throws.
    pub fn of(load: Load) -> Self {
        match load {
            Load::SignalRed => Star::Red,
            Load::SignalGreen => Star::Green,
            _ => Star::White,
        }
    }
}

/// A burning (or not yet lit) star.
#[derive(Component, Debug)]
pub struct Flare {
    pub star: Star,
    pub velocity: Vec3,
    /// Seconds since it left the muzzle.
    pub age: f32,
    /// It has come down and lies where it fell.
    pub landed: bool,
    pub shooter: Option<Entity>,
    draw_in: f32,
}

impl Flare {
    pub fn burning(&self) -> bool {
        self.age >= IGNITE && self.age < IGNITE + BURN
    }

    /// 0..1: how bright, fading over its last second.
    pub fn glow(&self) -> f32 {
        if !self.burning() {
            return 0.0;
        }
        (IGNITE + BURN - self.age).clamp(0.0, 1.0)
    }
}

/// A gun that throws flares, not bullets: ballistics leaves its shots to
/// [`launch`].
#[derive(Component, Clone, Copy, Debug)]
pub struct Launcher;

/// Where white stars are burning now, and how bright: robots' sight
/// reads it (a star overhead lights you as a lamp would).
#[derive(Resource, Default, Debug)]
pub struct Starlight(pub Vec<(Vec3, f32)>);

impl Starlight {
    /// How lit a point is by the stars, 0..1.
    pub fn at(&self, p: Vec3) -> f32 {
        self.0
            .iter()
            .map(|(s, glow)| glow * (1.0 - s.distance(p) / WHITE_REACH).clamp(0.0, 1.0).sqrt())
            .fold(0.0, f32::max)
    }
}

pub struct FlaresPlugin;

impl Plugin for FlaresPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Starlight>().add_systems(
            FixedUpdate,
            (launch, fly)
                .chain()
                .after(super::ballistics::BallisticsSystems),
        );
    }
}

/// A flare gun's shot becomes a star.
fn launch(
    mut commands: Commands,
    mut shots: MessageReader<Shot>,
    guns: Query<&Gun, With<Launcher>>,
    stars: Query<(Entity, &Flare)>,
) {
    for shot in shots.read() {
        let Ok(gun) = guns.get(shot.gun) else {
            continue;
        };
        // The oldest goes first.
        let mut alive: Vec<_> = stars.iter().collect();
        if alive.len() >= STARS_CAP {
            alive.sort_by(|a, b| b.1.age.total_cmp(&a.1.age));
            commands.entity(alive[0].0).try_despawn();
        }
        commands.spawn((
            Name::new("Flare"),
            Flare {
                star: Star::of(gun.firearm.last_fired),
                velocity: shot.direction.normalize_or(Vec3::NEG_Z) * shot.speed,
                age: 0.0,
                landed: false,
                shooter: shot.shooter,
                draw_in: IGNITE,
            },
            Transform::from_translation(shot.origin),
            Visibility::default(),
        ));
    }
}

/// Stars fly, hit, burn and draw robots; then they're gone.
fn fly(
    mut commands: Commands,
    mut stars: Query<(Entity, &mut Flare, &mut Transform)>,
    spatial: SpatialQuery,
    parts: Query<&crate::robots::RobotPart>,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut light: ResMut<Starlight>,
) {
    let dt = time.delta_secs();
    light.0.clear();
    let filter = SpatialQueryFilter::from_mask([Layer::World, Layer::Robot]);
    for (e, mut f, mut t) in &mut stars {
        f.age += dt;
        if f.age > IGNITE + BURN {
            commands.entity(e).despawn();
            continue;
        }
        if !f.landed {
            let v = f.velocity;
            let k = if v.y > 0.0 { DRAG_UP } else { DRAG_DOWN };
            let mut nv = v + Vec3::new(0.0, -9.81, 0.0) * dt;
            nv -= nv * (k * nv.length() * dt).min(0.9);
            let step = (v + nv) * 0.5 * dt;
            if let Ok(dir) = Dir3::new(step) {
                if let Some(hit) =
                    spatial.cast_ray(t.translation, dir, step.length(), true, &filter)
                {
                    let at = t.translation + dir * hit.distance;
                    if let Ok(part) = parts.get(hit.entity) {
                        // In the face: it burns and it dazzles.
                        hits.write(Hit {
                            entity: hit.entity,
                            point: at,
                            normal: hit.normal,
                            velocity: v,
                            damage: 20.0,
                            shooter: f.shooter,
                            mass_g: 20.0,
                            load: Load::Standard,
                            blow: None,
                        });
                        commands
                            .entity(part.robot)
                            .insert(crate::robots::Stunned(DAZZLE));
                    }
                    t.translation = at + hit.normal * 0.03;
                    f.velocity = Vec3::ZERO;
                    f.landed = true;
                    continue;
                }
            }
            t.translation += step;
            f.velocity = nv;
        }
        if !f.burning() {
            continue;
        }
        if f.star == Star::White {
            light.0.push((t.translation, f.glow()));
        }
        f.draw_in -= dt;
        if f.draw_in <= 0.0 {
            f.draw_in = DRAW_EVERY;
            // What's out there sees it and comes to look.
            noise.write(crate::robots::Noise {
                at: t.translation,
                radius: if f.star == Star::White {
                    SEEN_WHITE
                } else {
                    SEEN_SIGNAL
                },
                source: None,
            });
        }
    }
}

/// A star's light and its burning ball (client side).
pub fn flare_visuals(
    mut commands: Commands,
    added: Query<(Entity, &Flare), Added<Flare>>,
    mut lights: Query<(&ChildOf, &mut PointLight)>,
    stars: Query<&Flare>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, f) in &added {
        let (colour, reach) = match f.star {
            Star::White => (Color::srgb(1.0, 0.95, 0.85), WHITE_REACH),
            Star::Red => (Color::srgb(1.0, 0.15, 0.08), 25.0),
            Star::Green => (Color::srgb(0.2, 1.0, 0.25), 25.0),
        };
        let lin = colour.to_linear();
        commands.entity(e).with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(Sphere::new(0.05))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: colour,
                    emissive: LinearRgba::rgb(lin.red * 40.0, lin.green * 40.0, lin.blue * 40.0),
                    unlit: true,
                    ..default()
                })),
            ));
            c.spawn((
                PointLight {
                    color: colour,
                    intensity: 0.0,
                    range: reach,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::default(),
            ));
        });
    }
    for (parent, mut light) in &mut lights {
        let Ok(f) = stars.get(parent.parent()) else {
            continue;
        };
        // A burning star flickers.
        let flicker = 0.85 + 0.15 * (f.age * 31.0).sin() * (f.age * 17.0).cos();
        let peak = if f.star == Star::White { 4.0e7 } else { 4.0e6 };
        light.intensity = peak * f.glow() * flicker;
    }
}

#[cfg(test)]
mod vr_tests {
    use super::*;
    use crate::rig::Side;
    use crate::weapons::ballistics::Projectile;
    use crate::weapons::feel::vr_tests::*;

    /// The signal pistol by hand: broken open with A, a star thumbed in,
    /// shut, fired up. A star (no bullet) climbs and lights: a white one
    /// lights the ground about it, robots are told where it burns; a red
    /// one draws them further and lights nothing.
    #[test]
    fn the_flare_pistol_throws_a_burning_star() {
        #[derive(Resource, Default)]
        struct Heard(Vec<f32>);
        for (id, star) in [("flare_white", Star::White), ("flare_red", Star::Red)] {
            let mut r = hold("flare_pistol", false, false);
            r.vr.app.init_resource::<Heard>().add_systems(
                FixedPostUpdate,
                |mut n: MessageReader<crate::robots::Noise>, mut h: ResMut<Heard>| {
                    h.0.extend(n.read().map(|n| n.radius))
                },
            );
            press(&mut r, "a");
            assert!(gun_mut(&mut r).firearm.bolt_travel > 0.5, "open");
            take(&mut r, id, 1);
            let (port, _) = load_port(&mut r);
            let away = r.vr.hand_state(Side::Left).world.translation;
            glide(&mut r, away, port, 12);
            r.vr.step(2);
            r.vr.grip(Side::Left, 0.0);
            r.vr.step(2);
            press(&mut r, "a");
            assert_eq!(gun_mut(&mut r).firearm.live_rounds(), 1, "{id} in");
            r.vr.trigger(Side::Right, 1.0);
            r.vr.step(3);
            r.vr.trigger(Side::Right, 0.0);
            r.vr.step(60);
            let mut q = r.vr.app.world_mut().query::<(&Flare, &GlobalTransform)>();
            let (f, at) = q
                .single(r.vr.app.world())
                .map(|(f, g)| (f.star, g.translation()))
                .unwrap();
            assert_eq!(f, star);
            assert!(at.y > r.feet.y + 5.0, "it climbed: {at}");
            let mut p =
                r.vr.app
                    .world_mut()
                    .query_filtered::<(), With<Projectile>>();
            assert_eq!(p.iter(r.vr.app.world()).count(), 0, "no bullet");
            let lit =
                r.vr.app
                    .world()
                    .resource::<Starlight>()
                    .at(at - Vec3::Y * 20.0);
            assert_eq!(lit > 0.3, star == Star::White, "{id}: lit {lit}");
            let heard = &r.vr.app.world().resource::<Heard>().0;
            let want = if star == Star::White {
                SEEN_WHITE
            } else {
                SEEN_SIGNAL
            };
            assert!(heard.contains(&want), "{id}: {heard:?}");
            r.vr.step(BURN as usize * 64 + 60);
            let mut q = r.vr.app.world_mut().query::<&Flare>();
            assert_eq!(q.iter(r.vr.app.world()).count(), 0, "burnt out");
        }
    }
}
