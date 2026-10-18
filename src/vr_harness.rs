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
}
