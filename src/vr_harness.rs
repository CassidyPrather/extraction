//! Tests through the headset's hand path: poses and grip/trigger values
//! into [`RigInput`], the way the OpenXR driver writes them, so reach,
//! collision layers and what the hand finds are all in play.
//!
//! The bots and most tests send `HandAction::Grab { item }` straight to an
//! entity. That skips everything a real hand has to get right, and the
//! first headset playtest found what it hid: doors that no hand could find,
//! a magazine the gun's collider kept out of the well, a cord whose toggle
//! was outside its collider. Whatever a person does with their hands gets a
//! test here.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::interact::{GripRole, Hand, InteractEvent};
use crate::items::{Catalog, SpawnItem, spawn_item};
use crate::rig::{LocalPlayer, RigDriver, RigHand, RigInput, Side};

/// What the hands did, collected each frame.
#[derive(Resource, Default)]
pub struct Seen(pub Vec<InteractEvent>);

fn collect(mut events: MessageReader<InteractEvent>, mut seen: ResMut<Seen>) {
    seen.0.extend(events.read().cloned());
}

pub struct Vr {
    pub app: App,
    pub player: Entity,
}

impl Vr {
    /// A headless game on `map` with the rig driven as by a headset.
    pub fn new(map: &str) -> Self {
        let opts = crate::LaunchOptions {
            robots: false,
            map: map.into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.init_resource::<Seen>().add_systems(Update, collect);
        app.finish();
        app.cleanup();
        app.update();
        app.insert_resource(RigDriver::Xr);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .expect("a local player");
        let mut vr = Self { app, player };
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        vr.step(3);
        vr
    }

    /// Stand the playspace at `at` (feet), facing `yaw`.
    pub fn stand(&mut self, at: Vec3, yaw: f32) {
        let mut t = self
            .app
            .world_mut()
            .get_mut::<Transform>(self.player)
            .unwrap();
        t.translation = at;
        t.rotation = Quat::from_rotation_y(yaw);
        self.step(2);
    }

    fn root(&self) -> Transform {
        *self.app.world().get::<Transform>(self.player).unwrap()
    }

    /// The head, in the playspace.
    pub fn head(&mut self, local: Vec3) {
        self.app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap()
            .head = Transform::from_translation(local);
    }

    /// A hand at a world pose, pointing `-Z` of `rot`, with its controls.
    pub fn hand(&mut self, side: Side, world: Vec3, rot: Quat, grip: f32, trigger: f32) {
        let root = self.root();
        let inv = root.compute_affine().inverse();
        let local = Transform::from_matrix(
            (inv * Transform::from_translation(world)
                .with_rotation(rot)
                .compute_affine())
            .into(),
        );
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        let h = input.hand_mut(side);
        h.pose = local;
        h.tracked = true;
        h.grip = grip;
        h.trigger = trigger;
    }

    /// Move a hand to `world`, keeping its rotation and controls.
    pub fn move_hand(&mut self, side: Side, world: Vec3) {
        let (rot, grip, trigger) = {
            let input = self.app.world().get::<RigInput>(self.player).unwrap();
            let h = input.hand(side);
            (self.root().rotation * h.pose.rotation, h.grip, h.trigger)
        };
        self.hand(side, world, rot, grip, trigger);
    }

    pub fn grip(&mut self, side: Side, v: f32) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        input.hand_mut(side).grip = v;
    }

