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
    /// A profile of its own, removed afterwards.
    dir: Option<std::path::PathBuf>,
}

impl Drop for Vr {
    fn drop(&mut self) {
        if let Some(d) = &self.dir {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}

impl Vr {
    /// A headless game on `map` with the rig driven as by a headset.
    pub fn new(map: &str) -> Self {
        let opts = crate::LaunchOptions {
            robots: false,
            map: map.into(),
            ..Default::default()
        };
        Self::with(opts, None)
    }

    /// The training yard as someone new gets it: a profile of its own (so
    /// the kit is in the crate), starting in the truck bed.
    pub fn first_run() -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "extraction-first-run-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let opts = crate::LaunchOptions {
            robots: false,
            map: crate::camp::TRAINING_YARD.into(),
            data_dir: Some(dir.clone()),
            ..Default::default()
        };
        Self::with(opts, Some(dir))
    }

    pub(crate) fn with(opts: crate::LaunchOptions, dir: Option<std::path::PathBuf>) -> Self {
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
        let mut vr = Self { app, player, dir };
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

    /// A hand at `local` in the playspace (so it moves with the body, as a
    /// real hand does while you climb), level, with its grip.
    pub fn hand_local(&mut self, side: Side, local: Vec3, grip: f32) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        let h = input.hand_mut(side);
        h.pose = Transform::from_translation(local);
        h.tracked = true;
        h.grip = grip;
    }

    /// Where the playspace (and the feet under the head) stands.
    pub fn feet(&self) -> Vec3 {
        self.root().translation
    }

