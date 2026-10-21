//! A guard on avian's contact bookkeeping.
//!
//! avian 0.7 keeps, for each touching contact pair, one constraint handle
//! per contact manifold, and its solver trusts that blindly
//! (`solver/plugin.rs:398` indexes the pair's manifolds by the handle).
//! The narrow phase can leave a pair with more handles than manifolds
//! when a body is switched off and on while touching, which every seat,
//! stow, wedged door and anomaly lift does: a pair that starts touching
//! as it starts generating constraints gets its manifolds added twice,
//! once per path, and a collider switched off under a disabled body
//! comes back the same way. The solver then panics.
//!
//! A body pushed out of a deep overlap (something switched on, set
//! down or spawned inside it) can be thrown hard, and a heap of them can
//! feed on itself until positions run to 1e15 m and turn NaN, which
//! avian then trips over (an inverted AABB, `collider/mod.rs:512`, or a
//! stall in its tree). Nothing in the game moves a body faster than a
//! thrown grenade, so every body gets a speed limit unless it has its
//! own.
//!
//! And between the narrow phase (and the islands it wakes) and the
//! solver, every step, any pair with handles past its
//! last manifold has those handles taken off again. They are the last
//! ones on the pair (a handle's index is its place in the pair's list),
//! so the ones left still point at the right manifolds.

use avian3d::collision::contact_types::ContactId;
use avian3d::dynamics::solver::constraint_graph::ConstraintGraph;
use avian3d::dynamics::solver::schedule::SolverSystems;
use avian3d::prelude::*;
use bevy::prelude::*;

pub struct ContactGuardPlugin;

/// Fast enough for anything thrown, fired or falling in the game.
pub const MAX_SPEED: f32 = 120.0;
/// Rad/s: a spun casing turns at about 25.
pub const MAX_SPIN: f32 = 120.0;

impl Plugin for ContactGuardPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(|add: On<Add, RigidBody>, mut commands: Commands| {
            commands
                .entity(add.entity)
                .try_insert_if_new((MaxLinearSpeed(MAX_SPEED), MaxAngularSpeed(MAX_SPIN)));
        })
        .add_systems(
            PhysicsSchedule,
            mend_handles
                .after(PhysicsStepSystems::NarrowPhase)
                .before(SolverSystems::PrepareContactConstraints),
        );
    }
}