    pub fn trigger(&mut self, side: Side, v: f32) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        input.hand_mut(side).trigger = v;
    }

    pub fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
        }
    }

    pub fn hand_state(&mut self, side: Side) -> Hand {
        let mut q = self.app.world_mut().query::<(&RigHand, &Hand)>();
        q.iter(self.app.world())
            .find(|(rh, _)| rh.side == side)
            .map(|(_, h)| h.clone())
            .expect("a hand")
    }

    pub fn spawn(&mut self, id: &str, at: Transform) -> Entity {
        let catalog = self.app.world().resource::<Catalog>().clone();
        let kind = catalog.kind(id).expect("an item id");
        let mut commands = self.app.world_mut().commands();
        let e = spawn_item(
            &mut commands,
            &catalog,
            SpawnItem {
                kind,
                transform: at,
                condition: 1.0,
            },
        );
        self.app.world_mut().flush();
        self.step(2);
        e
    }

    /// World pose of an item's grab point of `role`.
    pub fn point(&self, item: Entity, role: impl Fn(&GripRole) -> bool) -> Vec3 {
        let gt = self.app.world().get::<GlobalTransform>(item).unwrap();
        let g = self
            .app
            .world()
            .get::<crate::interact::Grabbable>(item)
            .unwrap();
        let p = g
            .points
            .iter()
            .find(|p| role(&p.role))
            .expect("a grab point");
        gt.mul_transform(p.local).translation()
    }

    pub fn pressed(&self) -> Vec<Entity> {
        self.app
            .world()
            .resource::<Seen>()
            .0
            .iter()
            .filter_map(|e| match e {
                InteractEvent::Pressed { item, .. } => Some(*item),
                _ => None,
            })
            .collect()
    }
}

mod tests {
    use super::*;
    use crate::weapons::Gun;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    fn on_ground(vr: &Vr, x: f32, z: f32) -> Vec3 {
        vr.app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(x, z)
    }