    pub fn grip(&mut self, side: Side, v: f32) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        input.hand_mut(side).grip = v;
    }

    /// Tracked fingers, as the XR driver reads them from hand-tracking
    /// joints (`None`: a controller without finger sensing).
    pub fn fingers(&mut self, side: Side, f: Option<crate::rig::fingers::Fingers>) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        input.hand_mut(side).fingers = f;
    }

    pub fn trigger(&mut self, side: Side, v: f32) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        input.hand_mut(side).trigger = v;
    }

    pub fn primary(&mut self, side: Side, down: bool) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        input.hand_mut(side).primary = down;
    }

    pub fn secondary(&mut self, side: Side, down: bool) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        input.hand_mut(side).secondary = down;
    }

    pub fn stick_click(&mut self, side: Side, down: bool) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        input.hand_mut(side).stick_click = down;
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

    /// With Index controllers a relaxed hand reads no squeeze but its
    /// fingers still wrap the gun: it stays in hand until the fingers open
    /// too. (Fingers from synthetic hand-tracking joints, as the XR driver
    /// gets them.)
    #[test]
    fn relaxed_tracked_fingers_keep_hold_of_a_gun() {
        use crate::rig::fingers::{fingers_from_joints, tests::synthetic_hand};
        let hand_of = |curl: f32| {
            fingers_from_joints(&synthetic_hand([curl; 5], [0.0; 4], Transform::IDENTITY))
        };
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let at = feet + Vec3::new(0.15, 1.1, -0.35);
        let gun = vr.spawn("pm_pistol", Transform::from_translation(at));
        let grip_at = vr.point(gun, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip_at, Quat::IDENTITY, 1.0, 0.0);
        vr.fingers(Side::Right, hand_of(0.8));
        vr.step(4);
        assert_eq!(
            vr.hand_state(Side::Right).holding,
            Some(gun),
            "pistol in hand"
        );
        // The squeeze relaxes; the fingers stay round the grip.
        vr.grip(Side::Right, 0.0);
        vr.step(6);
        assert_eq!(
            vr.hand_state(Side::Right).holding,
            Some(gun),
            "a relaxed hand dropped the gun"
        );
        // The fingers open: now it goes.
        vr.fingers(Side::Right, hand_of(0.1));
        vr.step(6);
        assert_eq!(
            vr.hand_state(Side::Right).holding,
            None,
            "open fingers kept it"
        );
    }

    /// Grabbing by closing the fingers is a comfort setting, off by
    /// default: off, tracked fingers closing on a magazine leave it; on,
    /// they take it (no squeeze either way).
    #[test]
    fn closing_tracked_fingers_grabs_only_when_asked() {
        use crate::rig::comfort::Comfort;
        use crate::rig::fingers::{fingers_from_joints, tests::synthetic_hand};
        let hand_of = |curl: f32| {
            fingers_from_joints(&synthetic_hand([curl; 5], [0.0; 4], Transform::IDENTITY))
        };
        for on in [false, true] {
            let mut vr = Vr::new("training_yard");
            vr.app.world_mut().insert_resource(Comfort {
                finger_grab: on,
                ..Comfort::default()
            });
            let feet = on_ground(&vr, OPEN.x, OPEN.z);
            vr.stand(feet, 0.0);
            let at = feet + Vec3::new(0.2, 1.0, -0.4);
            let item = vr.spawn("pm_mag", Transform::from_translation(at));
            vr.app
                .world_mut()
                .entity_mut(item)
                .insert(RigidBody::Static);
            vr.hand(Side::Right, at, Quat::IDENTITY, 0.0, 0.0);
            vr.fingers(Side::Right, hand_of(0.1));
            vr.step(3);
            vr.fingers(Side::Right, hand_of(0.85));
            vr.step(4);
            let held = vr.hand_state(Side::Right).holding;
            assert_eq!(held.is_some(), on, "finger grab {on}: holding {held:?}");
            if on {
                // Opening the fingers lets it go again.
                vr.fingers(Side::Right, hand_of(0.1));
                vr.step(4);
                assert_eq!(vr.hand_state(Side::Right).holding, None);
            }
        }
    }

    /// A wall of the world's in front of you, its near face `near` m ahead.
    fn wall(vr: &mut Vr, feet: Vec3, near: f32) {
        vr.app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(3.0, 3.0, 0.2),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            Transform::from_translation(feet + Vec3::new(0.0, 1.2, -near - 0.1)),
        ));
        vr.step(2);
    }

    /// A hand pushed into a wall stops at its face: the hand you see stays
    /// this side while the tracked one goes in.
    #[test]
    fn a_hand_pressed_into_a_wall_stops_at_its_face() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        wall(&mut vr, feet, 0.5);
        vr.hand(
            Side::Right,
            feet + Vec3::new(0.2, 1.2, -0.3),
            Quat::IDENTITY,
            0.0,
            0.0,
        );
        vr.step(5);
        vr.move_hand(Side::Right, feet + Vec3::new(0.2, 1.2, -0.75));
        vr.step(15);
        let hand = vr.hand_state(Side::Right);
        let felt = hand.felt.expect("a physics hand").translation;
        let face = feet.z - 0.5;
        // The body's front: its centre plus its box toward the fingers.
        let front = felt.z + crate::interact::physics_hands::HAND_CENTRE.z
            - crate::interact::physics_hands::HAND_HALF.z;
        assert!(
            front > face - 0.015,
            "the hand went {:.3} m into the wall",
            face - front
        );
        assert!(
            front < face + 0.05,
            "the hand stopped {:.3} m short",
            front - face
        );
        assert!(
            hand.world.translation.z < face - 0.2,
            "the tracked hand is in the wall"
        );
    }

    /// Shots seen leaving guns.
    #[derive(Resource, Default)]
    struct Shots(Vec<crate::weapons::ballistics::Shot>);

    fn collect_shots(
        mut shots: MessageReader<crate::weapons::ballistics::Shot>,
        mut seen: ResMut<Shots>,
    ) {
        seen.0.extend(shots.read().cloned());
    }

    /// A pistol pushed against a wall stays this side of it, and a round
    /// fired with its muzzle at the wall can't come out the far side.
    #[test]
    fn a_gun_pushed_into_a_wall_cant_fire_through_it() {
        use extraction_core::firearm::Magazine;
        let mut vr = Vr::new("training_yard");
        vr.app
            .init_resource::<Shots>()
            .add_systems(FixedUpdate, collect_shots);
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        wall(&mut vr, feet, 0.55);
        let gun = vr.spawn(
            "pm_pistol",
            Transform::from_translation(feet + Vec3::new(0.15, 1.2, -0.3)),
        );
        vr.app.world_mut().entity_mut(gun).insert(RigidBody::Static);
        let grip = vr.point(gun, |r| *r == GripRole::Primary);
        let rake = vr
            .app
            .world()
            .get::<crate::weapons::Gun>(gun)
            .unwrap()
            .layout
            .grip
            .rotation;
        vr.hand(Side::Right, grip, rake, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(gun)
            .insert(RigidBody::Dynamic);
        vr.step(10);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(gun));
        {
            let mut g = vr
                .app
                .world_mut()
                .get_mut::<crate::weapons::Gun>(gun)
                .unwrap();
            let f = &mut g.firearm;
            f.insert_magazine(Magazine {
                family: "pm".into(),
                capacity: 8,
                rounds: 8,
                ..Default::default()
            })
            .unwrap();
            f.set_bolt_travel(1.0);
            f.release_bolt();
            while f.fire_mode() != extraction_core::firearm::FireMode::Semi {
                f.cycle_mode();
            }
            f.condition = 1.0;
        }
        // Shove the hand 0.4 m on, well through the wall.
        vr.move_hand(Side::Right, grip + Vec3::new(0.0, 0.0, -0.45));
        vr.step(30);
        let face = feet.z - 0.55;
        let gt = *vr.app.world().get::<GlobalTransform>(gun).unwrap();
        assert!(gt.translation().z > face, "the gun went through the wall");
        vr.trigger(Side::Right, 1.0);
        vr.step(4);
        vr.trigger(Side::Right, 0.0);
        let shots = &vr.app.world().resource::<Shots>().0;
        assert!(!shots.is_empty(), "the pistol fired");
        for s in shots {
            assert!(
                s.origin.z > face - 0.005,
                "a round left from {} beyond the wall",
                s.origin
            );
        }
        // However the muzzle got past a wall (poked through a gap, a fast
        // swing), the round stops at it: pin the gun with a thin board
        // across it between grip and muzzle, and fire again.
        vr.app.world_mut().resource_mut::<Shots>().0.clear();
        vr.app.world_mut().entity_mut(gun).insert(RigidBody::Static);
        vr.step(2);
        let gt = *vr.app.world().get::<GlobalTransform>(gun).unwrap();
        let layout = vr
            .app
            .world()
            .get::<crate::weapons::Gun>(gun)
            .unwrap()
            .layout
            .clone();
        let (g, m) = (
            gt.transform_point(layout.grip.translation),
            gt.transform_point(layout.muzzle),
        );
        let board = (g + m) / 2.0;
        vr.app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(0.6, 0.6, 0.01),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            Transform::from_translation(board),
        ));
        vr.step(2);
        vr.trigger(Side::Right, 1.0);
        vr.step(4);
        vr.trigger(Side::Right, 0.0);
        let shots = &vr.app.world().resource::<Shots>().0;
        assert!(!shots.is_empty(), "the pistol fired again");
        for s in shots {
            assert!(
                s.origin.z > board.z,
                "a round left from {} past the board at {}",
                s.origin,
                board.z
            );
        }
    }

    /// A palm pushed against a door, from the side it swings away from,
    /// opens it: no grip needed.
    #[test]
    fn a_palm_pushes_a_door_open() {
        let mut vr = Vr::new("training_yard");
        let doors: Vec<(Entity, crate::world::buildings::DoorLeaf, GlobalTransform)> = vr
            .app
            .world_mut()
            .query::<(Entity, &crate::world::buildings::DoorLeaf, &GlobalTransform)>()
            .iter(vr.app.world())
            .map(|(e, d, g)| (e, *d, *g))
            .collect();
        let (door, leaf, gt) = doors.first().cloned().expect("a door in the yard");
        // Stand on the leaf's +Z side (it swings toward -Z, away from you).
        let mid = gt.transform_point(Vec3::new(leaf.0.width * 0.6, 1.1, 0.0));
        let out = gt.rotation() * Vec3::Z;
        let feet = on_ground(&vr, mid.x + out.x * 0.55, mid.z + out.z * 0.55);
        let yaw = (-out.x).atan2(-out.z).to_degrees();
        vr.stand(feet, yaw);
        let face = Quat::from_rotation_arc(Vec3::NEG_Z, -out);
        vr.hand(Side::Right, mid + out * 0.25, face, 0.0, 0.0);
        vr.step(5);
        // Push: the real hand goes on through the leaf.
        vr.move_hand(Side::Right, mid - out * 0.15);
        vr.step(60);
        let d = vr
            .app
            .world()
            .get::<crate::world::buildings::Door>(door)
            .unwrap();
        assert!(d.open && d.angle > 0.5, "the door didn't swing open: {d:?}");
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

    /// Where the feet stand, in the world (the head may lean past them).
    fn body_at(vr: &mut Vr) -> Vec3 {
        let root = *vr.app.world().get::<Transform>(vr.player).unwrap();
        let body = vr
            .app
            .world()
            .get::<crate::player::Body>(vr.player)
            .unwrap();
        let feet = body.feet.unwrap_or_default();
        root.transform_point(Vec3::new(feet.x, 0.0, feet.y))
    }

    /// A beam across your way at 1.35 m: walking at it standing, you stop;
    /// ducking, you pass under; standing up beneath it, the body stays
    /// under it. (The training yard wanted a beam to duck under and had to
    /// leave it without a collider: the body was 1.75 m tall however low
    /// your head went.)
    #[test]
    fn duck_under_a_beam() {
        let mut vr = Vr::new("marrow_creek");
        let zone = vr.app.world().resource::<crate::world::Zone>().clone();
        let start = zone.on_ground(0.0, 200.0);
        vr.stand(start + Vec3::Y * 0.05, 0.0);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        vr.step(30);
        let from = body_at(&mut vr);
        // Facing -Z; the beam 1 m ahead, its underside 1.35 m over the
        // ground there.
        let beam_z = from.z - 1.0;
        let ground = zone.on_ground(from.x, beam_z).y;
        vr.app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(4.0, 0.2, 0.3),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            Transform::from_xyz(from.x, ground + 1.35 + 0.1, beam_z),
        ));
        vr.step(2);
        let walk = |vr: &mut Vr, head_y: f32| {
            for i in 0..=200 {
                vr.head(Vec3::new(0.0, head_y, -2.0 * i as f32 / 200.0));
                vr.step(1);
            }
            vr.step(10);
        };
        walk(&mut vr, 1.65);
        let stood = body_at(&mut vr);
        assert!(
            stood.z > beam_z + 0.2,
            "standing, the beam stops you ({:.2} vs beam at {beam_z:.2})",
            stood.z
        );
        // Back to the start (the feet with it), crouch, and go again.
        vr.head(Vec3::new(0.0, 1.0, 0.0));
        vr.app
            .world_mut()
            .get_mut::<crate::player::Body>(vr.player)
            .unwrap()
            .feet = None;
        vr.stand(start + Vec3::Y * 0.05, 0.0);
        vr.step(30);
        walk(&mut vr, 1.0);
        let ducked = body_at(&mut vr);
        assert!(
            ducked.z < beam_z - 0.5,
            "ducking, you pass under ({:.2} vs beam at {beam_z:.2})",
            ducked.z
        );
        // Back under it (the feet follow the head to within a lean of
        // it), and stand up there.
        vr.head(Vec3::new(0.0, 1.0, -0.65));
        vr.step(60);
        let under = body_at(&mut vr);
        assert!(
            (under.z - beam_z).abs() < 0.2,
            "under the beam ({:.2})",
            under.z
        );
        vr.head(Vec3::new(0.0, 1.65, -0.65));
        vr.step(30);
        let h = vr
            .app
            .world()
            .get::<crate::player::Body>(vr.player)
            .unwrap()
            .height;
        assert!(h < 1.36, "the body stays under the beam ({h:.2} m tall)");
        let after = body_at(&mut vr);
        assert!(
            (after.y - under.y).abs() < 0.05,
            "and on the ground ({:.2} -> {:.2})",
            under.y,
            after.y
        );
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

    /// No button sits on another panel: each is beside its own card, clear
    /// of its neighbours' faces (in the truck, by the yard's tailgate).
    #[test]
    fn buttons_sit_clear_of_other_panels() {
        use crate::interact::poke::{BUTTON_R, PokeButton};
        use crate::raid::bench::ScreenText;
        let mut vr = Vr::new("training_yard");
        vr.step(5);
        let world = vr.app.world_mut();
        let buttons: Vec<(Entity, Vec3)> = world
            .query::<(&PokeButton, &GlobalTransform)>()
            .iter(world)
            .map(|(b, gt)| (b.target, gt.translation()))
            .collect();
        let panels: Vec<(Entity, GlobalTransform, Vec2)> = world
            .query::<(Entity, &GlobalTransform, &ScreenText)>()
            .iter(world)
            .map(|(e, gt, s)| (e, *gt, s.size))
            .collect();
        assert!(buttons.len() >= 5, "buttons: {}", buttons.len());
        let mut clashes = Vec::new();
        for (target, at) in &buttons {
            for (e, gt, size) in &panels {
                if e == target {
                    continue;
                }
                let p = gt.affine().inverse().transform_point3(*at);
                let reach = BUTTON_R + 0.012;
                if p.z.abs() < 0.06
                    && p.x.abs() < size.x * 0.5 + reach
                    && p.y.abs() < size.y * 0.5 + reach
                {
                    clashes.push(format!("{target} button on {e} at {p:.3?}"));
                }
            }
        }
        assert!(clashes.is_empty(), "{clashes:#?}");
    }

    /// A finger on the VIGNETTE card's button: held just short of the cap
    /// nothing happens; pushed in, it clicks once and the setting moves on.
    #[test]
    fn a_fingertip_pushes_a_button() {
        use crate::interact::poke::{CAP, FINGERTIP, PokeButton};
        use crate::raid::comfort_board::ComfortCard;
        let mut vr = Vr::new("training_yard");
        vr.step(5);
        let card = vr
            .app
            .world_mut()
            .query::<(Entity, &ComfortCard)>()
            .iter(vr.app.world())
            .find(|(_, c)| **c == ComfortCard::Vignette)
            .map(|(e, _)| e)
            .expect("the vignette card");
        let button = *vr
            .app
            .world_mut()
            .query::<(&PokeButton, &GlobalTransform)>()
            .iter(vr.app.world())
            .find(|(b, _)| b.target == card)
            .map(|(_, gt)| gt)
            .expect("a button on it");
        let before = vr
            .app
            .world()
            .get_resource::<crate::rig::comfort::Comfort>()
            .map(|c| c.vignette);
        // Finger pointing into the wall at the button.
        let normal = button.rotation() * Vec3::Z;
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, -normal);
        let tip_at = |d: f32| button.translation() + normal * d - rot * FINGERTIP;
        vr.hand(Side::Right, tip_at(0.08), rot, 0.0, 0.0);
        vr.step(2);
        for k in 0..10 {
            vr.move_hand(Side::Right, tip_at(0.08 - k as f32 * 0.006 + 0.0));
            vr.step(1);
        }
        // Resting on the cap, not pushing: no click.
        vr.move_hand(Side::Right, tip_at(CAP + 0.009));
        vr.step(3);
        assert!(!vr.pressed().contains(&card), "a touch isn't a press");
        // Pushed home.
        for k in 0..6 {
            vr.move_hand(Side::Right, tip_at(CAP - k as f32 * 0.003));
            vr.step(1);
        }
        vr.step(2);
        assert_eq!(
            vr.pressed().iter().filter(|e| **e == card).count(),
            1,
            "one click"
        );
        let after = vr
            .app
            .world()
            .get_resource::<crate::rig::comfort::Comfort>()
            .map(|c| c.vignette);
        if before.is_some() {
            assert_ne!(before, after, "the setting moved on");
        }
    }

    /// One-handed, a magazine in the hand touched to a box of rounds set
    /// down fills; with two hands, the box has to be in the other hand.
    #[test]
    fn one_handed_fill_a_magazine_from_a_box_set_down() {
        for one_handed in [false, true] {
            let mut vr = Vr::new("training_yard");
            vr.app
                .world_mut()
                .resource_mut::<crate::rig::comfort::Comfort>()
                .one_handed = one_handed;
            let feet = vr.root().translation;
            vr.step(10);
            let box_at = feet + Vec3::new(0.1, 0.9, -0.45);
            let ammo = vr.spawn("ammo_9x18", Transform::from_translation(box_at));
            vr.app
                .world_mut()
                .entity_mut(ammo)
                .insert(RigidBody::Static);
            let mag_at = feet + Vec3::new(0.3, 1.0, -0.45);
            let mag = vr.spawn("pm_mag", Transform::from_translation(mag_at));
            vr.app.world_mut().entity_mut(mag).insert(RigidBody::Static);
            vr.hand(Side::Right, mag_at, Quat::IDENTITY, 0.0, 0.0);
            vr.step(2);
            vr.grip(Side::Right, 1.0);
            vr.step(4);
            assert_eq!(vr.hand_state(Side::Right).holding, Some(mag));
            let before = vr
                .app
                .world()
                .get::<crate::items::Item>(mag)
                .unwrap()
                .rounds;
            // Down onto the box where it lies.
            vr.move_hand(Side::Right, box_at + Vec3::Y * 0.03);
            vr.step(10);
            let after = vr
                .app
                .world()
                .get::<crate::items::Item>(mag)
                .unwrap()
                .rounds;
            assert_eq!(
                after > before,
                one_handed,
                "{before} -> {after}, one-handed {one_handed}"
            );
        }
    }

    /// One-handed: the pistol's empty well brought down onto a magazine in a
    /// pouch takes it, and the second button racks a round in; no second
    /// hand anywhere. With two hands, the same move does nothing.
    #[test]
    fn one_handed_reload_from_a_pouch() {
        for one_handed in [false, true] {
            let mut vr = Vr::new("training_yard");
            vr.app
                .world_mut()
                .resource_mut::<crate::rig::comfort::Comfort>()
                .one_handed = one_handed;
            let feet = vr.root().translation;
            vr.step(10);
            // A loaded magazine into a pouch, with the one hand.
            let mag = vr.spawn(
                "pm_mag",
                Transform::from_translation(feet + Vec3::new(0.2, 1.0, -0.4)),
            );
            vr.app
                .world_mut()
                .get_mut::<crate::items::Item>(mag)
                .unwrap()
                .rounds = 8;
            vr.app.world_mut().entity_mut(mag).insert(RigidBody::Static);
            vr.hand(
                Side::Right,
                feet + Vec3::new(0.2, 1.0, -0.4),
                Quat::IDENTITY,
                0.0,
                0.0,
            );
            vr.step(2);
            vr.grip(Side::Right, 1.0);
            vr.step(4);
            assert_eq!(vr.hand_state(Side::Right).holding, Some(mag));
            let pouch = {
                let mut q = vr.app.world_mut().query::<(
                    &crate::interact::Socket,
                    &crate::interact::SocketOwner,
                    &GlobalTransform,
                )>();
                q.iter(vr.app.world())
                    .find(|(s, o, _)| {
                        o.0 == vr.player
                            && s.occupant.is_none()
                            && s.kind
                                == crate::interact::SocketKind::Body(
                                    extraction_core::inventory::SlotKind::Pouch,
                                )
                    })
                    .map(|(_, _, g)| g.translation())
                    .expect("a pouch")
            };
            vr.move_hand(Side::Right, pouch);
            vr.step(2);
            vr.grip(Side::Right, 0.0);
            vr.step(4);
            assert!(
                vr.app
                    .world()
                    .get::<crate::interact::InSocket>(mag)
                    .is_some(),
                "pouched"
            );
            // The pistol, empty, in the same hand.
            let pistol = vr.spawn(
                "pm_pistol",
                Transform::from_translation(feet + Vec3::new(0.3, 1.0, -0.45)),
            );
            vr.app
                .world_mut()
                .entity_mut(pistol)
                .insert(RigidBody::Static);
            vr.hand(
                Side::Right,
                feet + Vec3::new(0.3, 1.0, -0.45),
                Quat::IDENTITY,
                0.0,
                0.0,
            );
            vr.step(2);
            vr.grip(Side::Right, 1.0);
            vr.step(4);
            assert_eq!(vr.hand_state(Side::Right).holding, Some(pistol));
            vr.primary(Side::Right, true);
            vr.step(3);
            vr.primary(Side::Right, false);
            vr.step(20);
            let well = |vr: &mut Vr| {
                let mut q = vr.app.world_mut().query::<(
                    &crate::interact::Socket,
                    &crate::interact::SocketOwner,
                    &GlobalTransform,
                )>();
                q.iter(vr.app.world())
                    .find(|(s, o, _)| {
                        o.0 == pistol && matches!(s.kind, crate::interact::SocketKind::MagWell(_))
                    })
                    .map(|(s, _, g)| (s.occupant, g.translation()))
                    .expect("a well")
            };
            assert_eq!(well(&mut vr).0, None, "empty to start");
            // The well brought down onto the pouched magazine's top.
            let mag_top = {
                let g = *vr.app.world().get::<GlobalTransform>(mag).unwrap();
                let h = vr
                    .app
                    .world()
                    .get::<crate::weapons::Mag>(mag)
                    .unwrap()
                    .height;
                g.transform_point(Vec3::Y * h * 0.5)
            };
            let hand_now = vr.hand_state(Side::Right).world.translation;
            let off = well(&mut vr).1 - hand_now;
            for k in 0..=10 {
                let above = Vec3::Y * 0.15 * (1.0 - k as f32 / 10.0);
                vr.move_hand(Side::Right, mag_top - off + above);
                vr.step(1);
            }
            vr.step(3);
            let seated = well(&mut vr).0 == Some(mag);
            assert_eq!(seated, one_handed, "seated from the pouch: {seated}");
            if !one_handed {
                continue;
            }
            // The second button racks a round in.
            let before = vr.app.world().get::<Gun>(pistol).unwrap().firearm.chambers[0].is_some();
            assert!(!before, "nothing chambered yet");
            vr.secondary(Side::Right, true);
            vr.step(3);
            vr.secondary(Side::Right, false);
            vr.step(60);
            let gun = vr.app.world().get::<Gun>(pistol).unwrap();
            assert!(gun.firearm.chambers[0].is_some(), "racked: a round in");
        }
    }

    /// Seated: a head at a seated height is lifted to a standing eye (the
    /// body's slots ride at standing height with it); something on the
    /// floor is out of reach of a hand held as low as a seated person
    /// reaches, until the crouch button brings the view and the hands down
    /// to it. Standing play, the same head isn't lifted.
    #[test]
    fn seated_lift_and_crouch_to_the_floor() {
        for seated in [false, true] {
            let mut vr = Vr::new("training_yard");
            vr.app
                .world_mut()
                .resource_mut::<crate::rig::comfort::Comfort>()
                .seated = seated;
            // On the floor, under where the hand hangs.
            let at = vr.root().transform_point(Vec3::new(0.25, 0.05, -0.35));
            let radio = vr.spawn("radio", Transform::from_translation(at));
            vr.app
                .world_mut()
                .entity_mut(radio)
                .insert(RigidBody::Static);
            // Raw poses each frame, as a headset driver writes them: a seated
            // head, the hand down beside the chair at knee height (no lean).
            let raw = |vr: &mut Vr, grip: f32, crouch: bool| {
                let mut input = vr.app.world_mut().get_mut::<RigInput>(vr.player).unwrap();
                input.head = Transform::from_xyz(0.0, 1.2, 0.0);
                let h = input.hand_mut(Side::Right);
                h.pose = Transform::from_xyz(0.25, 0.55, -0.35);
                h.grip = grip;
                h.tracked = true;
                input.crouch = crouch;
                input.lifted = 0.0;
                drop(input);
                vr.step(1);
            };
            // (The seated height is the highest of the last few seconds.)
            for _ in 0..(90 * (crate::rig::seated::WINDOW + 1)) {
                raw(&mut vr, 0.0, false);
            }
            let head_y = vr
                .app
                .world()
                .get::<RigInput>(vr.player)
                .unwrap()
                .head
                .translation
                .y;
            let want = if seated {
                crate::rig::seated::STAND_EYE
            } else {
                1.2
            };
            let lifted = vr.app.world().get::<RigInput>(vr.player).unwrap().lifted;
            let seat = vr
                .app
                .world()
                .get::<crate::rig::seated::Seat>(vr.player)
                .copied();
            assert!(
                (head_y - want).abs() < 0.02,
                "seated {seated}: head at {head_y}, lifted {lifted}, seat {seat:?}"
            );
            // The floor's out of reach, sat up.
            for _ in 0..6 {
                raw(&mut vr, 1.0, false);
            }
            assert_ne!(
                vr.hand_state(Side::Right).holding,
                Some(radio),
                "out of reach"
            );
            if !seated {
                continue;
            }
            // The crouch button: down to it.
            for _ in 0..60 {
                raw(&mut vr, 0.0, true);
            }
            for _ in 0..6 {
                raw(&mut vr, 1.0, true);
            }
            assert_eq!(
                vr.hand_state(Side::Right).holding,
                Some(radio),
                "reached the floor"
            );
        }
    }

    /// The MUSIC card, in the row over the stash, pushed in by a fingertip:
    /// the score goes off.
    #[test]
    fn a_fingertip_turns_the_score_off() {
        use crate::interact::poke::{CAP, FINGERTIP, PokeButton};
        use crate::raid::comfort_board::ComfortCard;
        let mut vr = Vr::new("training_yard");
        vr.step(5);
        *vr.app
            .world_mut()
            .resource_mut::<crate::rig::comfort::Comfort>() = Default::default();
        vr.step(1);
        let card = vr
            .app
            .world_mut()
            .query::<(Entity, &ComfortCard)>()
            .iter(vr.app.world())
            .find(|(_, c)| **c == ComfortCard::Score)
            .map(|(e, _)| e)
            .expect("the music card");
        let button = *vr
            .app
            .world_mut()
            .query::<(&PokeButton, &GlobalTransform)>()
            .iter(vr.app.world())
            .find(|(b, _)| b.target == card)
            .map(|(_, gt)| gt)
            .expect("a button on it");
        let normal = button.rotation() * Vec3::Z;
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, -normal);
        let tip_at = |d: f32| button.translation() + normal * d - rot * FINGERTIP;
        vr.hand(Side::Right, tip_at(0.08), rot, 0.0, 0.0);
        vr.step(2);
        for k in 0..10 {
            vr.move_hand(Side::Right, tip_at(0.08 - k as f32 * 0.006));
            vr.step(1);
        }
        for k in 0..6 {
            vr.move_hand(Side::Right, tip_at(CAP - k as f32 * 0.003));
            vr.step(1);
        }
        vr.step(2);
        assert!(vr.pressed().contains(&card), "pressed");
        let c = vr.app.world().resource::<crate::rig::comfort::Comfort>();
        assert!(!c.score, "the score is off");
    }

    /// Something on the ground three metres off: point at it, squeeze,
    /// flick the hand back, and it's in your hand. Pointing away from it,
    /// the same flick does nothing.
    #[test]
    fn point_and_flick_fetches_it() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let item = vr.spawn(
            "field_dressing",
            Transform::from_translation(on_ground(&vr, OPEN.x, OPEN.z - 3.0) + Vec3::Y * 0.1),
        );
        vr.step(60);
        let lying = vr
            .app
            .world()
            .get::<GlobalTransform>(item)
            .unwrap()
            .translation();
        let hand = feet + Vec3::new(0.2, 1.2, -0.3);
        let flick = |vr: &mut Vr, from: Vec3| {
            vr.grip(Side::Right, 1.0);
            vr.step(2);
            for k in 1..=4 {
                vr.move_hand(Side::Right, from + Vec3::new(0.0, 0.01, 0.045) * k as f32);
                vr.step(1);
            }
            vr.step(80);
        };

        // Pointing off to the side: nothing chosen, nothing comes.
        let away = Quat::from_rotation_arc(Vec3::NEG_Z, (lying - hand + Vec3::X * 2.0).normalize());
        vr.hand(Side::Right, hand, away, 0.0, 0.0);
        vr.step(3);
        assert_eq!(vr.hand_state(Side::Right).far, None, "nothing pointed at");
        flick(&mut vr, hand);
        let still = vr
            .app
            .world()
            .get::<GlobalTransform>(item)
            .unwrap()
            .translation();
        assert!(
            still.distance(lying) < 0.05,
            "it stayed put: {lying} -> {still}"
        );
        assert_eq!(vr.hand_state(Side::Right).holding, None);
        vr.grip(Side::Right, 0.0);
        vr.step(3);

        // Pointing at it.
        let at = Quat::from_rotation_arc(Vec3::NEG_Z, (lying - hand).normalize());
        vr.hand(Side::Right, hand, at, 0.0, 0.0);
        vr.step(3);
        assert_eq!(vr.hand_state(Side::Right).far, Some(item), "pointed at");
        flick(&mut vr, hand);
        assert_eq!(
            vr.hand_state(Side::Right).holding,
            Some(item),
            "caught: it's at {}",
            vr.app
                .world()
                .get::<GlobalTransform>(item)
                .unwrap()
                .translation()
        );
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

    /// Walk up the practice house's front steps in your room, through the
    /// open door: you end up on its floor. (The first headset session
    /// couldn't: the capsule appeared inside the first step and was pushed
    /// back out, pushing the room away.)
    #[test]
    fn walking_up_the_house_steps_in_the_room() {
        let mut vr = Vr::new("training_yard");
        let house = {
            let mut q = vr
                .app
                .world_mut()
                .query::<(&crate::world::buildings::Building, &GlobalTransform)>();
            q.iter(vr.app.world())
                .find(|(b, _)| b.prefab == "house_plank")
                .map(|(_, g)| g.compute_transform())
                .expect("the practice house")
        };
        let start = house.transform_point(Vec3::new(0.0, 0.0, -6.0));
        let yaw = house.rotation.to_euler(EulerRot::YXZ).0 + std::f32::consts::PI;
        vr.stand(on_ground(&vr, start.x, start.z) + Vec3::Y * 0.05, yaw);
        let doors: Vec<Entity> = {
            let mut q =
                vr.app
                    .world_mut()
                    .query::<(Entity, &crate::world::buildings::Door, &GlobalTransform)>();
            q.iter(vr.app.world())
                .filter(|(_, _, g)| g.translation().distance(house.translation) < 8.0)
                .map(|(e, _, _)| e)
                .collect()
        };
        assert!(!doors.is_empty(), "the house has a door");
        for d in &doors {
            vr.app.world_mut().write_message(InteractEvent::Pressed {
                item: *d,
                player: vr.player,
            });
        }
        vr.step(120);
        // Three and a bit metres across the room, up three steps.
        for i in 0..=420 {
            vr.head(Vec3::new(0.0, 1.65, -3.3 * i as f32 / 420.0));
            vr.step(1);
        }
        vr.step(30);
        let floor = house.translation.y + 0.5;
        let foot = feet(&vr).y;
        assert!(
            (foot - floor).abs() < 0.08,
            "on the house floor (feet {foot:.2}, floor {floor:.2})"
        );
    }

    /// Stand at the yard kitchen's bench, on its 13 degree slope, and lean
    /// in over it for ten seconds, swaying a little: you stay where you
    /// stand. (The first headset session slid down this slope here: the
    /// movement capsule leant in under the head, caught the bench's edge
    /// and let go of the ground.)
    #[test]
    fn leaning_over_the_kitchen_bench_on_a_slope_stays_put() {
        let mut vr = Vr::new("training_yard");
        let zone = vr.app.world().resource::<crate::world::Zone>().clone();
        // The bench is beside the truck stop, its long side along z; stand
        // east of it, facing west.
        let stop = zone
            .marker(&extraction_core::map::MarkerKind::TruckStop)
            .unwrap()
            .at;
        let at = Vec2::new(stop[0] - 4.5 + 0.75, stop[1] - 1.0);
        assert!(
            zone.hm.slope_deg([at.x, at.y]) > 10.0,
            "a slope to stand on"
        );
        vr.stand(
            zone.on_ground(at.x, at.y) + Vec3::Y * 0.02,
            std::f32::consts::FRAC_PI_2,
        );
        vr.step(60);
        let before = feet(&vr);
        let mut rng = extraction_core::noise::Rng::new(7);
        for i in 0..=900 {
            let t = i as f32 / 90.0;
            let k = t.min(1.0);
            let jitter = Vec3::new(
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
            ) * 0.004;
            // Leaning in and down, looking at the bench, swaying a little.
            let at = Vec3::new(
                (t * 2.1).sin() * 0.15,
                1.65 - 0.2 * k + (t * 3.0).sin() * 0.02,
                -0.4 * k + (t * 1.3).sin() * 0.075,
            );
            vr.app
                .world_mut()
                .get_mut::<RigInput>(vr.player)
                .unwrap()
                .head = Transform::from_translation(at + jitter)
                .with_rotation(Quat::from_rotation_x(-0.5 * k));
            vr.step(1);
        }
        let after = feet(&vr);
        let drift = Vec2::new(after.x - before.x, after.z - before.z).length();
        assert!(drift < 0.05, "slid {drift:.2} m leaning over the bench");
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
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
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

/// The training yard's stations, each done by hand as in a headset: the
/// yard has to notice every one through the real hand path.
fn tests_feet(vr: &Vr) -> Vec3 {
    vr.app
        .world()
        .get::<Transform>(vr.player)
        .unwrap()
        .translation
}

mod onboarding {
    use super::*;
    use crate::onboarding::{Goal, Lamp, OnboardCard, Onboarding, Placed, StationPost};
    use crate::raid::truck::{FLOOR, Pressable, Stash, Truck};
    use crate::weapons::Gun;

    fn ob(vr: &Vr) -> Onboarding {
        vr.app.world().resource::<Onboarding>().clone()
    }

    fn done(vr: &Vr, g: Goal) -> bool {
        ob(vr).is_done(g)
    }

    fn placed(vr: &mut Vr, which: Placed) -> Entity {
        vr.app
            .world_mut()
            .query::<(Entity, &Placed)>()
            .iter(vr.app.world())
            .find(|(_, p)| **p == which)
            .map(|(e, _)| e)
            .expect("placed for the station")
    }

    fn at(vr: &Vr, e: Entity) -> Vec3 {
        vr.app
            .world()
            .get::<GlobalTransform>(e)
            .unwrap()
            .translation()
    }

    fn ground(vr: &Vr, xz: [f32; 2]) -> Vec3 {
        vr.app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(xz[0], xz[1])
    }

    /// Open hand on it, squeeze.
    fn take(vr: &mut Vr, side: Side, item: Entity) {
        let p = at(vr, item);
        vr.hand(side, p, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(side, 1.0);
        vr.step(4);
        assert_eq!(vr.hand_state(side).holding, Some(item), "took it");
    }

    /// Point the empty hand at it from `hand`, squeeze, flick back.
    fn flick(vr: &mut Vr, side: Side, hand: Vec3, item: Entity) {
        let target = at(vr, item);
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, (target - hand).normalize());
        vr.hand(side, hand, rot, 0.0, 0.0);
        vr.step(3);
        assert_eq!(vr.hand_state(side).far, Some(item), "pointed at it");
        vr.grip(side, 1.0);
        vr.step(2);
        let back = -(target - hand).normalize() * 0.045 + Vec3::Y * 0.01;
        for k in 1..=4 {
            vr.move_hand(side, hand + back * k as f32);
            vr.step(1);
        }
        vr.step(90);
    }

    fn truck_pose(vr: &mut Vr) -> Transform {
        let mut q = vr
            .app
            .world_mut()
            .query_filtered::<&GlobalTransform, With<Truck>>();
        q.iter(vr.app.world()).next().unwrap().compute_transform()
    }

    fn in_the_bed(vr: &mut Vr) -> Vec3 {
        let truck = truck_pose(vr);
        let feet = truck.transform_point(Vec3::new(0.3, FLOOR + 0.05, 0.6));
        let yaw = truck.rotation.to_euler(EulerRot::YXZ).0;
        vr.stand(feet, yaw);
        vr.step(10);
        feet
    }

    fn stash_item(vr: &mut Vr, id: &str) -> Entity {
        let catalog = vr.app.world().resource::<Catalog>().clone();
        let contents: Vec<Entity> = vr
            .app
            .world_mut()
            .query_filtered::<&crate::interact::body::Backpack, With<Stash>>()
            .iter(vr.app.world())
            .flat_map(|b| b.contents.clone())
            .collect();
        contents
            .into_iter()
            .find(|e| {
                vr.app
                    .world()
                    .get::<crate::items::Item>(*e)
                    .is_some_and(|it| catalog.get(it.kind()).id == id)
            })
            .unwrap_or_else(|| panic!("{id} in the crate"))
    }

    fn card(vr: &mut Vr, which: OnboardCard) -> Entity {
        vr.app
            .world_mut()
            .query::<(Entity, &OnboardCard)>()
            .iter(vr.app.world())
            .find(|(_, c)| **c == which)
            .map(|(e, _)| e)
            .expect("the card")
    }

    fn lamps(vr: &mut Vr) -> Vec<Lamp> {
        let mut v: Vec<StationPost> = vr
            .app
            .world_mut()
            .query::<&StationPost>()
            .iter(vr.app.world())
            .copied()
            .collect();
        v.sort_by_key(|p| p.index);
        v.into_iter().map(|p| p.lamp).collect()
    }

    fn said(vr: &Vr, text: &str) -> bool {
        vr.app
            .world()
            .resource::<crate::raid::Raid>()
            .log
            .iter()
            .any(|(t, _)| t.contains(text))
    }

    /// Station 1: in the bed, reach into the crate and squeeze; the pistol
    /// and the radio each tick, and the first lamp goes from lit to done
    /// while the second lights.
    #[test]
    fn station_kit_from_the_crate() {
        let mut vr = Vr::first_run();
        in_the_bed(&mut vr);
        let l = lamps(&mut vr);
        assert_eq!(l[0], Lamp::Lit, "the crate's lamp is lit first: {l:?}");
        assert!(l[1..].iter().all(|l| *l == Lamp::Dark), "{l:?}");
        let pistol = stash_item(&mut vr, "pm_pistol");
        take(&mut vr, Side::Right, pistol);
        assert!(done(&vr, Goal::TakePistol));
        assert!(!done(&vr, Goal::TakeRadio));
        let radio = stash_item(&mut vr, "radio");
        take(&mut vr, Side::Left, radio);
        vr.step(2);
        assert!(done(&vr, Goal::TakeRadio));
        let l = lamps(&mut vr);
        assert_eq!((l[0], l[1]), (Lamp::Done, Lamp::Lit), "{l:?}");
        assert!(
            said(&vr, "Next: the lamp off the back of the truck"),
            "the driver sends you on"
        );
    }

    /// Station 2: the right stick sideways turns you, pulled down crouches
    /// you; or squatting for real counts too.
    #[test]
    fn station_getting_about() {
        let mut vr = Vr::first_run();
        {
            let mut o = vr.app.world_mut().resource_mut::<Onboarding>();
            o.done = vec![Goal::TakePistol, Goal::TakeRadio];
        }
        let feet = ground(&vr, [-3.5, 91.0]);
        vr.stand(feet, 0.0);
        vr.step(10);
        assert!(!done(&vr, Goal::Turn));
        let stick = |vr: &mut Vr, v: Vec2| {
            vr.app
                .world_mut()
                .get_mut::<RigInput>(vr.player)
                .unwrap()
                .hand_mut(Side::Right)
                .stick = v;
            vr.step(3);
        };
        // (The OpenXR driver turns the rig on it; headless, the stick is
        // what there is.)
        stick(&mut vr, Vec2::new(1.0, 0.0));
        stick(&mut vr, Vec2::ZERO);
        assert!(done(&vr, Goal::Turn), "the turn counted");
        stick(&mut vr, Vec2::new(0.0, -1.0));
        stick(&mut vr, Vec2::ZERO);
        assert!(done(&vr, Goal::Crouch), "the crouch counted");
        // Squatting for real does it too.
        let mut o = vr.app.world_mut().resource_mut::<Onboarding>();
        o.done.retain(|g| *g != Goal::Crouch);
        vr.head(Vec3::new(0.0, 1.0, 0.0));
        vr.step(3);
        assert!(done(&vr, Goal::Crouch), "squatting counted");
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        // Run: the left stick clicked and pushed forward (the driver turns
        // the click into sprint).
        {
            let mut input = vr.app.world_mut().get_mut::<RigInput>(vr.player).unwrap();
            input.locomotion = Vec2::Y;
            input.sprint = true;
        }
        vr.step(5);
        assert!(done(&vr, Goal::Run), "the run counted");
    }

    /// Station 3: let go of a thing at a pocket, and of another over the
    /// shoulder.
    #[test]
    fn station_put_it_away() {
        let mut vr = Vr::first_run();
        let feet = ground(&vr, [3.0, 70.0]);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.7, 0.0));
        vr.step(30);
        let radio = vr.spawn(
            "radio",
            Transform::from_translation(feet + Vec3::new(0.3, 1.0, -0.4)),
        );
        vr.app
            .world_mut()
            .entity_mut(radio)
            .insert(RigidBody::Static);
        take(&mut vr, Side::Right, radio);
        // The nearest free body slot that takes it.
        let slots: Vec<Vec3> = {
            let mut q = vr.app.world_mut().query::<(
                &crate::interact::Socket,
                &crate::interact::SocketOwner,
                &GlobalTransform,
            )>();
            q.iter(vr.app.world())
                .filter(|(s, o, _)| {
                    o.0 == vr.player
                        && s.occupant.is_none()
                        && matches!(
                            s.kind,
                            crate::interact::SocketKind::Body(
                                extraction_core::inventory::SlotKind::Pocket
                                    | extraction_core::inventory::SlotKind::Pouch
                            )
                        )
                })
                .map(|(_, _, g)| g.translation())
                .collect()
        };
        assert!(!slots.is_empty(), "a pocket");
        let from = at(&vr, radio);
        for i in 0..=12 {
            vr.move_hand(Side::Right, from.lerp(slots[0], i as f32 / 12.0));
            vr.step(1);
        }
        vr.grip(Side::Right, 0.0);
        vr.step(4);
        assert!(done(&vr, Goal::PutAway), "pocketed");
        assert!(!done(&vr, Goal::Pack));

        let kit = vr.spawn(
            "cleaning_kit",
            Transform::from_translation(feet + Vec3::new(0.25, 1.0, -0.4)),
        );
        vr.app.world_mut().entity_mut(kit).insert(RigidBody::Static);
        take(&mut vr, Side::Right, kit);
        let shoulder = feet + Vec3::new(0.15, 1.65, 0.12);
        let from = at(&vr, kit);
        for i in 0..=10 {
            vr.move_hand(Side::Right, from.lerp(shoulder, i as f32 / 10.0));
            vr.step(1);
        }
        vr.grip(Side::Right, 0.0);
        vr.step(4);
        assert!(done(&vr, Goal::Pack), "into the pack");
    }

    /// Station 3: the box in the pen is out of reach (the fence keeps you
    /// off it); point, squeeze, flick, and it's in your hand.
    #[test]
    fn station_out_of_reach() {
        let mut vr = Vr::first_run();
        let pen = crate::onboarding::FAR_STAND_AT;
        let feet = ground(&vr, [pen[0], pen[1] + crate::onboarding::PEN_HALF + 0.35]);
        vr.stand(feet, 0.0);
        vr.step(60);
        let item = placed(&mut vr, Placed::FarBox);
        let box_at = at(&vr, item);
        let reach = box_at.xz().distance(feet.xz());
        assert!(reach > 1.5, "out of reach ({reach:.2} m off)");
        // Walking at it gets you no closer than the fence.
        vr.app
            .world_mut()
            .get_mut::<RigInput>(vr.player)
            .unwrap()
            .locomotion = Vec2::Y;
        vr.step(120);
        vr.app
            .world_mut()
            .get_mut::<RigInput>(vr.player)
            .unwrap()
            .locomotion = Vec2::ZERO;
        vr.step(10);
        let feet = super::tests_feet(&vr);
        let reach = box_at.xz().distance(feet.xz());
        assert!(reach > 1.5, "the fence holds you off ({reach:.2} m)");
        flick(
            &mut vr,
            Side::Right,
            feet + Vec3::new(0.2, 1.3, -0.25),
            item,
        );
        assert_eq!(vr.hand_state(Side::Right).holding, Some(item), "caught");
        assert!(done(&vr, Goal::FarGrab));
    }

    /// Station 4, the range, all by hand: rounds into the magazine (box
    /// and magazine touched together), the magazine up into the well, the
    /// slide racked with the other hand's trigger, the safety off with a
    /// stick click, and a shot that hits the 15 m plate.
    #[test]
    fn station_the_range() {
        let mut vr = Vr::first_run();
        let table = crate::onboarding::RANGE_TABLE_AT;
        let feet = ground(&vr, [table[0], table[1] + 0.75]);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.7, 0.0));
        vr.step(60);
        let near = |vr: &mut Vr, id: &str| -> Entity {
            let catalog = vr.app.world().resource::<Catalog>().clone();
            let mut q = vr
                .app
                .world_mut()
                .query::<(Entity, &crate::items::Item, &GlobalTransform)>();
            q.iter(vr.app.world())
                .filter(|(_, it, gt)| {
                    catalog.get(it.kind()).id == id
                        && gt.translation().xz().distance(Vec2::from(table)) < 1.0
                })
                .map(|(e, ..)| e)
                .next()
                .unwrap_or_else(|| panic!("a {id} on the table"))
        };
        let mag = near(&mut vr, "pm_mag");
        let ammo = near(&mut vr, "ammo_9x18");
        let gun = near(&mut vr, "pm_pistol");
        take(&mut vr, Side::Left, mag);
        take(&mut vr, Side::Right, ammo);
        // Bring them together in front of the chest.
        let (a, b) = (at(&vr, mag), at(&vr, ammo));
        let meet = feet + Vec3::new(0.0, 1.2, -0.35);
        for i in 0..=15 {
            let k = i as f32 / 15.0;
            vr.move_hand(Side::Left, a.lerp(meet - Vec3::X * 0.03, k));
            vr.move_hand(Side::Right, b.lerp(meet + Vec3::X * 0.03, k));
            vr.step(1);
        }
        vr.step(4);
        let rounds = vr
            .app
            .world()
            .get::<crate::items::Item>(mag)
            .unwrap()
            .rounds;
        assert!(rounds > 0, "rounds went in ({rounds})");
        assert!(done(&vr, Goal::FillMag));
        // Put the box down; the pistol in the right hand.
        vr.grip(Side::Right, 0.0);
        vr.step(20);
        let grip_at = vr.point(gun, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip_at, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(4);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(gun));
        let hold = feet + Vec3::new(0.15, 1.2, -0.35);
        vr.move_hand(Side::Right, hold);
        vr.step(20);
        let well = {
            let w = vr.app.world().get::<Gun>(gun).unwrap().magwell.unwrap();
            at(&vr, w)
        };
        let from = at(&vr, mag);
        let start = well - Vec3::Y * 0.25;
        for i in 0..=10 {
            vr.move_hand(Side::Left, from.lerp(start, i as f32 / 10.0));
            vr.step(1);
        }
        for i in 0..=30 {
            vr.move_hand(Side::Left, start.lerp(well, i as f32 / 30.0));
            vr.step(1);
        }
        vr.step(5);
        vr.grip(Side::Left, 0.0);
        vr.step(5);
        assert!(done(&vr, Goal::SeatMag), "seated");
        // Rack: the left trigger on the slide, back, and let go.
        let slide = vr.point(gun, |r| matches!(r, GripRole::Handle(_)));
        vr.hand(Side::Left, slide, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.trigger(Side::Left, 1.0);
        vr.step(2);
        let back = vr.app.world().get::<GlobalTransform>(gun).unwrap().back();
        for i in 1..=14 {
            vr.move_hand(Side::Left, slide + back * (0.005 * i as f32));
            vr.step(1);
        }
        vr.trigger(Side::Left, 0.0);
        vr.move_hand(Side::Left, feet + Vec3::new(-0.3, 1.0, -0.2));
        vr.step(10);
        assert!(done(&vr, Goal::Rack), "racked");
        // Safety off: click the stick on the gun hand.
        vr.stick_click(Side::Right, true);
        vr.step(2);
        vr.stick_click(Side::Right, false);
        vr.step(2);
        assert!(done(&vr, Goal::Safety), "off safe");
        // Aim at the nearest plate and squeeze.
        let plate = {
            let mut q = vr
                .app
                .world_mut()
                .query::<(&crate::world::buildings::Building, &GlobalTransform)>();
            q.iter(vr.app.world())
                .filter(|(b, _)| b.prefab == "target")
                .map(|(_, gt)| gt.transform_point(Vec3::new(0.0, 1.2, 0.07)))
                .min_by(|a, b| a.distance(feet).total_cmp(&b.distance(feet)))
                .unwrap()
        };
        for _ in 0..4 {
            let hand_rot = vr.hand_state(Side::Right).world.rotation;
            let gt = vr
                .app
                .world()
                .get::<GlobalTransform>(gun)
                .unwrap()
                .compute_transform();
            let rel = hand_rot.inverse() * gt.rotation;
            let want = Transform::from_translation(gt.translation)
                .looking_at(plate, Vec3::Y)
                .rotation;
            vr.hand(Side::Right, hold, want * rel.inverse(), 1.0, 0.0);
            vr.step(15);
        }
        vr.trigger(Side::Right, 1.0);
        vr.step(6);
        vr.trigger(Side::Right, 0.0);
        vr.step(30);
        assert!(done(&vr, Goal::HitPlate), "hit the plate at 15 m");
        // A on the gun hand: the magazine drops out.
        vr.primary(Side::Right, true);
        vr.step(3);
        vr.primary(Side::Right, false);
        vr.step(10);
        assert!(done(&vr, Goal::DropMag), "the mag dropped");
    }

    /// Station 5: a hand swings the house's door open; the find on the
    /// crate inside comes to a squeeze.
    #[test]
    fn station_the_house() {
        let mut vr = Vr::first_run();
        let doorway = vr.app.world().resource::<crate::onboarding::Doorway>().0;
        let find = placed(&mut vr, Placed::HouseFind);
        let inward = (at(&vr, find) - doorway).with_y(0.0).normalize();
        let doors: Vec<(Entity, crate::world::buildings::DoorLeaf, GlobalTransform)> = vr
            .app
            .world_mut()
            .query::<(Entity, &crate::world::buildings::DoorLeaf, &GlobalTransform)>()
            .iter(vr.app.world())
            .map(|(e, d, g)| (e, *d, *g))
            .collect();
        let (door, leaf, gt) = doors
            .into_iter()
            .min_by(|a, b| {
                a.2.translation()
                    .distance(doorway)
                    .total_cmp(&b.2.translation().distance(doorway))
            })
            .unwrap();
        let handle = gt.transform_point(Vec3::new(leaf.0.width - 0.12, 1.0, 0.0));
        let outside = handle - inward * 0.6;
        vr.stand(ground(&vr, [outside.x, outside.z]), 0.0);
        vr.hand(Side::Right, handle, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        vr.grip(Side::Right, 0.0);
        vr.step(60);
        assert!(
            vr.app
                .world()
                .get::<crate::world::buildings::Door>(door)
                .unwrap()
                .open
        );
        assert!(done(&vr, Goal::OpenDoor), "the door counted");
        // In over the threshold, to the crate.
        let inside = doorway + inward * 0.4;
        vr.stand(Vec3::new(inside.x, doorway.y, inside.z), 0.0);
        vr.step(20);
        take(&mut vr, Side::Right, find);
        assert!(done(&vr, Goal::Search), "the find counted");
    }

    /// First aid: raise the left hand and look at the back of it (the
    /// wrist's screen); then a dressing in hand, the face button.
    #[test]
    fn station_first_aid() {
        let mut vr = Vr::first_run();
        let feet = ground(&vr, [2.0, 8.0]);
        vr.stand(feet, 0.0);
        vr.step(20);
        // The hand low at the side: not looking at it.
        vr.hand(
            Side::Left,
            feet + Vec3::new(-0.25, 0.9, 0.0),
            Quat::IDENTITY,
            0.0,
            0.0,
        );
        vr.step(60);
        assert!(!done(&vr, Goal::Wrist), "a hand at your side isn't a look");
        // Raised in front of the face, the head bent to it.
        let hand_at = feet + Vec3::new(-0.05, 1.35, -0.3);
        let hand_rot = Quat::from_rotation_z(-0.95);
        vr.hand(Side::Left, hand_at, hand_rot, 0.0, 0.0);
        vr.step(2);
        let panel = Transform::from_translation(hand_at)
            .with_rotation(hand_rot)
            .transform_point(crate::onboarding::WRIST_PANEL);
        let eye = feet + Vec3::new(0.0, 1.65, 0.0);
        let look =
            Transform::from_translation(Vec3::new(0.0, 1.65, 0.0)).looking_to(panel - eye, Vec3::Y);
        vr.app
            .world_mut()
            .get_mut::<RigInput>(vr.player)
            .unwrap()
            .head = look;
        vr.step(60);
        assert!(done(&vr, Goal::Wrist), "the look counted");
        let d = vr.spawn(
            "field_dressing",
            Transform::from_translation(feet + Vec3::new(0.25, 1.0, -0.4)),
        );
        vr.app.world_mut().entity_mut(d).insert(RigidBody::Static);
        take(&mut vr, Side::Right, d);
        // Wound round the left forearm (`interact::first_aid`).
        let wrist = feet + Vec3::new(-0.05, 1.15, -0.4);
        vr.hand(Side::Left, wrist, Quat::IDENTITY, 0.0, 0.0);
        let centre = wrist + Vec3::Z * 0.12;
        for i in 0..=48 {
            let a = std::f32::consts::TAU * i as f32 / 24.0;
            vr.move_hand(
                Side::Right,
                centre + Vec3::new(a.cos(), a.sin(), 0.0) * 0.06,
            );
            vr.step(1);
        }
        vr.step(3);
        assert!(done(&vr, Goal::Dressing), "the dressing counted");
    }

    /// Left-handed: the holster is on the left hip (a pistol let go there
    /// from the left hand goes in it), and the watch is on the right wrist
    /// (looking at the left one doesn't count).
    #[test]
    fn left_handed_holster_and_watch() {
        let mut vr = Vr::first_run();
        vr.app
            .world_mut()
            .resource_mut::<crate::rig::comfort::Comfort>()
            .left_handed = true;
        let feet = ground(&vr, [2.0, 8.0]);
        vr.stand(feet, 0.0);
        vr.step(20);
        let holster = {
            let mut q = vr.app.world_mut().query::<(
                Entity,
                &crate::interact::Socket,
                &crate::interact::SocketOwner,
                &GlobalTransform,
            )>();
            q.iter(vr.app.world())
                .find(|(_, s, o, _)| {
                    o.0 == vr.player
                        && s.kind
                            == crate::interact::SocketKind::Body(
                                extraction_core::inventory::SlotKind::Holster,
                            )
                })
                .map(|(e, _, _, g)| (e, g.translation()))
                .expect("a holster")
        };
        assert!(
            holster.1.x < feet.x - 0.1,
            "on the left hip: {:?}",
            holster.1
        );
        let pistol = vr.spawn(
            "pm_pistol",
            Transform::from_translation(feet + Vec3::new(-0.25, 1.0, -0.4)),
        );
        vr.app
            .world_mut()
            .entity_mut(pistol)
            .insert(RigidBody::Static);
        take(&mut vr, Side::Left, pistol);
        let from = at(&vr, pistol);
        for i in 0..=12 {
            vr.move_hand(Side::Left, from.lerp(holster.1, i as f32 / 12.0));
            vr.step(1);
        }
        vr.grip(Side::Left, 0.0);
        vr.step(4);
        let seated = vr
            .app
            .world()
            .get::<crate::interact::InSocket>(pistol)
            .map(|s| s.socket);
        assert_eq!(seated, Some(holster.0), "holstered on the left");

        // The left wrist, looked at: nothing.
        let look_at = |vr: &mut Vr, side: Side, x: f32| {
            let hand_at = feet + Vec3::new(x, 1.35, -0.3);
            let hand_rot = Quat::from_rotation_z(0.95 * side.sign());
            vr.hand(side, hand_at, hand_rot, 0.0, 0.0);
            vr.step(2);
            let panel = Transform::from_translation(hand_at)
                .with_rotation(hand_rot)
                .transform_point(
                    crate::onboarding::WRIST_PANEL * Vec3::new(-side.sign(), 1.0, 1.0),
                );
            let eye = feet + Vec3::new(0.0, 1.65, 0.0);
            vr.app
                .world_mut()
                .get_mut::<RigInput>(vr.player)
                .unwrap()
                .head = Transform::from_translation(Vec3::new(0.0, 1.65, 0.0))
                .looking_to(panel - eye, Vec3::Y);
            vr.step(60);
        };
        look_at(&mut vr, Side::Left, -0.05);
        assert!(!done(&vr, Goal::Wrist), "the gun hand's wrist has no watch");
        look_at(&mut vr, Side::Right, 0.05);
        assert!(done(&vr, Goal::Wrist), "the right wrist's watch");
    }

    /// The shimmer: a part thrown by hand (grip, swing, open) sets off the
    /// snare on the way in, from outside it.
    #[test]
    fn station_shimmer_throw_a_part_first() {
        let mut vr = Vr::first_run();
        vr.step(30);
        let snare = crate::onboarding::SNARE_AT;
        let r = crate::onboarding::SNARE_R;
        // West of it, facing it (+X).
        let feet = ground(&vr, [snare[0] - r - 1.6, snare[1]]);
        vr.stand(feet, -std::f32::consts::FRAC_PI_2);
        vr.step(20);
        let part = vr.spawn(
            "spare_parts",
            Transform::from_translation(feet + Vec3::new(0.3, 1.0, 0.3)),
        );
        vr.app
            .world_mut()
            .entity_mut(part)
            .insert(RigidBody::Static);
        take(&mut vr, Side::Right, part);
        vr.app
            .world_mut()
            .entity_mut(part)
            .insert(RigidBody::Dynamic);
        // Back by the shoulder, then a quick swing forward and up; open.
        let start = feet + Vec3::new(-0.1, 1.5, 0.3);
        vr.move_hand(Side::Right, start);
        vr.step(10);
        for k in 1..=6 {
            vr.move_hand(
                Side::Right,
                start + Vec3::new(0.11, 0.025, -0.01) * k as f32,
            );
            vr.step(1);
        }
        vr.grip(Side::Right, 0.0);
        vr.step(120);
        assert!(done(&vr, Goal::Probe), "the thrown part found the snare");
    }

    /// Station 7: walk into the shimmer and the driver warns you; from its
    /// edge, point and flick the stone out of it.
    #[test]
    fn station_the_shimmer() {
        let mut vr = Vr::first_run();
        let heart = crate::onboarding::SHIMMER_AT;
        let r = crate::onboarding::SHIMMER_R;
        vr.step(30);
        let stone = placed(&mut vr, Placed::Stone);
        // Too close.
        vr.stand(ground(&vr, [heart[0] - r * 0.6, heart[1]]), 0.0);
        vr.step(10);
        assert!(said(&vr, "Back out. That shimmer burns"), "warned");
        // Back to the edge, facing in (+X).
        let feet = ground(&vr, [heart[0] - r - 0.8, heart[1]]);
        vr.stand(feet, -std::f32::consts::FRAC_PI_2);
        vr.step(60);
        flick(
            &mut vr,
            Side::Right,
            feet + Vec3::new(0.25, 1.2, 0.2),
            stone,
        );
        assert_eq!(vr.hand_state(Side::Right).holding, Some(stone), "caught");
        assert!(done(&vr, Goal::Artifact));
    }

    /// Station 8: a fingertip on the PUSH ME card's button.
    #[test]
    fn station_buttons() {
        use crate::interact::poke::{CAP, FINGERTIP, PokeButton};
        let mut vr = Vr::first_run();
        let st = &crate::onboarding::STATIONS[crate::onboarding::station_of(Goal::Button)];
        vr.stand(
            ground(&vr, [st.at[0] + 0.8, st.at[1]]),
            std::f32::consts::FRAC_PI_2,
        );
        vr.step(10);
        let target = card(&mut vr, OnboardCard::Push);
        let button = *vr
            .app
            .world_mut()
            .query::<(&PokeButton, &GlobalTransform)>()
            .iter(vr.app.world())
            .find(|(b, _)| b.target == target)
            .map(|(_, gt)| gt)
            .expect("a button on it");
        let normal = button.rotation() * Vec3::Z;
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, -normal);
        let tip_at = |d: f32| button.translation() + normal * d - rot * FINGERTIP;
        vr.hand(Side::Right, tip_at(0.08), rot, 0.0, 0.0);
        vr.step(2);
        for k in 0..=20 {
            vr.move_hand(
                Side::Right,
                tip_at(0.08 - k as f32 * (0.08 - CAP + 0.012) / 20.0),
            );
            vr.step(1);
        }
        vr.step(3);
        assert!(done(&vr, Goal::Button), "the push counted");
    }

    /// Station 9: radio in hand at the north post, the face button; the
    /// truck comes round in seconds; the cord in its bed ends the yard.
    #[test]
    fn station_going_home() {
        let _restarts = crate::RESTART_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _ = crate::take_restart();
        let mut vr = Vr::first_run();
        // Everything else done, as if walked through.
        {
            let mut o = vr.app.world_mut().resource_mut::<Onboarding>();
            o.done = crate::onboarding::all_goals()
                .filter(|g| !matches!(g, Goal::CallTruck | Goal::PullCord))
                .collect();
        }
        let st = &crate::onboarding::STATIONS[crate::onboarding::station_of(Goal::CallTruck)];
        let feet = ground(&vr, [st.at[0], st.at[1] + 1.5]);
        vr.stand(feet, 0.0);
        vr.step(20);
        let radio = vr.spawn(
            "radio",
            Transform::from_translation(feet + Vec3::new(0.25, 1.0, -0.4)),
        );
        vr.app
            .world_mut()
            .entity_mut(radio)
            .insert(RigidBody::Static);
        take(&mut vr, Side::Right, radio);
        vr.primary(Side::Right, true);
        vr.step(3);
        vr.primary(Side::Right, false);
        vr.step(3);
        assert!(done(&vr, Goal::CallTruck), "the call counted");
        vr.step(((crate::onboarding::HURRY + 1.0) * 90.0) as usize);
        assert!(
            matches!(
                vr.app.world().resource::<crate::raid::Raid>().phase,
                crate::raid::Phase::TruckWaiting { .. }
            ),
            "the truck's here"
        );
        let truck = truck_pose(&mut vr);
        assert!(
            truck.translation.xz().distance(feet.xz()) < 25.0,
            "at the north post"
        );
        in_the_bed(&mut vr);
        let pull = vr
            .app
            .world_mut()
            .query::<(Entity, &Pressable, &GlobalTransform)>()
            .iter(vr.app.world())
            .find(|(_, p, _)| **p == Pressable::BellPull)
            .map(|(e, _, g)| (e, *g))
            .unwrap();
        let toggle = pull.1.transform_point(Vec3::new(0.0, -0.26, 0.0));
        vr.hand(Side::Left, toggle, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Left, 1.0);
        vr.step(3);
        assert!(done(&vr, Goal::PullCord));
        assert!(ob(&vr).finished());
        let to = crate::take_restart().expect("on to the Zone");
        assert_eq!(to.map, "marrow_creek");
    }

    /// The cord within reach of where you start asks before it ends the
    /// yard: one pull is a question, a second goes.
    #[test]
    fn an_early_cord_asks_first() {
        let _restarts = crate::RESTART_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _ = crate::take_restart();
        let mut vr = Vr::first_run();
        in_the_bed(&mut vr);
        let pull = vr
            .app
            .world_mut()
            .query::<(Entity, &Pressable, &GlobalTransform)>()
            .iter(vr.app.world())
            .find(|(_, p, _)| **p == Pressable::BellPull)
            .map(|(_, _, g)| *g)
            .unwrap();
        let toggle = pull.transform_point(Vec3::new(0.0, -0.26, 0.0));
        let tug = |vr: &mut Vr| {
            vr.hand(Side::Left, toggle, Quat::IDENTITY, 0.0, 0.0);
            vr.step(2);
            vr.grip(Side::Left, 1.0);
            vr.step(3);
            vr.grip(Side::Left, 0.0);
            vr.step(3);
        };
        tug(&mut vr);
        assert!(crate::take_restart().is_none(), "one pull only asks");
        assert!(said(&vr, "Pull again"));
        tug(&mut vr);
        assert!(crate::take_restart().is_some(), "the second goes");
    }

    /// A station's SKIP card (squeezed) passes it and lights the next; the
    /// board's START OVER puts you back at the crate. Boards say the keys in
    /// flat mode and the controls in a headset.
    #[test]
    fn skip_start_over_and_the_prompts() {
        let mut vr = Vr::first_run();
        vr.step(5);
        let far = crate::onboarding::station_of(Goal::FarGrab);
        let skip = card(&mut vr, OnboardCard::Skip(far));
        take_card(&mut vr, skip);
        assert!(done(&vr, Goal::FarGrab) && ob(&vr).skipped.contains(&Goal::FarGrab));
        assert_eq!(lamps(&mut vr)[far], Lamp::Done);
        let restart = card(&mut vr, OnboardCard::StartOver);
        take_card(&mut vr, restart);
        assert!(ob(&vr).done.is_empty());
        assert_eq!(lamps(&mut vr)[0], Lamp::Lit);

        let text = |vr: &mut Vr| {
            vr.app
                .world_mut()
                .query::<(
                    &crate::onboarding::StationSign,
                    &crate::raid::bench::ScreenText,
                )>()
                .iter(vr.app.world())
                .find(|(s, _)| s.0 == 0)
                .map(|(_, t)| t.text.clone())
                .unwrap()
        };
        vr.step(2);
        assert!(text(&mut vr).contains("Squeeze GRIP"), "{}", text(&mut vr));
        vr.app.insert_resource(RigDriver::Flat);
        vr.step(2);
        assert!(text(&mut vr).contains("Press E"), "{}", text(&mut vr));
    }

    /// Out in a valley too: a squeeze on a pistol that's on safe gets told
    /// how to take it off, instead of nothing happening.
    #[test]
    fn a_trigger_on_safe_says_why() {
        let mut vr = Vr::new("marrow_creek");
        let feet = ground(&vr, [0.0, 300.0]);
        vr.stand(feet, 0.0);
        vr.step(10);
        let gun = vr.spawn(
            "pm_pistol",
            Transform::from_translation(feet + Vec3::new(0.15, 1.1, -0.35)),
        );
        let grip_at = vr.point(gun, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip_at, Quat::IDENTITY, 1.0, 0.0);
        vr.step(10);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(gun));
        vr.trigger(Side::Right, 1.0);
        vr.step(3);
        assert!(said(&vr, "Safety's on. Click the stick"), "told why");
    }

    /// On the track to the gate, a beam at head height between two stacks
    /// of crates: walking at it upright, you're stopped; ducked, you go
    /// under, and it's noticed.
    #[test]
    fn ducking_under_the_beam() {
        let mut vr = Vr::first_run();
        let beam = crate::onboarding::BEAM_AT;
        // South of it, facing north along the track.
        vr.stand(ground(&vr, [beam[0], beam[1] + 2.0]), 0.0);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        vr.step(20);
        let walk = |vr: &mut Vr, frames: usize| {
            vr.app
                .world_mut()
                .get_mut::<RigInput>(vr.player)
                .unwrap()
                .locomotion = Vec2::Y;
            vr.step(frames);
            vr.app
                .world_mut()
                .get_mut::<RigInput>(vr.player)
                .unwrap()
                .locomotion = Vec2::ZERO;
            vr.step(5);
        };
        walk(&mut vr, 240);
        let z = super::tests_feet(&vr).z;
        assert!(
            z > beam[1] + 0.2,
            "upright, the beam stops you (feet at z {z:.2})"
        );
        assert!(!ob(&vr).ducked, "standing tall isn't ducking");
        vr.head(Vec3::new(0.0, 1.1, 0.0));
        vr.step(20);
        walk(&mut vr, 240);
        let z = super::tests_feet(&vr).z;
        assert!(z < beam[1] - 0.5, "ducked, you go under (feet at z {z:.2})");
        assert!(ob(&vr).ducked, "the duck was noticed");
        assert!(said(&vr, "Mind your head"));
    }

    /// From where you start in the bed, walk at the cord: you get within an
    /// arm of its toggle (the bench, the crate and the stand are all in the
    /// bed with you).
    #[test]
    fn the_cord_is_in_reach_from_the_bed() {
        let mut vr = Vr::first_run();
        in_the_bed(&mut vr);
        let toggle = {
            let mut q = vr.app.world_mut().query::<(&Pressable, &GlobalTransform)>();
            q.iter(vr.app.world())
                .find(|(p, _)| **p == Pressable::BellPull)
                .map(|(_, g)| g.transform_point(Vec3::new(0.0, -0.26, 0.0)))
                .unwrap()
        };
        // Face it and walk at it for four seconds.
        let feet = super::tests_feet(&vr);
        let to = (toggle - feet).with_y(0.0).normalize();
        vr.stand(feet, (-to.x).atan2(-to.z));
        vr.app
            .world_mut()
            .get_mut::<RigInput>(vr.player)
            .unwrap()
            .locomotion = Vec2::Y;
        let mut closest = f32::MAX;
        for _ in 0..(90 * 4) {
            vr.step(1);
            let f = super::tests_feet(&vr);
            closest = closest.min(f.xz().distance(toggle.xz()));
        }
        eprintln!("nearest to the cord: {closest:.2} m");
        // An arm, from the body's middle, reaches about 0.65 m.
        assert!(
            closest < 0.65,
            "the nearest the body gets to the cord is {closest:.2} m (from {feet:.2?} to {toggle:.2?})"
        );
    }

    /// Walk up to a card and squeeze it.
    fn take_card(vr: &mut Vr, card: Entity) {
        let p = at(vr, card);
        let n = vr
            .app
            .world()
            .get::<GlobalTransform>(card)
            .unwrap()
            .rotation()
            * Vec3::Z;
        vr.stand(ground(vr, [p.x + n.x * 0.5, p.z + n.z * 0.5]), 0.0);
        vr.hand(Side::Right, p, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        vr.grip(Side::Right, 0.0);
        vr.step(3);
    }
}