fn mend_handles(
    mut contacts: ResMut<ContactGraph>,
    mut constraints: ResMut<ConstraintGraph>,
    mut spare: Local<Vec<(ContactId, Entity, Entity, usize)>>,
) {
    spare.clear();
    for pair in contacts.bypass_change_detection().iter_active_mut() {
        // The flag has done its job once the narrow phase has run.
        pair.flags
            .remove(ContactPairFlags::STARTED_GENERATING_CONSTRAINTS);
    }
    for pair in contacts.iter_active() {
        let Some((edge, _)) = contacts.get_by_id(pair.contact_id) else {
            continue;
        };
        let extra = edge
            .constraint_handles
            .len()
            .saturating_sub(pair.manifolds.len());
        if extra > 0
            && let (Some(b1), Some(b2)) = (pair.body1, pair.body2)
        {
            spare.push((pair.contact_id, b1, b2, extra));
        }
    }
    for &(id, b1, b2, extra) in spare.iter() {
        warn!("contact {id:?} had {extra} constraint handles too many; mended");
        for _ in 0..extra {
            constraints.pop_manifold(&mut contacts.edges, id, b1, b2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Layer;

    /// A body of two boxes (so a pair can hold several manifolds, and the
    /// count changes as it rolls) and a ball on a child collider.
    fn spawn_lump(commands: &mut Commands, at: Vec3, k: u32) -> Entity {
        let shape = Collider::compound(vec![
            (Vec3::ZERO, Quat::IDENTITY, Collider::cuboid(0.6, 0.3, 0.6)),
            (
                Vec3::new(0.35, 0.2, 0.0),
                Quat::from_rotation_z(0.4),
                Collider::cuboid(0.2, 0.5, 0.2),
            ),
        ]);
        commands
            .spawn((
                RigidBody::Dynamic,
                shape,
                CollisionLayers::new(Layer::Item, [Layer::World, Layer::Item, Layer::Robot]),
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(k as f32)),
            ))
            .with_child((
                Collider::sphere(0.18),
                CollisionLayers::new(Layer::Robot, [Layer::World, Layer::Item, Layer::Robot]),
                Transform::from_xyz(-0.3, -0.1, 0.2),
            ))
            .id()
    }

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) as u32
        }
    }

    /// Everything the game does to live bodies (sensors, disabling them
    /// as seating and stowing do, body-type swaps, layers, colliders
    /// leaving, despawns, new shapes), done at random to a heap of
    /// touching bodies, several times a step, on the parallel solver.
    /// Without the guard this panics in avian within a second.
    #[test]
    fn churning_touching_bodies_never_breaks_the_solver() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            PhysicsPlugins::default(),
            ContactGuardPlugin,
        ))
        .insert_resource(Time::<Fixed>::from_hz(90.0))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 90.0),
        ));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(40.0, 1.0, 40.0),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
            Transform::from_xyz(0.0, -0.5, 0.0),
        ));
        let mut bodies: Vec<Entity> = Vec::new();
        let mut rng = Rng(7);
        let spawn = |app: &mut App, rng: &mut Rng| {
            let k = rng.next();
            let at = Vec3::new(
                (k % 5) as f32 * 0.7 - 1.4,
                0.4 + (k % 7) as f32 * 0.5,
                ((k / 5) % 5) as f32 * 0.7 - 1.4,
            );
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = spawn_lump(&mut c, at, k);
            q.apply(app.world_mut());
            e
        };
        for _ in 0..48 {
            bodies.push(spawn(&mut app, &mut rng));
        }
        for _ in 0..90 {
            app.update();
        }
        for tick in 0..1800 {
            for _ in 0..6 {
                let i = rng.next() as usize % bodies.len();
                let e = bodies[i];
                let child = app
                    .world()
                    .get::<Children>(e)
                    .and_then(|c| c.first().copied());
                let w = app.world_mut();
                let op = rng.next() % 11;
                match op {
                    0 => {
                        if w.get::<Sensor>(e).is_some() {
                            w.entity_mut(e).remove::<Sensor>();
                        } else {
                            w.entity_mut(e).insert(Sensor);
                        }
                    }
                    1 => {
                        if w.get::<RigidBodyDisabled>(e).is_some() {
                            w.entity_mut(e).remove::<RigidBodyDisabled>();
                        } else {
                            w.entity_mut(e).insert(RigidBodyDisabled);
                        }
                    }
                    2 => {
                        // How a seated or stowed item goes: body and
                        // colliders off together, and back on together.
                        let kids: Vec<Entity> = w
                            .get::<Children>(e)
                            .map(|c| c.iter().collect())
                            .unwrap_or_default();
                        if w.get::<RigidBodyDisabled>(e).is_some() {
                            w.entity_mut(e)
                                .remove::<(RigidBodyDisabled, ColliderDisabled)>();
                            for k in kids {
                                w.entity_mut(k).remove::<ColliderDisabled>();
                            }
                        } else {
                            w.entity_mut(e)
                                .insert((RigidBodyDisabled, ColliderDisabled));
                            for k in kids {
                                w.entity_mut(k).insert(ColliderDisabled);
                            }
                        }
                    }
                    3 => {
                        let rb = match rng.next() % 3 {
                            0 => RigidBody::Static,
                            1 => RigidBody::Kinematic,
                            _ => RigidBody::Dynamic,
                        };
                        w.entity_mut(e).insert(rb);
                    }
                    4 => {
                        let layers = if rng.next() % 2 == 0 {
                            CollisionLayers::new(Layer::Stowed, LayerMask::NONE)
                        } else {
                            CollisionLayers::new(Layer::Item, [Layer::World, Layer::Item])
                        };
                        w.entity_mut(e).insert(layers);
                    }
                    5 => {
                        // A collider leaves its body and stays where it is.
                        if let Some(c) = child {
                            w.entity_mut(c).remove::<ChildOf>();
                        }
                    }
                    6 => {
                        if let Some(c) = child {
                            if w.get::<ColliderDisabled>(c).is_some() {
                                w.entity_mut(c).remove::<ColliderDisabled>();
                            } else {
                                w.entity_mut(c).insert(ColliderDisabled);
                            }
                        }
                    }
                    7 => {
                        w.entity_mut(e).despawn();
                        bodies[i] = spawn(&mut app, &mut rng);
                    }
                    8 => {
                        w.entity_mut(e).insert(Collider::cuboid(0.5, 0.5, 0.5));
                    }
                    9 => {
                        if let Some(c) = child {
                            w.entity_mut(c).insert(RigidBody::Dynamic);
                        }
                    }
                    _ => {
                        if let Some(mut v) = w.get_mut::<LinearVelocity>(e) {
                            v.0 += Vec3::new(0.0, 3.0, 0.0);
                        }
                    }
                }
            }
            // Keep the heap in a heap.
            if tick % 300 == 299 {
                for e in bodies.clone() {
                    if let Some(mut t) = app.world_mut().get_mut::<Transform>(e) {
                        t.translation =
                            Vec3::new(t.translation.x * 0.3, 1.0, t.translation.z * 0.3);
                    }
                }
            }
            app.update();
        }
    }
}
