//! The ways interact switches an item's physics off and on (socketing,
//! stowing, taking out), run against Avian on a pile of touching bodies.
//!
//! Avian panics ("Neither body X nor Y is in an island") when a touching
//! contact joins two bodies that are both out of the islands. These paths
//! insert `RigidBodyDisabled`, `ColliderDisabled` and `ChildOf` in one go
//! on bodies that may be touching others. That's safe: Avian drops a
//! body's contacts as it's disabled, and contacts with a disabled body
//! make no constraints. This test keeps it so (an Avian upgrade could
//! change it).

use avian3d::prelude::*;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;

use super::body::{Backpack, stow_in_backpack};
use super::{Socket, SocketKind, physics_on, seat_in_socket};
use extraction_core::inventory::SlotKind;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        bevy::asset::AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        PhysicsPlugins::default(),
    ))
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f32(1.0 / 60.0),
    ));
    app.finish();
    app.cleanup();
    app
}

fn run(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn cube(app: &mut App, at: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::cuboid(0.2, 0.2, 0.2),
            Transform::from_translation(at),
        ))
        .id()
}

/// A pile of boxes on the ground, and a "gun" (a dynamic body) lying
/// across them with a socket on it. Each box in turn: seated in the socket
/// while touching the pile, taken back out into the pile, stowed in a bag,
/// taken out again, over and over. Avian must neither panic nor lose them.
#[test]
fn switching_physics_off_and_on_in_a_pile_is_safe() {
    let mut app = app();
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(6.0, 0.2, 6.0),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    let boxes: Vec<Entity> = (0..6)
        .map(|i| {
            cube(
                &mut app,
                Vec3::new((i % 3) as f32 * 0.21, 0.1 + (i / 3) as f32 * 0.21, 0.0),
            )
        })
        .collect();
    let gun = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::cuboid(0.8, 0.06, 0.1),
            Transform::from_xyz(0.2, 0.5, 0.0),
        ))
        .id();
    let socket = app
        .world_mut()
        .spawn((
            Socket {
                kind: SocketKind::Body(SlotKind::Pouch),
                occupant: None,
                radius: 0.1,
                seat: Transform::IDENTITY,
            },
            Transform::from_xyz(0.3, 0.05, 0.0),
            ChildOf(gun),
        ))
        .id();
    let bag = app
        .world_mut()
        .spawn((
            Backpack {
                owner: Entity::PLACEHOLDER,
                open: true,
                model: Default::default(),
                contents: Vec::new(),
                carried: false,
                half: Vec3::splat(0.2),
            },
            RigidBody::Dynamic,
            RigidBodyDisabled,
            ColliderDisabled,
            Collider::cuboid(0.4, 0.4, 0.4),
            Transform::from_xyz(-0.3, 0.2, 0.0),
        ))
        .id();
    // Let the pile come to rest (touching, some asleep).
    run(&mut app, 120);

    let mut state =
        SystemState::<(Commands, Query<&mut Socket>, Query<&mut Backpack>)>::new(app.world_mut());
    for round in 0..3 {
        for &b in &boxes {
            // Into the socket, from the pile.
            {
                let (mut c, mut sockets, _) = state.get_mut(app.world_mut()).unwrap();
                let mut s = sockets.get_mut(socket).unwrap();
                seat_in_socket(&mut c, b, socket, &mut s);
            }
            state.apply(app.world_mut());
            run(&mut app, 2);
            // Back out, dropped onto the pile.
            {
                let (mut c, mut sockets, _) = state.get_mut(app.world_mut()).unwrap();
                sockets.get_mut(socket).unwrap().occupant = None;
                let at = Transform::from_xyz(0.21, 0.32 + round as f32 * 0.01, 0.0);
                physics_on(&mut c, b, at, Vec3::ZERO, Vec3::ZERO, true);
            }
            state.apply(app.world_mut());
            run(&mut app, 2);
            // Into the bag while touching, and out again.
            {
                let (mut c, _, mut bags) = state.get_mut(app.world_mut()).unwrap();
                let mut pack = bags.get_mut(bag).unwrap();
                stow_in_backpack(&mut c, b, bag, &mut pack, Vec3::ZERO);
            }
            state.apply(app.world_mut());
            run(&mut app, 2);
            {
                let (mut c, _, mut bags) = state.get_mut(app.world_mut()).unwrap();
                bags.get_mut(bag).unwrap().contents.retain(|e| *e != b);
                let at = Transform::from_xyz(0.0, 0.3, 0.0);
                physics_on(&mut c, b, at, Vec3::ZERO, Vec3::ZERO, true);
            }
            state.apply(app.world_mut());
            run(&mut app, 3);
        }
    }
    run(&mut app, 120);
    // Nothing fell through the ground or flew off.
    for &b in &boxes {
        let p = app.world().get::<Position>(b).unwrap().0;
        assert!(p.y > -0.05 && p.length() < 3.0, "box {b} ended at {p}");
    }
}
