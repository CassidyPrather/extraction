//! Animals have bodies: a shot or a blast can kill them, and what's left
//! is the Custodian's to keep.
//!
//! Each bird, deer and dog carries a collider that follows it
//! ([`AnimalBody`]): a member of the robots' layer that collides with
//! nothing, so bullets, fragments and blasts (which look for robots) hit
//! it and nobody bumps into it. A hit kills: a bird drops off its perch or
//! out of the sky and lies where it fell, a deer or dog falls where it
//! stood (`flock`, `herd`). Every death leaves a [`Carcass`], an entity
//! of its own at the body, for the machines: the Custodian keeps
//! everything, "animals included" (`docs/LORE.md`), and a robot that
//! takes the carcass (despawns it) takes the body with it. [`Killed`]
//! says so for sound and the sim.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

use super::flock::{BirdState, Flock, Species};
use super::herd::{Herd, Kind, State};
use crate::world::Layer;

/// What a carcass was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarcassKind {
    Bird,
    Heron,
    Deer,
    Dog,
}

/// A dead animal, for the machines to collect: despawn it and the body
/// goes too.
#[derive(Component, Clone, Copy, Debug)]
pub struct Carcass {
    pub kind: CarcassKind,
    pub at: Vec3,
}

/// An animal died: shot (`by` the shooter), caught by a hound, or blown up.
#[derive(Message, Clone, Copy, Debug)]
pub struct Killed {
    pub at: Vec3,
    pub kind: CarcassKind,
    pub by: Option<Entity>,
}

/// The collider that's an animal's body: which flock or herd, which one.
#[derive(Component, Clone, Copy, Debug)]
pub struct AnimalBody {
    pub owner: Entity,
    pub index: usize,
}

/// Bodies have been made for this flock or herd.
#[derive(Component)]
struct Bodied;

/// Hit here, by this shooter: this tick's kills, and who for the record.
#[derive(Resource, Default)]
struct Shots(Vec<(Entity, usize, Option<Entity>)>);

pub struct BodiesPlugin;

impl Plugin for BodiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Killed>()
            .init_resource::<Shots>()
            .add_systems(
                FixedUpdate,
                (give_bodies, take_hits, deaths, follow)
                    .chain()
                    .after(super::WildlifeSystems)
                    .after(crate::weapons::ballistics::BallisticsSystems),
            );
    }
}

fn layers() -> CollisionLayers {
    // Found by bullets and blasts (they look for robots); collides with
    // nothing.
    CollisionLayers::new(Layer::Robot, LayerMask::NONE)
}

fn give_bodies(
    mut commands: Commands,
    flocks: Query<(Entity, &Flock), Without<Bodied>>,
    herds: Query<(Entity, &Herd), Without<Bodied>>,
) {
    for (e, f) in &flocks {
        let r = if f.species == Species::Heron {
            0.22
        } else {
            0.13 * f.species.size()
        };
        for (i, b) in f.birds.iter().enumerate() {
            commands.spawn((
                Name::new("Bird body"),
                AnimalBody { owner: e, index: i },
                Transform::from_translation(b.pos),
                RigidBody::Kinematic,
                Collider::sphere(r),
                layers(),
            ));
        }
        commands.entity(e).insert(Bodied);
    }
    for (e, h) in &herds {
        let (w, hgt, l) = match h.kind {
            Kind::Deer => (0.3, 0.45, 1.0),
            Kind::Dog => (0.24, 0.35, 0.8),
        };
        for (i, b) in h.beasts.iter().enumerate() {
            commands.spawn((
                Name::new("Animal body"),
                AnimalBody { owner: e, index: i },
                Transform::from_translation(b.pos),
                RigidBody::Kinematic,
                Collider::cuboid(w, hgt, l),
                layers(),
            ));
        }
        commands.entity(e).insert(Bodied);
    }
}