/// Climbing with your hands: a ladder up the face of a 3 m block, with
/// its top to step onto, and a 1.1 m wall to mantle.
mod climbing {
    use super::*;
    use crate::interact::hand_climb::Ladder;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    fn on_ground(vr: &Vr, x: f32, z: f32) -> Vec3 {
        vr.app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(x, z)
    }

    /// A block of the world's, `h` tall and `deep` deep, its near face
    /// `near` m in front of `feet`, 3 m wide.
    fn block(vr: &mut Vr, feet: Vec3, near: f32, h: f32, deep: f32) {
        vr.app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(3.0, h, deep),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            Transform::from_translation(feet + Vec3::new(0.0, h * 0.5, -near - deep * 0.5)),
        ));
        vr.step(2);
    }

    /// The 3 m block with a ladder up its face, 0.35 m in front of you.
    fn ladder(vr: &mut Vr, feet: Vec3) -> Entity {
        block(vr, feet, 0.4, 3.0, 2.0);
        let face = feet + Vec3::new(0.0, 0.0, -0.4);
        let l = vr
            .app
            .world_mut()
            .spawn(
                Ladder {
                    bottom: face + Vec3::Z * 0.05,
                    top: face + Vec3::new(0.0, 3.0, 0.05),
                    width: 0.45,
                    out: Vec3::Z,
                    over: Some(feet + Vec3::new(0.0, 3.0, -0.9)),
                }
                .bundle(),
            )
            .id();
        vr.step(2);
        l
    }

    /// One pull: take hold with `side` at shoulder height, let the other
    /// go, and draw the hand down to the waist.
    fn pull(vr: &mut Vr, side: Side) {
        let x = if side == Side::Right { 0.15 } else { -0.15 };
        vr.hand_local(side, Vec3::new(x, 1.55, -0.3), 1.0);
        vr.step(2);
        vr.hand_local(side.other(), Vec3::new(-x, 1.2, -0.15), 0.0);
        vr.step(1);
        for i in 1..=12 {
            vr.hand_local(side, Vec3::new(x, 1.55 - 0.055 * i as f32, -0.3), 1.0);
            vr.step(1);
        }
    }

    /// Hand over hand up a 3 m ladder, and onto the top.
    #[test]
    fn climb_a_ladder_hand_over_hand() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        ladder(&mut vr, feet);
        let mut side = Side::Right;
        for _ in 0..12 {
            pull(&mut vr, side);
            side = side.other();
            if vr.feet().y > feet.y + 2.9 {
                break;
            }
        }
        for s in Side::BOTH {
            vr.hand_local(s, Vec3::new(0.0, 1.0, -0.2), 0.0);
        }
        vr.step(30);
        let now = vr.feet();
        assert!(
            (now.y - (feet.y + 3.0)).abs() < 0.15 && now.z < feet.z - 0.5,
            "on top of the block: feet {now} from {feet}"
        );
    }

    /// Halfway up, let go with both hands: down you go.
    #[test]
    fn let_go_of_the_ladder_and_fall() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        ladder(&mut vr, feet);
        pull(&mut vr, Side::Right);
        pull(&mut vr, Side::Left);
        // (The body catches up with the last pull.)
        vr.step(10);
        let up = vr.feet().y - feet.y;
        assert!(up > 0.8, "climbing: up {up}");
        // Holding still, it holds you.
        vr.step(30);
        let held = vr.feet().y - feet.y;
        assert!((held - up).abs() < 0.05, "held at {up}, now {held}");
        for s in Side::BOTH {
            vr.hand_local(s, Vec3::new(0.0, 1.0, -0.2), 0.0);
        }
        vr.step(90);
        let down = vr.feet().y - feet.y;
        assert!(down < 0.1, "fell: {down} up");
    }

    /// Both palms put on a top at `top` m (a block `deep` m deep, or a
    /// wall with a window whose sill that is), then pushed down `push` m.
    /// Where are your feet, from where you stood?
    fn palms_down(top: f32, deep: f32, window: bool, push: f32) -> Vec3 {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        block(&mut vr, feet, 0.35, top, deep);
        if window {
            // The wall over the window, from 2.1 m up.
            vr.app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(3.0, 0.8, deep),
                CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
                Transform::from_translation(feet + Vec3::new(0.0, 2.5, -0.35 - deep * 0.5)),
            ));
            vr.step(2);
        }
        let at = |y: f32| [Vec3::new(-0.2, y, -0.45), Vec3::new(0.2, y, -0.45)];
        for (s, p) in [Side::Left, Side::Right].into_iter().zip(at(top + 0.04)) {
            vr.hand_local(s, p, 0.0);
        }
        vr.step(3);
        let steps = (push / 0.035).ceil().max(1.0) as usize;
        for i in 1..=steps {
            let y = top + 0.04 - push * i as f32 / steps as f32;
            for (s, p) in [Side::Left, Side::Right].into_iter().zip(at(y)) {
                vr.hand_local(s, p, 0.0);
            }
            vr.step(1);
        }
        vr.step(5);
        for s in Side::BOTH {
            vr.hand_local(s, Vec3::new(0.0, 1.0, -0.2), 0.0);
        }
        vr.step(30);
        vr.feet() - feet
    }

    /// A ledge at head height: both hands on it, one pull down to the
    /// waist, and you're up.
    #[test]
    fn mantle_a_head_high_ledge_in_one_pull() {
        let d = palms_down(1.6, 1.0, false, 0.7);
        assert!(
            (d.y - 1.6).abs() < 0.15 && d.z < -0.4,
            "up on the ledge: moved {d}"
        );
    }

    /// Through a window: palms on the sill of a thin wall, push, and you're
    /// down on the floor beyond it.
    #[test]
    fn climb_in_through_a_window() {
        let d = palms_down(0.9, 0.2, true, 0.4);
        assert!(
            d.y.abs() < 0.15 && d.z < -0.55,
            "through the window, on the far side: moved {d}"
        );
    }

    /// Leaning both hands on a table doesn't put you on it.
    #[test]
    fn leaning_on_a_table_keeps_you_on_the_floor() {
        let d = palms_down(0.85, 1.0, false, 0.05);
        assert!(d.y.abs() < 0.05 && d.z.abs() < 0.1, "moved {d}");
    }

    /// Both palms on a 1.1 m wall, push down, and you're up on it.
    #[test]
    fn mantle_a_wall_with_both_palms() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        block(&mut vr, feet, 0.35, 1.1, 1.0);
        let at = |y: f32| [Vec3::new(-0.2, y, -0.5), Vec3::new(0.2, y, -0.5)];
        for (s, p) in [Side::Left, Side::Right].into_iter().zip(at(1.14)) {
            vr.hand_local(s, p, 0.0);
        }
        vr.step(3);
        for i in 1..=20 {
            for (s, p) in [Side::Left, Side::Right]
                .into_iter()
                .zip(at(1.14 - 0.035 * i as f32))
            {
                vr.hand_local(s, p, 0.0);
            }
            vr.step(1);
        }
        for s in Side::BOTH {
            vr.hand_local(s, Vec3::new(0.0, 1.0, -0.2), 0.0);
        }
        vr.step(30);
        let now = vr.feet();
        assert!(
            (now.y - (feet.y + 1.1)).abs() < 0.15 && now.z < feet.z - 0.4,
            "up on the wall: feet {now} from {feet}"
        );
    }

    /// One palm on the wall doesn't lift you; and the comfort setting
    /// puts you straight at the top of a ladder you grip.
    #[test]
    fn one_palm_holds_nothing_and_teleport_climbs_at_once() {
        let mut vr = Vr::new("training_yard");
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        block(&mut vr, feet, 0.35, 1.1, 1.0);
        vr.hand_local(Side::Right, Vec3::new(0.2, 1.14, -0.5), 0.0);
        vr.step(3);
        for i in 1..=20 {
            vr.hand_local(
                Side::Right,
                Vec3::new(0.2, 1.14 - 0.035 * i as f32, -0.5),
                0.0,
            );
            vr.step(1);
        }
        assert!(vr.feet().y < feet.y + 0.05, "one palm lifted you");

        let mut vr = Vr::new("training_yard");
        vr.app
            .world_mut()
            .insert_resource(crate::rig::comfort::Comfort {
                climb_teleport: true,
                ..Default::default()
            });
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        ladder(&mut vr, feet);
        vr.hand_local(Side::Right, Vec3::new(0.15, 1.55, -0.3), 1.0);
        vr.step(4);
        let now = vr.feet();
        assert!(
            (now.y - (feet.y + 3.0)).abs() < 0.15,
            "teleported to the top: feet {now} from {feet}"
        );
    }
}

