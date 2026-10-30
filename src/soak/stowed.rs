//! What's put away stays put away (`docs/SOAK.md`, "Stowed things"): a
//! thing in a pack, a pouch or the crate stays there, with the pose it was
//! put in, through every move of the pack, pulling it out, snapping it
//! back, walking, and a save and load. The first headset night lost
//! rounds, a radio and a cassette from the pack, stood things upright, and
//! let things fall through the crate and under it: "the ceremony bag of
//! no return".
//!
//! Driven through the headset's hand path (`vr_harness`).

use bevy::prelude::*;

use crate::interact::body::{Backpack, InBackpack};
use crate::rig::Side;
use crate::vr_harness::Vr;

/// Each stowed item's pose relative to its bag.
fn poses(vr: &mut Vr, bag: Entity) -> Vec<(Entity, Transform)> {
    let w = vr.app.world();
    let b = w.get::<GlobalTransform>(bag).unwrap().affine();
    let contents = w.get::<Backpack>(bag).unwrap().contents.clone();
    contents
        .into_iter()
        .map(|e| {
            let g = w.get::<GlobalTransform>(e).unwrap().affine();
            (e, Transform::from_matrix((b.inverse() * g).into()))
        })
        .collect()
}

/// The worn pack nearest the player (the crate is a Backpack too).
fn worn_pack(vr: &mut Vr) -> Entity {
    let feet = vr.feet();
    vr.app
        .world_mut()
        .query::<(Entity, &Backpack, &GlobalTransform)>()
        .iter(vr.app.world())
        .filter(|(_, b, _)| b.carried)
        .min_by(|a, b| {
            a.2.translation()
                .distance(feet)
                .total_cmp(&b.2.translation().distance(feet))
        })
        .map(|(e, ..)| e)
        .expect("a pack on your back")
}

/// Takes `item` in `side`'s hand where it lies.
fn take(vr: &mut Vr, side: Side, item: Entity) {
    let at = vr
        .app
        .world()
        .get::<GlobalTransform>(item)
        .unwrap()
        .translation();
    vr.hand(side, at, Quat::IDENTITY, 0.0, 0.0);
    vr.step(2);
    vr.grip(side, 1.0);
    vr.step(3);
    assert_eq!(vr.hand_state(side).holding, Some(item), "took it");
}

/// Every one of `before` still in `bag`, at the same pose in it.
fn same(vr: &mut Vr, bag: Entity, before: &[(Entity, Transform)], when: &str) {
    let now = poses(vr, bag);
    let w = vr.app.world();
    for (e, t) in before {
        assert!(
            w.get::<InBackpack>(*e).is_some_and(|i| i.bag == bag),
            "{when}: {e} left the pack"
        );
        let Some((_, n)) = now.iter().find(|(x, _)| x == e) else {
            panic!("{when}: {e} isn't in the pack's list");
        };
        let moved = n.translation.distance(t.translation);
        let turned = n.rotation.angle_between(t.rotation).to_degrees();
        assert!(
            moved < 0.01 && turned < 2.0,
            "{when}: {e} moved {moved:.3} m and turned {turned:.1} degrees in the pack"
        );
    }
}

