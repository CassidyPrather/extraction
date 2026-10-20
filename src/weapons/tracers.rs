//! Tracers (client side): a tracer round burns a bright streak along its
//! flight, from a little past the muzzle until its compound burns out
//! (about 800 m). The streak is the round's last few hundredths of a
//! second of path, as the eye smears it; it keeps a pixel or two of width
//! however far off it is, and fog doesn't swallow it (that's what a tracer
//! is for: seeing where your rounds go at dusk and in murk).
//!
//! - A pool of at most [`TRACER_CAP`] streaks; the oldest goes first.
//! - When the round stops, its streak shrinks into where it stopped.

use std::collections::VecDeque;

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use extraction_core::ammo::Load;

use super::ballistics::Projectile;

/// Streaks alive at once, at most.
pub const TRACER_CAP: usize = 32;
/// Where the compound lights, and where it burns out, metres from the
/// muzzle.
pub const IGNITE: f32 = 8.0;
pub const BURN_OUT: f32 = 800.0;
/// How much flight the eye smears into a streak, seconds.
pub const SMEAR: f32 = 0.02;
/// Its width as a share of its distance from the eye (a pixel and a bit in
/// a headset), and never thinner than this, metres.
pub const WIDTH_PER_M: f32 = 0.0015;
pub const MIN_WIDTH: f32 = 0.008;
/// Seconds a stopped round's streak takes to shrink away.
pub const FADE: f32 = 0.08;

pub struct TracerPlugin;

impl Plugin for TracerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TracerPool>()
            .add_systems(Startup, tracer_assets)
            .add_systems(Update, (light_tracers, draw_tracers).chain());
    }
}

#[derive(Resource)]
struct TracerAssets {
    mesh: Handle<Mesh>,
    glow: Handle<StandardMaterial>,
}

/// The streaks alive, oldest first.
#[derive(Resource, Default)]
pub struct TracerPool(pub VecDeque<Entity>);

impl TracerPool {
    /// Takes `e` in; returns the oldest if that puts it over the cap.
    pub fn admit(&mut self, e: Entity) -> Option<Entity> {
        self.0.push_back(e);
        if self.0.len() > TRACER_CAP {
            self.0.pop_front()
        } else {
            None
        }
    }
}

/// A streak following a round.
#[derive(Component)]
struct Streak {
    round: Entity,
    origin: Vec3,
    /// Where the round was last seen and how fast it went.
    at: Vec3,
    velocity: Vec3,
    /// Seconds left of its fade once the round has stopped.
    fading: Option<f32>,
}

fn tracer_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(TracerAssets {
        mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        // Red-orange burning compound: additive, unlit, unfogged.
        glow: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.35, 0.12),
            emissive: LinearRgba::rgb(1.0, 0.3, 0.08) * 40.0,
            alpha_mode: AlphaMode::Add,
            unlit: true,
            fog_enabled: false,
            ..default()
        }),
    });
}

/// A tracer round leaving the muzzle gets a streak.
fn light_tracers(
    mut commands: Commands,
    rounds: Query<(Entity, &Projectile, &Transform), Added<Projectile>>,
    assets: Option<Res<TracerAssets>>,
    mut pool: ResMut<TracerPool>,
) {
    let Some(a) = assets else {
        return;
    };
    for (e, p, t) in &rounds {
        if p.load != Load::Tracer {
            continue;
        }
        let streak = commands
            .spawn((
                Name::new("Tracer"),
                Streak {
                    round: e,
                    origin: t.translation,
                    at: t.translation,
                    velocity: p.velocity,
                    fading: None,
                },
                Mesh3d(a.mesh.clone()),
                MeshMaterial3d(a.glow.clone()),
                Transform::from_translation(t.translation).with_scale(Vec3::ZERO),
                NotShadowCaster,
                NotShadowReceiver,
            ))
            .id();
        if let Some(old) = pool.admit(streak) {
            commands.entity(old).try_despawn();
        }
    }
}