/// Tools worked with the hands.
mod tools {
    use super::*;
    use crate::destruction::hooks::{PRY_HOLD, Pried, Pryable};
    use crate::interact::tools::crowbar::{CLAW, GRIP, SHAFT};

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    fn on_ground(vr: &Vr, x: f32, z: f32) -> Vec3 {
        vr.app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(x, z)
    }

    /// The longest the torque on the block has stayed at 100 N·m or more
    /// (seconds), as Destruction counts it toward giving.
    #[derive(Resource, Default)]
    struct Strain {
        run: f32,
        longest: f32,
    }

    fn strain(mut r: MessageReader<Pried>, mut s: ResMut<Strain>, time: Res<Time>) {
        let hard = r.read().any(|p| p.torque >= 100.0);
        s.run = if hard { s.run + time.delta_secs() } else { 0.0 };
        s.longest = s.longest.max(s.run);
    }

    /// A crowbar taken up in both hands, its claw put to `seam` (world)
    /// with the bar leaning back toward `back` (level, toward you), then
    /// levered round the claw with the hands at `out` and `steady` metres up
    /// the bar from it, 0.45 rad, and held there.
    fn work_bar(vr: &mut Vr, feet: Vec3, seam: Vec3, back: Vec3, out: f32, steady: f32) {
        let bar = vr.spawn(
            "crowbar",
            Transform::from_translation(
                feet + Vec3::Y * 0.9 + (seam - feet).with_y(0.0) * 0.5 + Vec3::X * 0.3,
            ),
        );
        vr.app.world_mut().entity_mut(bar).insert(RigidBody::Static);
        let grip = vr.point(bar, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip, Quat::IDENTITY, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(bar)
            .insert(RigidBody::Dynamic);
        let shaft = vr.point(bar, |r| *r == GripRole::Support);
        vr.hand(Side::Left, shaft, Quat::IDENTITY, 1.0, 0.0);
        vr.step(4);
        assert!(
            vr.hand_state(Side::Left).supporting == Some(bar),
            "the other hand steadies the bar"
        );
        // The claw to the seam: the bar along a line up and back from it.
        let line = (back * 0.8 + Vec3::Y * 0.6).normalize();
        let hands_at = |line: Vec3, a: f32, b: f32| (seam + line * a, seam + line * b);
        let (r, l) = hands_at(line, (GRIP - CLAW).length(), (SHAFT - CLAW).length());
        for i in 1..=20 {
            let k = i as f32 / 20.0;
            vr.move_hand(Side::Right, grip.lerp(r, k));
            vr.move_hand(Side::Left, shaft.lerp(l, k));
            vr.step(1);
        }
        vr.step(10);
        let seat = vr
            .app
            .world()
            .get::<crate::interact::tools::crowbar::Crowbar>(bar)
            .unwrap()
            .seated
            .expect("the claw seats on the seam");
        // From here the hands go round where it actually seated, on the
        // line it seated along.
        let (pivot, line) = (seat.pivot, seat.line);
        let hands_at = |line: Vec3, a: f32, b: f32| (pivot + line * a, pivot + line * b);
        // Slide the hands to where they'll lever from, then pull the
        // handle round the claw, 0.45 rad, and hold it there.
        let (r0, l0) = hands_at(line, out, steady);
        vr.move_hand(Side::Right, r0);
        vr.move_hand(Side::Left, l0);
        vr.step(2);
        // Only the lever counts (the claw may have seated on the way in).
        *vr.app.world_mut().resource_mut::<Strain>() = Strain::default();
        let bent = Quat::from_axis_angle(Vec3::Y.cross(back).normalize(), -0.45) * line;
        for i in 1..=10 {
            let k = i as f32 / 10.0;
            let now = line.slerp(bent, k);
            let (r, l) = hands_at(now, out, steady);
            vr.move_hand(Side::Right, r);
            vr.move_hand(Side::Left, l);
            vr.step(1);
        }
        vr.step(40);
    }

    /// A crowbar in both hands, its claw to the seam on a block's face
    /// 0.6 m ahead at waist height; then levered round the claw with the
    /// hands at `out` and `steady` metres up the bar from it. Would it give
    /// (100 N·m held for Destruction's PRY_HOLD)?
    fn pry(out: f32, steady: f32) -> bool {
        let mut vr = Vr::new("training_yard");
        vr.app
            .init_resource::<Strain>()
            .add_systems(FixedUpdate, strain);
        let feet = on_ground(&vr, OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let face = feet + Vec3::new(0.0, 1.0, -0.6);
        let block = vr
            .app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(1.0, 1.0, 0.4),
                CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
                Transform::from_translation(face - Vec3::Z * 0.2),
                Pryable {
                    needs: 100.0,
                    seam: Vec3::new(0.0, 0.0, 0.23),
                    along: Vec3::X,
                    held: 0.0,
                },
            ))
            .id();
        work_bar(&mut vr, feet, face + Vec3::Z * 0.03, Vec3::Z, out, steady);
        let _ = block;
        vr.app.world().resource::<Strain>().longest >= PRY_HOLD
    }