#[test]
fn what_goes_in_the_pack_stays_where_it_was_put() {
    let mut vr = Vr::first_run();
    let feet = vr.feet() + Vec3::new(0.0, 0.0, -6.0);
    let feet = Vec3::new(
        feet.x,
        vr.app
            .world()
            .resource::<crate::world::Zone>()
            .ground(feet.x, feet.z),
        feet.z,
    );
    vr.stand(feet, 0.0);
    vr.step(10);
    let bag = worn_pack(&mut vr);
    // Off the right shoulder, and out in front: it hangs open.
    let shoulder = feet + Vec3::new(0.15, 1.6, 0.12);
    vr.hand(Side::Right, shoulder, Quat::IDENTITY, 0.0, 0.0);
    vr.step(3);
    vr.grip(Side::Right, 1.0);
    vr.step(3);
    assert_eq!(vr.hand_state(Side::Right).holding, Some(bag));
    let front = feet + Vec3::new(0.0, 1.3, -0.45);
    for i in 0..=12 {
        vr.move_hand(Side::Right, shoulder.lerp(front, i as f32 / 12.0));
        vr.step(1);
    }
    vr.step(20);
    assert!(
        vr.app.world().get::<Backpack>(bag).unwrap().open,
        "open in front"
    );
    // Things of each size, put in at an angle.
    for (k, id) in [
        "ammo_545",
        "radio",
        "field_dressing",
        "ak_mag",
        "flashlight",
    ]
    .iter()
    .enumerate()
    {
        let lay = feet + Vec3::new(-0.4, 1.0, -0.3 - k as f32 * 0.05);
        let item = vr.spawn(id, Transform::from_translation(lay));
        vr.app
            .world_mut()
            .entity_mut(item)
            .insert(avian3d::prelude::RigidBody::Static);
        take(&mut vr, Side::Left, item);
        let into = vr
            .app
            .world()
            .get::<GlobalTransform>(bag)
            .unwrap()
            .transform_point(Vec3::new(-0.08 + k as f32 * 0.04, 0.0, 0.0));
        let tilt = Quat::from_euler(EulerRot::XYZ, 0.6, 0.3 * k as f32, 0.9);
        vr.hand(Side::Left, into, tilt, 1.0, 0.0);
        vr.step(3);
        vr.grip(Side::Left, 0.0);
        vr.step(3);
        assert!(
            vr.app.world().get::<InBackpack>(item).is_some(),
            "{id} went in the pack"
        );
    }
    let put = poses(&mut vr, bag);
    assert_eq!(put.len(), 5);
    // The pose they were put in, not stood upright.
    let tilted = put
        .iter()
        .filter(|(_, t)| t.rotation.angle_between(Quat::IDENTITY) > 0.3)
        .count();
    assert_eq!(tilted, 5, "kept the angle they were put in at: {put:?}");
    // Swing it about, flip it over and back.
    for i in 0..90 {
        let a = i as f32 * 0.15;
        let at = front + Vec3::new(a.sin() * 0.3, (a * 0.7).cos() * 0.2, 0.0);
        let rot = Quat::from_rotation_z(a.sin() * 2.5);
        vr.hand(Side::Right, at, rot, 1.0, 0.0);
        vr.step(1);
    }
    vr.hand(Side::Right, front, Quat::IDENTITY, 1.0, 0.0);
    vr.step(20);
    same(&mut vr, bag, &put, "swung and flipped");
    // Let go in front (it hangs), walk off, take it again, put it on.
    vr.grip(Side::Right, 0.0);
    vr.step(10);
    same(&mut vr, bag, &put, "hanging");
    let on = feet + Vec3::new(3.0, 0.0, 2.0);
    let on = Vec3::new(
        on.x,
        vr.app
            .world()
            .resource::<crate::world::Zone>()
            .ground(on.x, on.z),
        on.z,
    );
    vr.stand(on, 0.5);
    vr.step(20);
    same(&mut vr, bag, &put, "walked off with it hanging");
    // By its top edge (a hand in the middle of the open pack takes what's
    // in it, as it should).
    let at = {
        let w = vr.app.world();
        let g = w.get::<GlobalTransform>(bag).unwrap();
        let half = w.get::<Backpack>(bag).unwrap().half;
        g.transform_point(Vec3::new(0.0, half.y + 0.03, 0.0))
    };
    vr.hand(Side::Right, at, Quat::IDENTITY, 0.0, 0.0);
    vr.step(3);
    vr.grip(Side::Right, 1.0);
    vr.step(3);
    assert_eq!(
        vr.hand_state(Side::Right).holding,
        Some(bag),
        "took it again"
    );
    let shoulder = vr.feet() + Vec3::new(0.15, 1.6, 0.12);
    for i in 0..=12 {
        vr.move_hand(Side::Right, at.lerp(shoulder, i as f32 / 12.0));
        vr.step(1);
    }
    vr.grip(Side::Right, 0.0);
    vr.step(10);
    same(&mut vr, bag, &put, "snapped on");
    // Walk with it on.
    for k in 0..5 {
        let p = vr.feet() + Vec3::new(1.0, 0.0, 0.0);
        let p = Vec3::new(
            p.x,
            vr.app
                .world()
                .resource::<crate::world::Zone>()
                .ground(p.x, p.z),
            p.z,
        );
        vr.stand(p, k as f32);
        vr.step(15);
    }
    same(&mut vr, bag, &put, "walked with it on");
}