/// The streak's pose: from `tail` back along the flight to `head` where the
/// round is, `width` thick (zero scale if it's not lit).
pub fn streak_pose(origin: Vec3, head: Vec3, velocity: Vec3, eye: Vec3) -> Transform {
    let flown = head.distance(origin);
    let dir = velocity.normalize_or(Vec3::NEG_Z);
    if !(IGNITE..BURN_OUT).contains(&flown) {
        return Transform::from_translation(head).with_scale(Vec3::ZERO);
    }
    // The eye's smear, but never back past where it lit.
    let len = (velocity.length() * SMEAR).min(flown - IGNITE).max(0.0);
    let mid = head - dir * (len * 0.5);
    let width = (mid.distance(eye) * WIDTH_PER_M).max(MIN_WIDTH);
    Transform::from_translation(mid)
        .looking_to(dir, if dir.y.abs() > 0.99 { Vec3::X } else { Vec3::Y })
        .with_scale(Vec3::new(width, width, len.max(1e-4)))
}

fn draw_tracers(
    mut commands: Commands,
    mut streaks: Query<(Entity, &mut Streak, &mut Transform), Without<Projectile>>,
    rounds: Query<(&Projectile, &Transform)>,
    eyes: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    time: Res<Time>,
    mut pool: ResMut<TracerPool>,
) {
    let eye = eyes.iter().next().map_or(Vec3::ZERO, |g| g.translation());
    let dt = time.delta_secs();
    for (e, mut s, mut t) in &mut streaks {
        if let Ok((p, rt)) = rounds.get(s.round) {
            s.at = rt.translation;
            s.velocity = p.velocity;
        } else {
            // It stopped: shrink into where it stopped.
            let left = s.fading.unwrap_or(FADE) - dt;
            if left <= 0.0 {
                pool.0.retain(|x| *x != e);
                commands.entity(e).despawn();
                continue;
            }
            s.fading = Some(left);
            s.velocity *= left / FADE;
        }
        *t = streak_pose(s.origin, s.at, s.velocity, eye);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lit a little past the muzzle, out by 800 m; the streak ends at the
    /// round, never reaches back past where it lit, and stays a visible
    /// width far off.
    #[test]
    fn a_streak_from_where_it_lit_to_the_round() {
        let v = Vec3::new(0.0, 0.0, -800.0);
        let eye = Vec3::ZERO;
        let at = |d: f32| streak_pose(Vec3::ZERO, Vec3::new(0.0, 0.0, -d), v, eye);
        assert_eq!(at(3.0).scale, Vec3::ZERO, "not lit yet");
        assert_eq!(at(900.0).scale, Vec3::ZERO, "burnt out");
        let near = at(12.0);
        assert!(
            (near.scale.z - 4.0).abs() < 1e-3,
            "back to where it lit: {near:?}"
        );
        let far = at(400.0);
        assert!((far.scale.z - 16.0).abs() < 1e-3, "the eye's smear");
        let head = far.translation + far.forward() * (far.scale.z * 0.5);
        assert!(head.distance(Vec3::new(0.0, 0.0, -400.0)) < 1e-2, "{head}");
        assert!(far.scale.x > near.scale.x && far.scale.x >= 0.5, "{far:?}");
    }

    /// A drum of tracers never holds more than the cap; the oldest go.
    #[test]
    fn the_pool_is_capped() {
        let mut pool = TracerPool::default();
        let mut gone = Vec::new();
        for i in 0..71u32 {
            gone.extend(pool.admit(Entity::from_raw_u32(i + 1).unwrap()));
        }
        assert_eq!(pool.0.len(), TRACER_CAP);
        assert_eq!(gone.len(), 71 - TRACER_CAP);
        assert_eq!(gone[0], Entity::from_raw_u32(1).unwrap(), "oldest first");
    }
}