    /// Levered from the end of the bar with both hands, the seam gives.
    #[test]
    fn a_crowbar_levered_from_its_end_prises_the_seam() {
        assert!(pry(0.67, 0.4), "two hands at the end didn't move it");
    }

    /// A real seam: one of Marrow Creek's supply crates, its lid's edge
    /// (Destruction's). Levered from the end of the bar, the lid gives and
    /// the crate spills open.
    #[test]
    fn a_crowbar_prises_a_supply_crate_open() {
        let mut vr = Vr::new("marrow_creek");
        vr.app
            .init_resource::<Strain>()
            .add_systems(FixedUpdate, strain);
        let (krate, gt, seam) = {
            let mut q = vr.app.world_mut().query_filtered::<
                (Entity, &GlobalTransform, &Pryable),
                With<crate::items::loot::SupplyCrate>,
            >();
            let (e, gt, p) = q.iter(vr.app.world()).next().expect("a supply crate");
            (e, *gt, *p)
        };
        // Stand off the lid's seam side, facing it.
        let at = gt.transform_point(seam.seam);
        let back = (gt.rotation() * Vec3::NEG_Z).with_y(0.0).normalize();
        let feet = {
            let p = at + back * 0.75;
            on_ground(&vr, p.x, p.z)
        };
        vr.stand(feet, (-back.x).atan2(-back.z) + std::f32::consts::PI);
        work_bar(&mut vr, feet, at + back * 0.02, back, 0.67, 0.4);
        vr.step(30);
        assert!(
            vr.app.world().get_entity(krate).is_err(),
            "the crate's lid held (longest strain {:.2} s)",
            vr.app.world().resource::<Strain>().longest
        );
    }