/// Saved and loaded, the pack and the crate hold the same things, lying
/// as they were put in (a save kept where a thing was in its bag, not how
/// it lay: everything came back stood upright).
#[test]
fn a_save_keeps_what_is_in_the_pack_and_the_crate_as_it_lay() {
    use crate::raid::truck::Stash;
    let dir = std::env::temp_dir().join(format!("extraction-stowed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("career.ron");
    let game = |path: &std::path::Path| {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                save: Some(path.to_path_buf()),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        for _ in 0..5 {
            app.update();
        }
        app
    };
    let mut a = game(&path);
    let (pack, crate_) = {
        let w = a.world_mut();
        let pack = w
            .query::<(Entity, &Backpack)>()
            .iter(w)
            .find(|(_, b)| b.carried)
            .map(|(e, _)| e)
            .unwrap();
        let crate_ = w
            .query_filtered::<Entity, With<Stash>>()
            .iter(w)
            .next()
            .unwrap();
        (pack, crate_)
    };
    let catalog = a.world().resource::<crate::items::Catalog>().clone();
    let mut put = Vec::new();
    for (k, (id, bag)) in [
        ("ammo_545", pack),
        ("radio", pack),
        ("cassette", pack),
        ("ak_mag", crate_),
        ("flashlight", crate_),
    ]
    .into_iter()
    .enumerate()
    {
        let Some(kind) = catalog.kind(id) else {
            continue;
        };
        let pose = Transform::from_xyz(-0.1 + 0.05 * k as f32, 0.05, 0.02)
            .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.7, 0.2 * k as f32, -0.5));
        let mut q =
            bevy::ecs::system::SystemState::<(Commands, Query<&mut Backpack>)>::new(a.world_mut());
        let (mut c, mut bags) = q.get_mut(a.world_mut()).unwrap();
        let e = crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::IDENTITY,
                condition: 1.0,
            },
        );
        let mut b = bags.get_mut(bag).unwrap();
        crate::interact::body::stow_posed(&mut c, e, bag, &mut b, pose);
        q.apply(a.world_mut());
        put.push((id, bag == pack, pose));
    }
    for _ in 0..5 {
        a.update();
    }
    let save = crate::persist::snapshot(a.world_mut());
    let mut career = a
        .world()
        .resource::<crate::persist::CareerState>()
        .0
        .clone();
    career.store(&save);
    crate::persist::write_career(&path, &career);
    drop(a);
    let mut b = game(&path);
    let w = b.world_mut();
    let catalog = w.resource::<crate::items::Catalog>().clone();
    let held: Vec<(String, bool, Transform)> = w
        .query::<(&crate::items::Item, &InBackpack, &Transform)>()
        .iter(w)
        .filter_map(|(it, inb, t)| {
            let bag = w.get::<Backpack>(inb.bag)?;
            Some((catalog.get(it.kind()).id.clone(), bag.carried, *t))
        })
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    for (id, in_pack, pose) in &put {
        let found = held.iter().find(|(i, p, t)| {
            i == id
                && p == in_pack
                && t.translation.distance(pose.translation) < 0.01
                && t.rotation.angle_between(pose.rotation) < 0.03
        });
        assert!(
            found.is_some(),
            "{id} came back in the {} as it lay: {held:?}",
            if *in_pack { "pack" } else { "crate" }
        );
    }
}
