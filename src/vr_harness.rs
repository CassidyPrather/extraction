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

    fn with(opts: crate::LaunchOptions, dir: Option<std::path::PathBuf>) -> Self {
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

    pub fn primary(&mut self, side: Side, down: bool) {
        let mut input = self
            .app
            .world_mut()
            .get_mut::<RigInput>(self.player)
            .unwrap();
        input.hand_mut(side).primary = down;
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

    /// Every panel's letters, as fitted to it, are big enough to read from
    /// where you'd read it (`render::screen::read_from`): the first headset
    /// session couldn't read the signs.
    #[test]
    fn every_panel_reads_from_where_you_stand() {
        use crate::raid::bench::ScreenText;
        use crate::render::screen::{MIN_EM_DEG, em_metres, read_from};
        let mut small = Vec::new();
        let mut seen = 0;
        for map in [
            "training_yard",
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
        ] {
            let dir =
                std::env::temp_dir().join(format!("extraction-text-{map}-{}", std::process::id()));
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
            for _ in 0..60 {
                app.update();
            }
            // Signs, notes and terminals are put up by the client: take them
            // from the map.
            let zone = app.world().resource::<crate::world::Zone>().clone();
            let mut panels = crate::world::notes::marker_panels(&zone.map);
            let mut q = app.world_mut().query::<(Option<&Name>, &ScreenText)>();
            panels.extend(
                q.iter(app.world())
                    .map(|(name, s)| (name.map_or("panel".into(), |n| n.to_string()), s.clone())),
            );
            for (name, screen) in &panels {
                if screen.text.trim().is_empty() {
                    continue;
                }
                seen += 1;
                let em = em_metres(screen);
                let at = read_from(screen);
                let deg = (em / at).atan().to_degrees();
                if deg < MIN_EM_DEG {
                    small.push(format!(
                        "{map}: {} ({:.2}x{:.2} m): {:.1} cm letters, {deg:.2} deg at {at} m: {:?}",
                        name,
                        screen.size.x,
                        screen.size.y,
                        em * 100.0,
                        screen.text.lines().next().unwrap_or("")
                    ));
                }
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        assert!(seen > 20, "panels seen: {seen}");
        assert!(small.is_empty(), "too small to read:\n{}", small.join("\n"));
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
        vr.primary(Side::Right, true);
        vr.step(3);
        vr.primary(Side::Right, false);
        vr.step(3);
        assert!(done(&vr, Goal::Dressing), "the dressing counted");
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
        assert!(said(&vr, "Too close"), "warned");
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