    /// Choked up by the claw, the same pull moves nothing.
    #[test]
    fn a_crowbar_held_short_moves_nothing() {
        assert!(!pry(0.25, 0.12), "hands by the claw prised it");
    }
}

/// Bolt cutters, worked with both hands.
mod cutters {
    use super::*;
    use crate::destruction::hooks::Cuttable;
    use crate::interact::tools::cutters::{ChainLink, JAWS, LEFT, Padlock};

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    /// Chain-link 0.6 m ahead (or a gate in it, padlocked). Cutters in both
    /// hands (or only one), jaws to the wire (or the shackle), then the
    /// hands squeezed together. Is the panel open?
    fn cut(both: bool, locked: bool) -> bool {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let wire = feet + Vec3::new(0.0, 1.0, -0.6);
        let panel = vr
            .app
            .world_mut()
            .spawn((
                ChainLink::default(),
                RigidBody::Static,
                Collider::cuboid(2.0, 2.0, 0.04),
                CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
                Transform::from_translation(wire),
            ))
            .id();
        let cuttable = Cuttable { needs: 0.0 };
        let mut lock = None;
        if locked {
            lock = Some(
                vr.app
                    .world_mut()
                    .spawn((
                        Padlock { holds: panel },
                        cuttable,
                        Collider::cuboid(0.05, 0.07, 0.02),
                        CollisionLayers::new(
                            crate::world::Layer::Item,
                            [crate::world::Layer::World],
                        ),
                        Transform::from_translation(wire),
                    ))
                    .id(),
            );
        } else {
            vr.app.world_mut().entity_mut(panel).insert(cuttable);
        }
        vr.step(2);
        let tool = vr.spawn(
            "bolt_cutters",
            Transform::from_translation(feet + Vec3::new(0.0, 0.9, -0.3)),
        );
        vr.app
            .world_mut()
            .entity_mut(tool)
            .insert(RigidBody::Static);
        let l = vr.point(tool, |r| *r == GripRole::Primary);
        let r = vr.point(tool, |r| *r == GripRole::Support);
        vr.hand(Side::Left, l, Quat::IDENTITY, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(tool)
            .insert(RigidBody::Dynamic);
        if both {
            vr.hand(Side::Right, r, Quat::IDENTITY, 1.0, 0.0);
            vr.step(3);
        }
        // Handles spread, the jaws onto the wire.
        let reach = (LEFT.z - JAWS.z) + 0.04;
        let spread = 0.3;
        let at = |s: f32| {
            let back = wire + Vec3::Z * reach;
            (back - Vec3::X * s * 0.5, back + Vec3::X * s * 0.5)
        };
        let (l1, r1) = at(spread);
        for i in 1..=15 {
            let k = i as f32 / 15.0;
            vr.move_hand(Side::Left, l.lerp(l1, k));
            if both {
                vr.move_hand(Side::Right, r.lerp(r1, k));
            }
            vr.step(1);
        }
        vr.step(8);
        // Squeeze.
        for i in 1..=12 {
            let (l2, r2) = at(spread - (spread - 0.1) * i as f32 / 12.0);
            vr.move_hand(Side::Left, l2);
            if both {
                vr.move_hand(Side::Right, r2);
            }
            vr.step(1);
        }
        vr.step(4);
        let w = vr.app.world();
        // Cut, Destruction takes the cut thing away; a padlock's panel is
        // left open.
        match lock {
            Some(lock) => {
                w.get_entity(lock).is_err()
                    && w.get::<ChainLink>(panel).unwrap().cut
                    && w.get::<ColliderDisabled>(panel).is_some()
            }
            None => w.get_entity(panel).is_err(),
        }
    }

    /// Both hands squeezing the handles cut through the wire: the panel is
    /// a gap.
    #[test]
    fn bolt_cutters_squeezed_with_both_hands_cut_chain_link() {
        assert!(cut(true, false), "the wire held");
    }

    /// One hand can't close them.
    #[test]
    fn bolt_cutters_in_one_hand_cut_nothing() {
        assert!(!cut(false, false), "one hand cut the wire");
    }

    /// A padlocked gate: cut the shackle and the gate opens.
    #[test]
    fn bolt_cutters_through_a_padlock_open_its_gate() {
        assert!(cut(true, true), "the padlock held");
    }
}

/// First aid and food, with the hands.
mod first_aid {
    use super::*;
    use crate::health::Health;
    use crate::interact::first_aid::NEEDLE;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    fn setup() -> (Vr, Vec3) {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let mut h = vr.app.world_mut().get_mut::<Health>(vr.player).unwrap();
        h.hp = 50.0;
        h.bleeding = 1.0;
        (vr, feet)
    }

