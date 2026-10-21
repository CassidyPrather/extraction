//! The pieces a break throws: real physics bodies, cheap ones.
//!
//! Each is a thin box (a shard's or splinter's bounds) on the
//! [`Layer::Debris`] layer: it lands on the world and nothing else (not
//! on you, the robots, items or each other), so a window's worth costs a
//! few contacts with the floor and no query ever finds one. They sleep
//! when they settle and are gone after [`DEBRIS_LIFE`]. At most
//! [`DEBRIS_CAP`] are alive: past it the oldest goes first, so a
//! firefight through a shop front costs no more than one window.
//!
//! No joints, no disabled bodies: a piece is a plain dynamic body, so
//! despawning it at any moment (mid-flight, asleep, the frame it was
//! spawned) leaves nothing behind for the solver's islands.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::fracture::P2;
use extraction_core::noise::Rng;
use std::collections::VecDeque;

use crate::world::Layer;

/// Pieces alive at once, every break together.
pub const DEBRIS_CAP: usize = 96;
/// Seconds a piece lies before it's cleared away.
pub const DEBRIS_LIFE: f32 = 10.0;
/// Pieces one break may throw (the rest of its pieces are too small to
/// matter or are left to the impact's glitter).
pub const PER_BREAK: usize = 18;

/// What a piece is, for its look and its weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Glass,
    /// Wood of a material (planks, a crate's slats).
    Wood(crate::render::materials::Mat),
}

/// A flying (or lying) piece. Its outline is in its own x-y plane about
/// its centre, `thick` deep along z.
#[derive(Component, Clone, Debug)]
pub struct Debris {
    pub kind: Kind,
    pub poly: Vec<P2>,
    pub thick: f32,
    pub age: f32,
    pub life: f32,
}

/// The live pieces, oldest first, and how many there have been.
#[derive(Resource, Debug, Default)]
pub struct DebrisPool {
    pub live: VecDeque<Entity>,
    /// The most alive at once (the gauge the tests read).
    pub peak: usize,
    pub spawned: u64,
    /// Cleared early to make room.
    pub culled: u64,
}

/// One piece to throw.
#[derive(Clone, Debug)]
pub struct Piece {
    pub kind: Kind,
    pub poly: Vec<P2>,
    pub thick: f32,
    /// Where its centre is and how it's turned (its z the thin way).
    pub at: Transform,
    pub velocity: Vec3,
    pub spin: Vec3,
}

pub struct DebrisPlugin;

impl Plugin for DebrisPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebrisPool>().add_systems(
            FixedUpdate,
            // Before anything breaks this tick: the pool then holds
            // only pieces that exist (last tick's commands applied).
            age_debris
                .after(crate::weapons::ballistics::BallisticsSystems)
                .before(super::DestructionSystems),
        );
    }
}

fn density(kind: Kind) -> f32 {
    match kind {
        Kind::Glass => 2500.0,
        Kind::Wood(_) => 550.0,
    }
}

/// Throws pieces, making room in the pool first.
pub fn throw(commands: &mut Commands, pool: &mut DebrisPool, pieces: Vec<Piece>) {
    for p in pieces.into_iter().take(PER_BREAK) {
        while pool.live.len() >= DEBRIS_CAP {
            if let Some(old) = pool.live.pop_front() {
                commands.entity(old).try_despawn();
                pool.culled += 1;
            }
        }
        let mut half = [0.0f32; 2];
        for v in &p.poly {
            half[0] = half[0].max(v[0].abs());
            half[1] = half[1].max(v[1].abs());
        }
        // Never thinner than a few millimetres to the solver.
        let size = Vec3::new(
            (half[0] * 2.0).max(0.01),
            (half[1] * 2.0).max(0.01),
            p.thick.max(0.006),
        );
        let e = commands
            .spawn((
                Name::new("Debris"),
                Debris {
                    kind: p.kind,
                    poly: p.poly,
                    thick: p.thick,
                    age: 0.0,
                    life: DEBRIS_LIFE,
                },
                p.at,
                Visibility::default(),
                RigidBody::Dynamic,
                Collider::cuboid(size.x, size.y, size.z),
                ColliderDensity(density(p.kind)),
                CollisionLayers::new(Layer::Debris, [Layer::World]),
                Friction::new(0.7),
                Restitution::new(0.15),
                LinearVelocity(p.velocity),
                AngularVelocity(p.spin),
                LinearDamping(0.05),
                AngularDamping(0.4),
            ))
            .id();
        pool.live.push_back(e);
        pool.spawned += 1;
    }
    pool.peak = pool.peak.max(pool.live.len());
}

/// Velocities for the pieces of a break at `centre`: pushed along `push`
/// (the round's way, or a blow's) hardest near `hit`, and a little
/// tumble. Deterministic from `seed`.
pub fn fling(rng: &mut Rng, at: Vec3, hit: Vec3, push: Vec3) -> (Vec3, Vec3) {
    let near = (1.0 - at.distance(hit) / 0.6).clamp(0.0, 1.0);
    let jitter = Vec3::new(
        rng.range(-1.0, 1.0),
        rng.range(-0.3, 0.8),
        rng.range(-1.0, 1.0),
    );
    let v = push * (0.25 + 0.75 * near) + jitter * 0.6;
    let spin = Vec3::new(
        rng.range(-9.0, 9.0),
        rng.range(-9.0, 9.0),
        rng.range(-9.0, 9.0),
    );
    (v, spin)
}

/// Pieces age out; the pool forgets the dead.
fn age_debris(
    mut commands: Commands,
    mut pieces: Query<(Entity, &mut Debris)>,
    mut pool: ResMut<DebrisPool>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut d) in &mut pieces {
        d.age += dt;
        if d.age >= d.life {
            commands.entity(e).try_despawn();
        }
    }
    // Gone already (aged out, culled, or swept with the Zone).
    pool.live
        .retain(|e| pieces.get(*e).is_ok_and(|(_, d)| d.age < d.life));
}