/// Bullets and blasts that found an animal kill it.
fn take_hits(
    mut hits: MessageReader<crate::weapons::ballistics::Hit>,
    bodies: Query<&AnimalBody>,
    mut flocks: Query<&mut Flock>,
    mut herds: Query<&mut Herd>,
    mut shots: ResMut<Shots>,
) {
    for h in hits.read() {
        let Ok(body) = bodies.get(h.entity) else {
            continue;
        };
        if let Ok(mut f) = flocks.get_mut(body.owner) {
            if let Some(b) = f.birds.get_mut(body.index).filter(|b| b.state.alive()) {
                b.state = BirdState::Dead;
                // Knocked along by the round, a little.
                b.vel = b.vel * 0.5 + h.velocity.normalize_or_zero() * 1.5;
                shots.0.push((body.owner, body.index, h.shooter));
            }
        }
        if let Ok(mut herd) = herds.get_mut(body.owner) {
            if let Some(b) = herd.beasts.get_mut(body.index).filter(|b| b.alive()) {
                b.state = State::Dead;
                b.vel = Vec3::ZERO;
                shots.0.push((body.owner, body.index, h.shooter));
            }
        }
    }
}

/// Every death, however it came (a shot, a hound), leaves a carcass; a
/// carcass taken away takes the body.
fn deaths(
    mut commands: Commands,
    mut flocks: Query<(Entity, &mut Flock)>,
    mut herds: Query<(Entity, &mut Herd)>,
    carcasses: Query<(), With<Carcass>>,
    mut shots: ResMut<Shots>,
    mut known: Local<HashMap<(Entity, usize), Entity>>,
    mut killed: MessageWriter<Killed>,
) {
    let mut by: HashMap<(Entity, usize), Option<Entity>> =
        shots.0.drain(..).map(|(o, i, s)| ((o, i), s)).collect();
    let mut dead =
        |owner: Entity, i: usize, at: Vec3, kind: CarcassKind, commands: &mut Commands| -> bool {
            match known.get(&(owner, i)) {
                // Taken away?
                Some(c) => !carcasses.contains(*c),
                None => {
                    let c = commands
                        .spawn((
                            Name::new("Carcass"),
                            Carcass { kind, at },
                            Transform::from_translation(at),
                        ))
                        .id();
                    known.insert((owner, i), c);
                    killed.write(Killed {
                        at,
                        kind,
                        by: by.remove(&(owner, i)).flatten(),
                    });
                    false
                }
            }
        };
    for (e, mut f) in &mut flocks {
        let kind = if f.species == Species::Heron {
            CarcassKind::Heron
        } else {
            CarcassKind::Bird
        };
        for (i, b) in f.birds.iter_mut().enumerate() {
            // Placed when it's down (it may still be falling).
            if b.state == BirdState::Dead
                && b.vel == Vec3::ZERO
                && dead(e, i, b.pos, kind, &mut commands)
            {
                b.state = BirdState::Gone;
            }
        }
    }
    for (e, mut h) in &mut herds {
        let kind = match h.kind {
            Kind::Deer => CarcassKind::Deer,
            Kind::Dog => CarcassKind::Dog,
        };
        for (i, b) in h.beasts.iter_mut().enumerate() {
            if b.state == State::Dead && !b.gone && dead(e, i, b.pos, kind, &mut commands) {
                b.gone = true;
            }
        }
    }
}

/// Bodies follow their animals; the living only (the dead are carcasses).
fn follow(
    mut commands: Commands,
    mut bodies: Query<(Entity, &AnimalBody, &mut Transform)>,
    flocks: Query<&Flock>,
    herds: Query<&Herd>,
) {
    for (e, body, mut t) in &mut bodies {
        let place = if let Ok(f) = flocks.get(body.owner) {
            f.birds
                .get(body.index)
                .filter(|b| b.state.alive())
                .map(|b| Transform::from_translation(b.pos + Vec3::Y * 0.12 * f.species.size()))
        } else if let Ok(h) = herds.get(body.owner) {
            let ride = super::herd_look::build(h.kind).ride;
            h.beasts.get(body.index).filter(|b| b.alive()).map(|b| {
                Transform::from_translation(b.pos + Vec3::Y * ride)
                    .with_rotation(Quat::from_rotation_y(-b.yaw - std::f32::consts::FRAC_PI_2))
            })
        } else {
            None
        };
        match place {
            Some(p) => *t = p,
            None => {
                commands.entity(e).despawn();
            }
        }
    }
}