    /// Take `id` in the right hand from in front of you.
    fn take(vr: &mut Vr, feet: Vec3, id: &str) -> Entity {
        let at = feet + Vec3::new(0.2, 1.1, -0.35);
        let item = vr.spawn(id, Transform::from_translation(at));
        vr.app
            .world_mut()
            .entity_mut(item)
            .insert(RigidBody::Static);
        vr.hand(Side::Right, at, Quat::IDENTITY, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(item)
            .insert(RigidBody::Dynamic);
        vr.step(2);
        assert_eq!(
            vr.hand_state(Side::Right).holding,
            Some(item),
            "{id} in hand"
        );
        item
    }

    fn gone(vr: &Vr, e: Entity) -> bool {
        vr.app.world().get_entity(e).is_err()
    }

    /// Wind a dressing round the left forearm (or only hold it there).
    fn dress(turns: f32) -> (bool, f32, f32) {
        let (mut vr, feet) = setup();
        let d = take(&mut vr, feet, "field_dressing");
        let wrist = feet + Vec3::new(-0.05, 1.15, -0.4);
        vr.hand(Side::Left, wrist, Quat::IDENTITY, 0.0, 0.0);
        let centre = wrist + Vec3::Z * 0.12;
        let steps = (turns * 24.0) as usize;
        for i in 0..=steps.max(30) {
            let a = std::f32::consts::TAU * (i.min(steps) as f32 / 24.0);
            vr.move_hand(
                Side::Right,
                centre + Vec3::new(a.cos(), a.sin(), 0.0) * 0.06,
            );
            vr.step(1);
        }
        vr.step(3);
        let h = vr.app.world().get::<Health>(vr.player).unwrap();
        (gone(&vr, d), h.bleeding, h.hp)
    }

    /// Circled round the forearm, the dressing goes on and the bleeding
    /// stops.
    #[test]
    fn a_dressing_wound_round_the_forearm_stops_the_bleeding() {
        let (used, bleeding, hp) = dress(2.0);
        assert!(
            used && bleeding == 0.0 && hp > 50.0,
            "{used} {bleeding} {hp}"
        );
    }

    /// Only held against it, it doesn't.
    #[test]
    fn a_dressing_only_held_to_the_arm_does_nothing() {
        let (used, bleeding, _) = dress(0.0);
        assert!(!used && bleeding > 0.0, "{used} {bleeding}");
    }

    /// An injector's tip to the thigh (or out in front), and the trigger.
    fn jab(thigh: bool) -> bool {
        let (mut vr, feet) = setup();
        let inj = take(&mut vr, feet, "regen_injector");
        let to = if thigh {
            let torso = {
                let mut q = vr
                    .app
                    .world_mut()
                    .query_filtered::<&GlobalTransform, With<crate::interact::body::Torso>>();
                *q.iter(vr.app.world()).next().unwrap()
            };
            torso.transform_point(Vec3::new(0.11, -0.62, -0.04))
        } else {
            feet + Vec3::new(0.2, 1.2, -0.5)
        };
        // The item sits on the hand as it was taken, so the hand goes
        // where the tip must be, less the tip's place on the item.
        let off = {
            let gt = vr.app.world().get::<GlobalTransform>(inj).unwrap();
            gt.transform_point(NEEDLE) - vr.hand_state(Side::Right).world.translation
        };
        vr.move_hand(Side::Right, to - off);
        vr.step(8);
        vr.trigger(Side::Right, 1.0);
        vr.step(3);
        gone(&vr, inj)
    }

    #[test]
    fn an_injector_goes_in_at_the_thigh() {
        assert!(jab(true), "the thigh took no injection");
    }

    #[test]
    fn an_injector_in_the_air_does_nothing() {
        assert!(!jab(false), "injected into the air");
    }

    /// Held at the mouth a moment, food is eaten; a bottle only drunk once
    /// it's tipped up.
    #[test]
    fn food_at_the_mouth_is_eaten_and_a_bottle_drunk_tipped_up() {
        for (id, tip, eaten) in [
            ("canned_stew", false, true),
            ("water_bottle", false, false),
            ("water_bottle", true, true),
        ] {
            let (mut vr, feet) = setup();
            let food = take(&mut vr, feet, id);
            let mouth = feet + Vec3::new(0.0, 1.65 - 0.08, -0.07);
            let rot = if tip {
                Quat::from_rotation_x(1.4)
            } else {
                Quat::IDENTITY
            };
            vr.hand(Side::Right, mouth, rot, 1.0, 0.0);
            vr.step(140);
            assert_eq!(gone(&vr, food), eaten, "{id} tipped {tip}");
        }
    }
}

/// Gestures the world can read.
mod gestures {
    use super::*;
    use crate::interact::gestures::{BREATH, Gestures, HUSHED};
    use crate::robots::Noise;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    /// Noises heard: whose, how far.
    #[derive(Resource, Default)]
    struct Heard(Vec<(Option<Entity>, f32)>);

    fn listen(mut n: MessageReader<Noise>, mut heard: ResMut<Heard>) {
        heard.0.extend(n.read().map(|n| (n.source, n.radius)));
    }

    fn setup() -> (Vr, Vec3) {
        let mut vr = Vr::new("training_yard");
        vr.app
            .init_resource::<Heard>()
            .add_systems(FixedUpdate, listen);
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        (vr, feet)
    }

    /// Winded, you're heard breathing; a hand over your mouth cuts it.
    #[test]
    fn a_hand_over_the_mouth_quietens_hard_breathing() {
        for over in [false, true] {
            let (mut vr, feet) = setup();
            let at = if over {
                feet + Vec3::new(0.0, 1.65 - 0.08, -0.1)
            } else {
                feet + Vec3::new(0.3, 1.0, -0.2)
            };
            vr.hand(Side::Right, at, Quat::IDENTITY, 0.0, 0.0);
            vr.step(3);
            vr.app.world_mut().resource_mut::<Heard>().0.clear();
            // Winded throughout.
            for _ in 0..200 {
                vr.app
                    .world_mut()
                    .get_mut::<crate::player::Body>(vr.player)
                    .unwrap()
                    .stamina = 0.0;
                vr.step(1);
            }
            let hush = vr.app.world().get::<Gestures>(vr.player).unwrap().hush;
            assert_eq!(hush, over, "a hand over the mouth: {over}");
            let player = vr.player;
            let loudest = vr
                .app
                .world()
                .resource::<Heard>()
                .0
                .iter()
                .filter(|(by, _)| *by == Some(player))
                .map(|(_, r)| *r)
                .fold(0.0, f32::max);
            let want = if over { BREATH * HUSHED } else { BREATH };
            assert!(
                (loudest - want).abs() < 0.3,
                "breathing heard {loudest} m, want {want} (hand over mouth {over})"
            );
        }
    }

    /// An open palm raised over the head reads as one; a raised fist
    /// doesn't.
    #[test]
    fn a_raised_open_palm_is_read_and_a_fist_is_not() {
        for open in [true, false] {
            let (mut vr, feet) = setup();
            let up = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
            vr.hand(
                Side::Right,
                feet + Vec3::new(0.25, 1.85, -0.15),
                up,
                if open { 0.0 } else { 1.0 },
                0.0,
            );
            vr.step(60);
            let palm = vr.app.world().get::<Gestures>(vr.player).unwrap().palm;
            assert_eq!(palm, open, "open hand {open}");
        }
    }
}

/// What a hand feels meeting things.
mod touch {
    use super::*;
    use crate::interact::touch::{HandStruck, Stuff};
    use crate::rig::shapes::{HapticShape, METAL, STONE, WIRE};

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    #[derive(Resource, Default)]
    struct Felt {
        shapes: Vec<HapticShape>,
        struck: Vec<HandStruck>,
    }