    /// Reach a hand to an item resting in front of you and squeeze: it's
    /// in your hand; open it: it falls.
    #[test]
    fn the_grip_takes_what_the_hand_is_on() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let at = feet + Vec3::new(0.2, 1.0, -0.4);
        let item = vr.spawn("pm_mag", Transform::from_translation(at));
        vr.app
            .world_mut()
            .entity_mut(item)
            .insert(RigidBody::Static);
        vr.hand(Side::Right, at, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(item));
        vr.grip(Side::Right, 0.0);
        vr.step(3);
        assert_eq!(vr.hand_state(Side::Right).holding, None);
    }

    /// Pistol in the right hand; the left hand's trigger takes the slide
    /// and pulls it back. The grip alone never grabs the slide.
    #[test]
    fn the_trigger_works_the_slide() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let at = feet + Vec3::new(0.15, 1.1, -0.35);
        let gun = vr.spawn("pm_pistol", Transform::from_translation(at));
        let grip_at = vr.point(gun, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip_at, Quat::IDENTITY, 1.0, 0.0);
        vr.step(4);
        assert_eq!(
            vr.hand_state(Side::Right).holding,
            Some(gun),
            "pistol in hand"
        );
        vr.step(10);
        let slide = vr.point(gun, |r| matches!(r, GripRole::Handle(_)));
        vr.hand(Side::Left, slide, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.trigger(Side::Left, 1.0);
        vr.step(2);
        assert!(
            vr.hand_state(Side::Left).handle.is_some(),
            "the left trigger took the slide"
        );
        let back = vr.app.world().get::<GlobalTransform>(gun).unwrap().back();
        for i in 1..=10 {
            vr.move_hand(Side::Left, slide + back * (0.004 * i as f32));
            vr.step(1);
        }
        let travel = vr.app.world().get::<Gun>(gun).unwrap().firearm.bolt_travel;
        assert!(travel > 0.5, "slide back ({travel})");
        vr.trigger(Side::Left, 0.0);
        vr.step(2);
        assert!(vr.hand_state(Side::Left).handle.is_none(), "let go");
    }

    /// A door in the yard opens to a hand squeezed on its leaf.
    #[test]
    fn a_hand_opens_a_door() {
        let mut vr = Vr::new("training_yard");
        let doors: Vec<(Entity, crate::world::buildings::DoorLeaf, GlobalTransform)> = vr
            .app
            .world_mut()
            .query::<(Entity, &crate::world::buildings::DoorLeaf, &GlobalTransform)>()
            .iter(vr.app.world())
            .map(|(e, d, g)| (e, *d, *g))
            .collect();
        let (door, leaf, gt) = doors.first().cloned().expect("a door in the yard");
        let handle = gt.transform_point(Vec3::new(leaf.0.width - 0.12, 1.0, 0.0));
        let out = gt.rotation() * Vec3::Z;
        let feet = on_ground(&vr, handle.x + out.x * 0.6, handle.z + out.z * 0.6);
        vr.stand(feet, 0.0);
        vr.hand(Side::Right, handle, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        assert!(vr.pressed().contains(&door), "the door answered the hand");
        vr.grip(Side::Right, 0.0);
        vr.step(60);
        let d = vr
            .app
            .world()
            .get::<crate::world::buildings::Door>(door)
            .unwrap();
        assert!(d.open, "the door is open");
    }

    /// Bring a magazine up into the well of the pistol in your other hand
    /// and let go: it seats.
    #[test]
    fn a_magazine_goes_in_the_well() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let at = feet + Vec3::new(0.15, 1.1, -0.35);
        let gun = vr.spawn("pm_pistol", Transform::from_translation(at));
        let grip_at = vr.point(gun, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip_at, Quat::IDENTITY, 1.0, 0.0);
        vr.step(15);
        let mag_at = feet + Vec3::new(-0.2, 1.0, -0.35);
        let mag = vr.spawn("pm_mag", Transform::from_translation(mag_at));
        vr.app.world_mut().entity_mut(mag).insert(RigidBody::Static);
        vr.hand(Side::Left, mag_at, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Left, 1.0);
        vr.step(3);
        assert_eq!(vr.hand_state(Side::Left).holding, Some(mag));
        // Up from below the grip, into the well.
        let well = {
            let g = vr.app.world().get::<Gun>(gun).unwrap();
            let w = g.magwell.expect("a magwell");
            vr.app
                .world()
                .get::<GlobalTransform>(w)
                .unwrap()
                .translation()
        };
        let start = well - Vec3::Y * 0.25;
        for i in 0..=30 {
            vr.move_hand(Side::Left, start.lerp(well, i as f32 / 30.0));
            vr.step(1);
        }
        vr.step(5);
        vr.grip(Side::Left, 0.0);
        vr.step(5);
        let seated = vr
            .app
            .world()
            .get::<crate::interact::InSocket>(mag)
            .is_some();
        assert!(seated, "the magazine is seated");
    }

    /// The training yard's cord answers a hand on its wooden toggle, the
    /// part you'd take hold of.
    #[test]
    fn the_cord_answers_a_hand_on_its_toggle() {
        let mut vr = Vr::new("training_yard");
        let pulls: Vec<(Entity, GlobalTransform)> = vr
            .app
            .world_mut()
            .query::<(Entity, &crate::raid::truck::Pressable, &GlobalTransform)>()
            .iter(vr.app.world())
            .filter(|(_, p, _)| **p == crate::raid::truck::Pressable::BellPull)
            .map(|(e, _, g)| (e, *g))
            .collect();
        let (pull, gt) = pulls.first().cloned().expect("a cord in the truck");
        let toggle = gt.transform_point(Vec3::new(0.0, -0.26, 0.0));
        vr.hand(Side::Right, toggle, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        assert!(vr.pressed().contains(&pull), "the cord answered");
    }

    /// Reach over your shoulder and squeeze: you've got your pack.
    #[test]
    fn the_pack_comes_off_over_the_shoulder() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.step(10);
        let bag = vr
            .app
            .world_mut()
            .query_filtered::<Entity, With<crate::interact::body::Backpack>>()
            .iter(vr.app.world())
            .next()
            .expect("a backpack");
        // Over the right shoulder, a hand's width behind the head.
        let at = feet + Vec3::new(0.15, 1.6, 0.12);
        vr.hand(Side::Right, at, Quat::IDENTITY, 0.0, 0.0);
        vr.step(3);
        assert_eq!(
            vr.hand_state(Side::Right).hover,
            Some(bag),
            "the bag is in reach"
        );
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(bag));
    }

    /// Something in your hand, over the shoulder, let go: it's put away,
    /// not dropped.
    #[test]
    fn letting_go_over_the_shoulder_puts_it_away() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let at = feet + Vec3::new(0.25, 1.0, -0.4);
        let item = vr.spawn("field_dressing", Transform::from_translation(at));
        vr.app
            .world_mut()
            .entity_mut(item)
            .insert(RigidBody::Static);
        vr.hand(Side::Right, at, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(item));
        let shoulder = feet + Vec3::new(0.15, 1.6, 0.12);
        for i in 0..=10 {
            vr.move_hand(Side::Right, at.lerp(shoulder, i as f32 / 10.0));
            vr.step(1);
        }
        vr.grip(Side::Right, 0.0);
        vr.step(4);
        let world = vr.app.world();
        let away = world
            .get::<crate::interact::body::InBackpack>(item)
            .is_some()
            || world.get::<crate::interact::InSocket>(item).is_some();
        assert!(away, "put away");
        assert_eq!(vr.hand_state(Side::Right).holding, None);
    }

    /// The steepest walkable open ground near `around`.
    fn slope_near(vr: &Vr, around: Vec2) -> (Vec2, f32) {
        let zone = vr.app.world().resource::<crate::world::Zone>();
        let mut best = (around, 0.0);
        for i in -20..=20 {
            for j in -20..=20 {
                let p = around + Vec2::new(i as f32, j as f32) * 2.0;
                let s = zone.hm.slope_deg([p.x, p.y]);
                if s > best.1 && s < 28.0 && zone.hm.road_at([p.x, p.y]).weight == 0 {
                    best = (p, s);
                }
            }
        }
        best
    }

    fn feet(vr: &Vr) -> Vec3 {
        vr.app
            .world()
            .get::<Transform>(vr.player)
            .unwrap()
            .translation
    }

    /// Walk down a hillside with the stick and let go: you stop, and stay
    /// stopped (the first playtest crept downhill for good).
    #[test]
    fn stopping_on_a_slope_stays_put() {
        let mut vr = Vr::new("marrow_creek");
        let (at, deg) = slope_near(&vr, Vec2::new(40.0, 280.0));
        assert!(deg > 8.0, "a slope ({deg} deg)");
        let n = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .hm
            .normal([at.x, at.y]);
        let down = Vec2::new(n[0], n[2]).normalize();
        let yaw = (-down.x).atan2(-down.y);
        let g = on_ground(&vr, at.x - down.x * 1.5, at.y - down.y * 1.5);
        vr.stand(g + Vec3::Y * 0.05, yaw);
        vr.step(30);
        vr.app
            .world_mut()
            .get_mut::<RigInput>(vr.player)
            .unwrap()
            .locomotion = Vec2::Y;
        vr.step(60);
        vr.app
            .world_mut()
            .get_mut::<RigInput>(vr.player)
            .unwrap()
            .locomotion = Vec2::ZERO;
        vr.step(30);
        let stopped = feet(&vr);
        vr.step(240);
        let after = feet(&vr);
        let drift = Vec2::new(after.x - stopped.x, after.z - stopped.z).length();
        assert!(drift < 0.03, "crept {drift:.3} m on a {deg:.0} deg slope");
    }

    /// Walk up a hillside in the room, no stick: the playspace stays where
    /// it is (being pushed out of the slope shoved it downhill).
    #[test]
    fn walking_uphill_in_the_room_keeps_the_playspace() {
        let mut vr = Vr::new("marrow_creek");
        let (at, deg) = slope_near(&vr, Vec2::new(40.0, 280.0));
        assert!(deg > 8.0, "a slope ({deg} deg)");
        let n = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .hm
            .normal([at.x, at.y]);
        let up = -Vec2::new(n[0], n[2]).normalize();
        let yaw = (-up.x).atan2(-up.y);
        vr.stand(on_ground(&vr, at.x, at.y) + Vec3::Y * 0.05, yaw);
        vr.step(30);
        let before = feet(&vr);
        // A metre uphill over two seconds (local -Z faces uphill).
        for i in 0..=120 {
            vr.head(Vec3::new(0.0, 1.65, -(i as f32) / 120.0));
            vr.step(1);
        }
        vr.step(10);
        let after = feet(&vr);
        let drift = Vec2::new(after.x - before.x, after.z - before.z).length();
        assert!(
            drift < 0.05,
            "playspace moved {drift:.3} m on a {deg:.0} deg slope"
        );
    }

    /// Step up into the back of the truck in the room, no stick: you end up
    /// standing on its bed.
    #[test]
    fn stepping_into_the_truck_in_the_room() {
        let mut vr = Vr::new("training_yard");
        let truck = {
            let mut q = vr
                .app
                .world_mut()
                .query_filtered::<&GlobalTransform, With<crate::raid::truck::Truck>>();
            q.iter(vr.app.world())
                .next()
                .expect("a truck")
                .compute_transform()
        };
        let behind = truck.transform_point(Vec3::new(0.0, 0.0, 3.0));
        let ground = on_ground(&vr, behind.x, behind.z);
        let yaw = truck.rotation.to_euler(EulerRot::YXZ).0;
        vr.stand(ground + Vec3::Y * 0.05, yaw);
        vr.step(30);
        for i in 0..=150 {
            vr.head(Vec3::new(0.0, 1.65, -1.6 * i as f32 / 150.0));
            vr.step(1);
        }
        vr.step(20);
        let bed = truck.translation.y + crate::raid::truck::FLOOR;
        let foot = feet(&vr).y;
        assert!(
            (foot - bed).abs() < 0.12,
            "standing on the bed (feet {foot:.2}, bed {bed:.2})"
        );
    }

    fn holster_y(vr: &mut Vr) -> f32 {
        let mut q = vr.app.world_mut().query::<(
            &crate::interact::Socket,
            &crate::interact::SocketOwner,
            &GlobalTransform,
        )>();
        q.iter(vr.app.world())
            .find(|(s, o, _)| {
                o.0 == vr.player
                    && s.kind
                        == crate::interact::SocketKind::Body(
                            extraction_core::inventory::SlotKind::Holster,
                        )
            })
            .map(|(_, _, g)| g.translation().y)
            .expect("a holster")
    }

    /// Bend down to reach your hip: the holster stays at your hip. Crouch
    /// right down: it comes down with you.
    #[test]
    fn bending_leaves_the_belt_where_it_is() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.7, 0.0));
        vr.step(60);
        let standing = holster_y(&mut vr) - feet.y;
        assert!(
            (0.75..1.0).contains(&standing),
            "holster at the hip ({standing:.2} m up)"
        );
        // Bending: head 35 cm lower and forward, looking down.
        vr.app
            .world_mut()
            .get_mut::<RigInput>(vr.player)
            .unwrap()
            .head = Transform::from_xyz(0.0, 1.35, -0.2).with_rotation(Quat::from_rotation_x(-0.9));
        vr.step(30);
        let bent = holster_y(&mut vr) - feet.y;
        assert!(
            (bent - standing).abs() < 0.02,
            "bending moved it {standing:.2} -> {bent:.2}"
        );
        // Crouching: head at 0.95.
        vr.head(Vec3::new(0.0, 0.95, 0.0));
        vr.step(30);
        let crouched = holster_y(&mut vr) - feet.y;
        assert!(
            crouched < standing - 0.2,
            "crouched {standing:.2} -> {crouched:.2}"
        );
    }

    /// Stand on a hillside swaying a little, as people do: the playspace
    /// stays put.
    #[test]
    fn swaying_on_a_slope_keeps_the_playspace() {
        let mut vr = Vr::new("marrow_creek");
        let (at, deg) = slope_near(&vr, Vec2::new(40.0, 280.0));
        assert!(deg > 8.0, "a slope ({deg} deg)");
        vr.stand(on_ground(&vr, at.x, at.y) + Vec3::Y * 0.05, 0.0);
        vr.step(30);
        let before = feet(&vr);
        for i in 0..600 {
            let t = i as f32 / 60.0;
            let sway = Vec3::new(
                0.03 * (t * 6.3).sin(),
                0.01 * (t * 3.1).sin(),
                0.04 * (t * 4.1).cos(),
            );
            vr.head(Vec3::new(0.0, 1.65, 0.0) + sway);
            vr.step(1);
        }
        let after = feet(&vr);
        let drift = Vec2::new(after.x - before.x, after.z - before.z).length();
        assert!(
            drift < 0.05,
            "playspace moved {drift:.3} m swaying on {deg:.0} deg"
        );
    }

    /// Lean over a table to look at what's on it: you aren't shoved back
    /// from it.
    #[test]
    fn leaning_over_a_table_doesnt_shove_you() {
        let mut vr = Vr::new("training_yard");
        let feet0 = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet0 + Vec3::Y * 0.05, 0.0);
        // A table top 0.76 m up, its near edge 0.35 m in front.
        vr.app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(1.2, 0.04, 0.6),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            Transform::from_translation(feet0 + Vec3::new(0.0, 0.74, -0.65)),
        ));
        vr.step(30);
        let before = feet(&vr);
        for i in 0..=60 {
            let k = i as f32 / 60.0;
            vr.app
                .world_mut()
                .get_mut::<RigInput>(vr.player)
                .unwrap()
                .head = Transform::from_xyz(0.0, 1.65 - 0.25 * k, -0.4 * k)
                .with_rotation(Quat::from_rotation_x(-0.8 * k));
            vr.step(1);
        }
        vr.step(60);
        let after = feet(&vr);
        let drift = Vec2::new(after.x - before.x, after.z - before.z).length();
        assert!(drift < 0.05, "shoved {drift:.3} m leaning over a table");
    }

    /// Everything there is to pick up in the yard starts at a standing
    /// reach: the kit in the truck's crate, on its shelf; the yard's loot
    /// on crates. Floors are for what you drop. (The first playtest spent
    /// it sitting on the ground picking things up.)
    #[test]
    fn nothing_to_pick_up_starts_on_the_floor() {
        let dir = std::env::temp_dir().join(format!("extraction-reach-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let opts = crate::LaunchOptions {
            robots: false,
            map: crate::camp::TRAINING_YARD.into(),
            data_dir: Some(dir.clone()),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..90 {
            app.update();
        }
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let bed = {
            let mut q = app
                .world_mut()
                .query_filtered::<&GlobalTransform, With<crate::raid::truck::Truck>>();
            q.iter(app.world()).next().unwrap().translation().y + crate::raid::truck::FLOOR
        };
        let catalog = app.world().resource::<crate::items::Catalog>().0.clone();
        let mut low = Vec::new();
        let mut in_crate = 0;
        let mut q = app.world_mut().query::<(
            &crate::items::Item,
            &GlobalTransform,
            Option<&crate::interact::body::InBackpack>,
            Option<&crate::interact::InSocket>,
        )>();
        for (item, gt, bag, socket) in q.iter(app.world()) {
            let p = gt.translation();
            // Artifacts grow out of the ground where it goes wrong: the one
            // deliberate exception, and the far grab is for them.
            let def = catalog.get(item.kind());
            if socket.is_some() || def.category == extraction_core::items::Category::Artifact {
                continue;
            }
            let (floor, what) = if bag.is_some() {
                in_crate += 1;
                (bed, "the bed")
            } else {
                (zone.ground(p.x, p.z), "the ground")
            };
            if p.y - floor < 0.5 {
                low.push(format!(
                    "{}: {:.2} m above {what} at {p:.1?}",
                    def.id,
                    p.y - floor
                ));
            }
        }
        // Every station the signs send you to has something on it.
        let markers: Vec<_> = zone
            .map
            .markers
            .iter()
            .filter(|m| matches!(m.kind, extraction_core::map::MarkerKind::Loot { .. }))
            .map(|m| (m.name.clone(), Vec2::new(m.at[0], m.at[1])))
            .collect();
        let mut q = app
            .world_mut()
            .query_filtered::<&GlobalTransform, With<crate::items::Item>>();
        let bare: Vec<_> = markers
            .iter()
            .filter(|(_, at)| {
                !q.iter(app.world())
                    .any(|gt| gt.translation().xz().distance(*at) < 1.0)
            })
            .map(|(name, _)| name.clone())
            .collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            !markers.is_empty() && bare.is_empty(),
            "nothing at {bare:?}"
        );
        assert!(in_crate >= 8, "the kit is in the crate ({in_crate} items)");
        assert!(low.is_empty(), "within a stoop of the floor: {low:#?}");
    }

    /// Out in the valleys, most of what's lying about is on a table, a
    /// shelf, a crate or a bench, not at your feet. Floors happen (and the
    /// far grab is for them), but as the exception.
    #[test]
    fn most_loot_in_the_zone_is_off_the_floor() {
        use bevy::ecs::system::RunSystemOnce;
        let mut report = Vec::new();
        for map in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let dir =
                std::env::temp_dir().join(format!("extraction-loot-{map}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let opts = crate::LaunchOptions {
                robots: false,
                map: map.into(),
                data_dir: Some(dir.clone()),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..90 {
                app.update();
            }
            let (total, low) = app
                .world_mut()
                .run_system_once(
                    |items: Query<
                        (&crate::items::Item, &GlobalTransform),
                        (
                            Without<crate::interact::body::InBackpack>,
                            Without<crate::interact::InSocket>,
                            Without<crate::interact::Held>,
                        ),
                    >,
                     catalog: Res<Catalog>,
                     spatial: SpatialQuery| {
                        let floor = SpatialQueryFilter::from_mask([crate::world::Layer::World]);
                        let mut low = Vec::new();
                        let mut total = 0;
                        for (item, gt) in &items {
                            let def = catalog.0.get(item.kind());
                            if def.category == extraction_core::items::Category::Artifact {
                                continue;
                            }
                            total += 1;
                            let p = gt.translation();
                            // Where you'd stand to reach it: the lowest
                            // surface within a stride around it (rays that
                            // start inside a wall, or drop off a storey,
                            // say nothing).
                            let below = (0..8)
                                .filter_map(|k| {
                                    let a = k as f32 * std::f32::consts::FRAC_PI_4;
                                    let from = p + Vec3::new(a.cos() * 0.6, 0.1, a.sin() * 0.6);
                                    let d = spatial
                                        .cast_ray(from, Dir3::NEG_Y, 2.1, true, &floor)?
                                        .distance;
                                    (d > 0.0).then_some(d - 0.1)
                                })
                                .fold(0.0f32, f32::max);
                            if below < 0.3 {
                                low.push(format!("{} {below:.2} m up at {p:.1?}", def.id));
                            }
                        }
                        (total, low)
                    },
                )
                .unwrap();
            let _ = std::fs::remove_dir_all(&dir);
            report.push((map, total, low));
        }
        for (map, total, low) in &report {
            eprintln!("{map}: {} of {total} on the floor", low.len());
            for l in low {
                eprintln!("  {l}");
            }
        }
        for (map, total, low) in &report {
            assert!(*total > 20, "{map} has loot ({total})");
            assert!(
                low.len() * 8 <= *total,
                "{map}: {} of {total} things lie on the floor",
                low.len()
            );
        }
    }
}