    fn feel(
        mut s: MessageReader<HapticShape>,
        mut h: MessageReader<HandStruck>,
        mut felt: ResMut<Felt>,
    ) {
        felt.shapes.extend(s.read().copied());
        felt.struck.extend(h.read().copied());
    }

    /// A wall 0.5 m ahead (of the world's, metal if asked, or a fence); the
    /// right hand brought into it from 0.3 m back at `speed` m/s. What did
    /// the hand feel, and was it heard?
    fn meet(what: Option<crate::render::materials::Mat>, fence: bool, speed: f32) -> Felt {
        let mut vr = Vr::new("training_yard");
        vr.app.init_resource::<Felt>().add_systems(Update, feel);
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let mut wall = vr.app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(3.0, 3.0, 0.2),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            Transform::from_translation(feet + Vec3::new(0.0, 1.2, -0.6)),
        ));
        if let Some(m) = what {
            wall.insert(crate::world::surfaces::Surfaces(vec![(
                Vec3::new(1.5, 1.5, 0.1),
                Transform::IDENTITY,
                m,
            )]));
        }
        if fence {
            wall.insert(crate::world::structures::Fence);
        }
        vr.step(2);
        let from = feet + Vec3::new(0.2, 1.2, -0.15);
        vr.hand(Side::Right, from, Quat::IDENTITY, 0.0, 0.0);
        vr.step(20);
        vr.app.world_mut().resource_mut::<Felt>().shapes.clear();
        let per_tick = speed / 90.0;
        let steps = (0.45 / per_tick).ceil() as usize;
        for i in 1..=steps {
            vr.move_hand(Side::Right, from - Vec3::Z * per_tick * i as f32);
            vr.step(1);
        }
        vr.step(10);
        std::mem::take(&mut *vr.app.world_mut().resource_mut::<Felt>())
    }

    /// A slow hand on stone: its shape, light, and not heard.
    #[test]
    fn a_slow_touch_on_stone_is_felt_lightly_and_quiet() {
        let f = meet(None, false, 0.3);
        let s = f
            .shapes
            .iter()
            .find(|s| s.side == Side::Right && s.shape == STONE)
            .unwrap_or_else(|| panic!("stone felt: {:?}", f.shapes));
        assert!(s.scale < 0.6, "a touch, not a blow: {}", s.scale);
        assert!(f.struck.is_empty(), "a touch was heard");
    }

    /// A slap on a metal wall: the ring, full, and heard as metal.
    #[test]
    fn a_slap_on_metal_rings_and_is_heard() {
        let f = meet(Some(crate::render::materials::Mat::RustyMetal), false, 4.0);
        let s = f
            .shapes
            .iter()
            .find(|s| s.side == Side::Right && s.shape == METAL)
            .unwrap_or_else(|| panic!("metal felt: {:?}", f.shapes));
        assert!(s.scale > 0.8, "a blow: {}", s.scale);
        assert!(
            f.struck.iter().any(|h| h.stuff == Stuff::Metal),
            "heard: {:?}",
            f.struck
        );
    }

    /// A fence rattles in the hand.
    #[test]
    fn a_fence_rattles() {
        let f = meet(None, true, 1.0);
        assert!(
            f.shapes.iter().any(|s| s.shape == WIRE),
            "wire felt: {:?}",
            f.shapes
        );
    }
}

/// The body you see: where its feet, knees, shoulders and slots are as the
/// head and hands move (`rig::stance`).
#[cfg(test)]
mod body {
    use super::*;
    use crate::interact::body::BodySlot;
    use crate::interact::{Socket, SocketKind};
    use crate::rig::stance::{PROTRACT, Stance};
    use extraction_core::inventory::SlotKind;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    fn standing() -> (Vr, Vec3) {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        for side in [Side::Left, Side::Right] {
            vr.hand_local(side, Vec3::new(0.25 * side.sign(), 0.95, -0.1), 0.0);
        }
        vr.step(90);
        (vr, feet)
    }

    fn stance(vr: &Vr) -> Stance {
        vr.app.world().get::<Stance>(vr.player).unwrap().clone()
    }

    fn look(vr: &mut Vr, at: Vec3, yaw: f32, pitch: f32) {
        vr.app
            .world_mut()
            .get_mut::<RigInput>(vr.player)
            .unwrap()
            .head = Transform::from_translation(at).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            yaw,
            pitch,
            0.0,
        ));
    }

    /// Where a body slot of `kind` is in the world.
    fn slot(vr: &mut Vr, kind: SlotKind) -> Vec3 {
        let mut q = vr
            .app
            .world_mut()
            .query::<(&BodySlot, &Socket, &GlobalTransform)>();
        q.iter(vr.app.world())
            .find(|(_, s, _)| s.kind == SocketKind::Body(kind))
            .map(|(_, _, g)| g.translation())
            .unwrap()
    }

    /// Looking round (left, right, down at your feet) moves neither foot.
    #[test]
    fn feet_stay_planted_when_you_turn_your_head() {
        let (mut vr, feet) = standing();
        let st = stance(&vr);
        assert!(st.ready);
        let planted = st.feet.map(|f| f.at);
        let _ = feet;
        for f in planted {
            assert!(
                (f.y - vr.feet().y).abs() < 0.01,
                "a foot on the ground: {f}"
            );
        }
        assert!(planted[0].x < planted[1].x, "left foot on the left");
        for (yaw, pitch) in [
            (0.7, 0.0),
            (-0.7, 0.0),
            (0.0, -1.1),
            (0.5, -0.8),
            (0.0, 0.0),
        ] {
            look(&mut vr, Vec3::new(0.0, 1.65, 0.0), yaw, pitch);
            vr.step(60);
            let now = stance(&vr).feet.map(|f| f.at);
            for i in 0..2 {
                assert!(
                    now[i].distance(planted[i]) < 0.005,
                    "looking ({yaw}, {pitch}) moved a foot: {} to {}",
                    planted[i],
                    now[i]
                );
            }
        }
    }

    /// Turning the whole body round (the head stays turned) and stepping
    /// across the room: the feet follow, a step at a time, and end up
    /// under you, toes the way you face.
    #[test]
    fn the_feet_step_after_a_body_that_turns_or_walks() {
        let (mut vr, feet) = standing();
        look(&mut vr, Vec3::new(0.0, 1.65, 0.0), 2.2, 0.0);
        let mut stepping_both = false;
        // The torso comes round lazily; the feet a step at a time, the last
        // squaring up once the body's settled.
        for _ in 0..420 {
            vr.step(1);
            let st = stance(&vr);
            stepping_both |= st.feet.iter().all(|f| !f.planted());
        }
        assert!(!stepping_both, "one foot at a time");
        let st = stance(&vr);
        for f in &st.feet {
            let diff = (f.yaw - 2.2).rem_euclid(std::f32::consts::TAU);
            assert!(
                diff < 0.6 || diff > std::f32::consts::TAU - 0.6,
                "toes {} for 2.2",
                f.yaw
            );
        }
        // A room-scale step: the head (and the body) half a metre aside.
        look(&mut vr, Vec3::new(0.6, 1.65, 0.0), 2.2, 0.0);
        vr.step(240);
        let st = stance(&vr);
        let under = st.pelvis.translation;
        for f in &st.feet {
            let d = Vec2::new(f.at.x - under.x, f.at.z - under.z).length();
            assert!(d < 0.3, "a foot {d} m from under the body");
            assert!((f.at.y - vr.feet().y).abs() < 0.01);
            let _ = feet;
        }
    }

    /// Crouching: the pelvis comes down, the knees bend forward, and the
    /// feet stay put; the bones keep their lengths.
    #[test]
    fn a_crouch_bends_the_knees_over_planted_feet() {
        let (mut vr, _) = standing();
        let before = stance(&vr);
        look(&mut vr, Vec3::new(0.0, 0.8, 0.0), 0.0, -0.3);
        vr.step(90);
        let st = stance(&vr);
        assert!(
            st.pelvis.translation.y < before.pelvis.translation.y - 0.3,
            "the pelvis came down: {} to {}",
            before.pelvis.translation.y,
            st.pelvis.translation.y
        );
        for i in 0..2 {
            assert!(
                st.feet[i].at.distance(before.feet[i].at) < 0.005,
                "a foot moved"
            );
            let ahead = st.pelvis.rotation * Vec3::NEG_Z;
            assert!(
                (st.knees[i] - st.hips[i]).dot(ahead) > 0.15,
                "knee {} not forward of hip {}",
                st.knees[i],
                st.hips[i]
            );
            let thigh = crate::rig::stance::THIGH * st.legs;
            assert!((st.knees[i].distance(st.hips[i]) - thigh).abs() < 0.01);
        }
    }

    /// Seated, the lap is under the eyes and the knees ahead of it, at the
    /// lap's height; the feet below the knees.
    #[test]
    fn seated_the_knees_are_ahead_of_the_lap() {
        let (mut vr, _) = standing();
        vr.app
            .world_mut()
            .resource_mut::<crate::rig::comfort::Comfort>()
            .seated = true;
        look(&mut vr, Vec3::new(0.0, 1.2, 0.0), 0.0, -0.6);
        vr.step(120);
        let st = stance(&vr);
        assert!(st.seated);
        let eye = vr
            .app
            .world()
            .get::<RigInput>(vr.player)
            .unwrap()
            .head
            .translation
            .y
            + vr.feet().y;
        assert!(
            (eye - st.pelvis.translation.y - 0.62).abs() < 0.05,
            "the lap under the eyes"
        );
        let ahead = st.pelvis.rotation * Vec3::NEG_Z;
        for i in 0..2 {
            assert!((st.knees[i] - st.hips[i]).dot(ahead) > 0.3, "knees ahead");
            assert!((st.knees[i].y - st.hips[i].y).abs() < 0.05, "thighs level");
            assert!(st.ankles[i].y < st.knees[i].y - 0.3, "shins down");
        }
    }

    /// The holster is on the gun hand's hip and the pockets on the thighs'
    /// fronts, where the drawn body has them: reaching down to them, the
    /// arm stays outside the body all the way.
    #[test]
    fn reaching_to_the_holster_keeps_the_arm_out_of_the_body() {
        let (mut vr, _) = standing();
        let holster = slot(&mut vr, SlotKind::Holster);
        let st = stance(&vr);
        assert!(
            holster.distance(st.hips[Side::Right.index()]) < 0.16,
            "the holster {holster} off the right hip {}",
            st.hips[1]
        );
        // The hand down to it, from the front, grip over the holster.
        vr.hand(
            Side::Right,
            holster + Vec3::Y * 0.06,
            Quat::from_rotation_x(-1.3),
            0.0,
            0.0,
        );
        vr.step(30);
        let st = stance(&vr);
        let grip = vr.hand_state(Side::Right).world;
        let wrist = grip.transform_point(Vec3::new(0.0, 0.0, 0.09));
        let hint = st.chest.rotation * Vec3::new(0.6, -1.0, 0.3).normalize();
        let axis = Vec2::new(st.chest.translation.x, st.chest.translation.z);
        let shoulder = st.shoulder(Side::Right);
        let (s, e) = crate::render::arms::arm_joints(shoulder, wrist, hint, axis, st.arm);
        assert_eq!(s, shoulder, "the shoulder stays on the body");
        // The body: an elliptic column round the spine, pelvis to neck.
        let inv = st.chest.rotation.inverse();
        let inside = |p: Vec3| {
            let along = (p - st.pelvis.translation).dot(st.chest.rotation * Vec3::Y);
            let q = inv * (p - st.pelvis.translation);
            let span = st.neck.distance(st.pelvis.translation);
            along > 0.0 && along < span && (q.x / 0.15).powi(2) + (q.z / 0.09).powi(2) < 1.0
        };
        for i in 1..20 {
            let t = i as f32 / 20.0;
            for p in [s.lerp(e, t), e.lerp(wrist, t)] {
                assert!(!inside(p), "the arm through the body at {p}");
            }
        }
    }

    /// Bending over (the head down and forward, looking down): the chest's
    /// pouches lean forward with the chest, out from under the eyes, and
    /// the belt's holster stays where a hand reaching down will find it.
    #[test]
    fn bending_over_the_pouches_lean_and_the_holster_stays() {
        let (mut vr, _) = standing();
        let pouch = slot(&mut vr, SlotKind::Pouch);
        let holster = slot(&mut vr, SlotKind::Holster);
        look(&mut vr, Vec3::new(0.0, 1.3, -0.25), 0.0, -1.1);
        vr.step(60);
        let bent_pouch = slot(&mut vr, SlotKind::Pouch);
        let bent_holster = slot(&mut vr, SlotKind::Holster);
        assert!(
            holster.distance(bent_holster) < 0.03,
            "the holster moved {}",
            holster.distance(bent_holster)
        );
        assert!(
            pouch.z - bent_pouch.z > 0.08,
            "the pouch {pouch} to {bent_pouch}: not forward with the chest"
        );
        let eye = vr.feet() + Vec3::new(0.0, 1.3, -0.25);
        assert!(
            eye.y - bent_pouch.y > 0.2,
            "the pouch {bent_pouch} at the eye {eye}"
        );
    }

    /// Straight out at arm's length and a little beyond: the shoulder
    /// comes forward (no further than a shoulder goes) and the drawn arm
    /// reaches the hand from it, not from somewhere off the body.
    #[test]
    fn a_full_reach_keeps_the_arm_on_its_shoulder() {
        let (mut vr, feet) = standing();
        let rest = stance(&vr).shoulder(Side::Right);
        let grip = rest + Vec3::new(0.05, -0.05, -0.74);
        vr.hand(Side::Right, grip, Quat::IDENTITY, 0.0, 0.0);
        vr.step(60);
        let st = stance(&vr);
        let shoulder = st.shoulder(Side::Right);
        assert!(
            shoulder.distance(rest) <= PROTRACT + 1e-3,
            "the shoulder came {} forward",
            shoulder.distance(rest)
        );
        assert!(shoulder.distance(rest) > 0.02, "it came forward");
        let hand = vr.hand_state(Side::Right).world;
        let wrist = hand.transform_point(Vec3::new(0.0, 0.0, 0.09));
        let hint = st.chest.rotation * Vec3::new(0.6, -1.0, 0.3).normalize();
        let axis = Vec2::new(st.chest.translation.x, st.chest.translation.z);
        let (s, _) = crate::render::arms::arm_joints(shoulder, wrist, hint, axis, st.arm);
        assert!(
            s.distance(shoulder) < 1e-3,
            "the arm pulled off its shoulder by {}",
            s.distance(shoulder)
        );
        let _ = feet;
    }
}
