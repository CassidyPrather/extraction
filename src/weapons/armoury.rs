//! What the later guns brought with them (`docs/ARMOURY.md`): a shot's
//! report against its calibre (the .22), and the RPK's folding bipod.

use bevy::prelude::*;

use super::Gun;
use crate::interact::{GrabPoint, Grabbable, GripRole, Hand};
use crate::items::{Catalog, Item};

/// The grab id of a bipod's legs (the action's handle is 0, a scope's
/// drum `zero::TURRET_HANDLE`).
pub const BIPOD_HANDLE: u8 = 3;
/// How far the legs swing from folded to deployed, radians about the
/// pivot's X (art/guns/rpk74.py).
pub const BIPOD_DEPLOY: f32 = -1.279;
/// How far a hand on the legs moves them down (or up) to swing them,
/// metres along the gun's up.
pub const BIPOD_SWING: f32 = 0.04;
/// Seconds the legs take to swing.
const BIPOD_TIME: f32 = 0.18;

/// A gun whose shot carries further or less far than its calibre says
/// (`report` in the catalog): robots' hearing multiplies by it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Report(pub f32);

/// A folding bipod: where it pivots and where its feet are (folded), gun
/// space, and whether it's down.
#[derive(Component, Clone, Copy, Debug)]
pub struct Bipod {
    pub pivot: Vec3,
    pub feet: Vec3,
    pub deployed: bool,
    /// 0 folded .. 1 deployed, easing toward `deployed`.
    pub swing: f32,
}

impl Bipod {
    /// The feet where they are now, gun space.
    pub fn feet_now(&self) -> Vec3 {
        let rot = Quat::from_rotation_x(BIPOD_DEPLOY * self.swing);
        self.pivot + rot * (self.feet - self.pivot)
    }
}

/// Each gun's bipod, gun space: (pivot, feet folded).
pub fn bipod_of(id: &str) -> Option<(Vec3, Vec3)> {
    match id {
        "rpk74" => Some((Vec3::new(0.0, 0.038, -0.6), Vec3::new(0.0, 0.0267, -0.8387))),
        _ => None,
    }
}

pub struct ArmouryPlugin;

impl Plugin for ArmouryPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(fit_out).add_systems(
            FixedUpdate,
            swing_bipods.after(crate::interact::InteractSystems::Follow),
        );
    }
}

/// A new gun gets its report and its bipod (and the bipod's grab point).
fn fit_out(
    add: On<Add, Gun>,
    mut commands: Commands,
    mut guns: Query<(&Gun, &Item, &mut Grabbable)>,
    catalog: Res<Catalog>,
) {
    let Ok((gun, item, mut grab)) = guns.get_mut(add.entity) else {
        return;
    };
    let id = &catalog.get(item.kind()).id;
    let report = gun.firearm.def.report;
    if report != 1.0 {
        commands.entity(add.entity).insert(Report(report));
    }
    if gun.firearm.def.cartridge == "26mm" {
        commands.entity(add.entity).insert(super::flares::Launcher);
    }
    if let Some((pivot, feet)) = bipod_of(id) {
        commands.entity(add.entity).insert(Bipod {
            pivot,
            feet,
            deployed: false,
            swing: 0.0,
        });
        grab.points.push(GrabPoint::new(
            Transform::from_translation(pivot.lerp(feet, 0.5)),
            GripRole::Handle(BIPOD_HANDLE),
            0.07,
        ));
    }
}

/// A hand on the legs swings them down or up; they ease there.
fn swing_bipods(
    hands: Query<&Hand>,
    mut guns: Query<(Entity, &mut Bipod, &GlobalTransform)>,
    time: Res<Time>,
) {
    for (e, mut bipod, gt) in &mut guns {
        for hand in &hands {
            if hand.handle != Some((e, BIPOD_HANDLE)) {
                continue;
            }
            let local = gt
                .affine()
                .inverse()
                .transform_point3(hand.world.translation);
            let d = (local - hand.handle_origin).y;
            if d < -BIPOD_SWING {
                bipod.deployed = true;
            } else if d > BIPOD_SWING {
                bipod.deployed = false;
            }
        }
        let want = if bipod.deployed { 1.0 } else { 0.0 };
        let step = time.delta_secs() / BIPOD_TIME;
        let swing = bipod.swing + (want - bipod.swing).clamp(-step, step);
        if swing != bipod.swing {
            bipod.swing = swing;
        }
    }
}

/// Through the headset path (`vr_harness`): each new gun loaded, cycled,
/// fired and cleared by hand.
#[cfg(test)]
mod vr_tests {
    use super::*;
    use crate::interact::GripRole;
    use crate::rig::Side;
    use crate::weapons::feel::vr_tests::*;
    use extraction_core::firearm::{FireMode, Malfunction, Round};

    /// The left hand's grip on the gun's forend, as `hold` takes it.
    fn take_support(r: &mut Range) {
        let support = r.vr.point(r.gun, |p| *p == GripRole::Support);
        r.vr.hand(Side::Left, support, Quat::IDENTITY, 1.0, 0.0);
        r.vr.step(20);
        assert_eq!(r.vr.hand_state(Side::Left).supporting, Some(r.gun));
    }

    /// Lets go with the left hand and takes it well away.
    fn let_go(r: &mut Range) {
        r.vr.grip(Side::Left, 0.0);
        r.vr.trigger(Side::Left, 0.0);
        r.vr.step(3);
        let away = r.feet + Vec3::new(-0.4, 1.0, 0.0);
        r.vr.move_hand(Side::Left, away);
        r.vr.step(3);
    }

    /// The supporting hand pumps the forend: back by `k` of its travel,
    /// then home.
    fn pump(r: &mut Range, k: f32) {
        let from = r.vr.hand_state(Side::Left).world.translation;
        let travel = gun_mut(r).layout.handle.as_ref().unwrap().travel;
        let back =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .back()
                .as_vec3();
        let to = from + back * travel * k;
        glide(r, from, to, 8);
        glide(r, to, from, 8);
        r.vr.step(2);
    }

    /// A shell from the air to the loading port, let go there.
    fn thumb_in(r: &mut Range, id: &str) {
        take(r, id, 1);
        let (port, _) = load_port(r);
        let away = r.vr.hand_state(Side::Left).world.translation;
        glide(r, away, port, 12);
        r.vr.step(2);
        r.vr.grip(Side::Left, 0.0);
        r.vr.step(2);
    }

    /// The MP-133 by hand: four shells into the tube through the port, the
    /// forend pumped by the supporting hand, a fifth topped up. Shot and
    /// pumped; then a short stroke jams a shell on the case, the trigger
    /// does nothing, and a full pump clears it.
    #[test]
    fn the_pump_loads_cycles_and_clears_a_short_stroke_by_hand() {
        let mut r = hold("mp133", false, false);
        gun_mut(&mut r).firearm.condition = 1.0;
        for n in 1..=4 {
            thumb_in(&mut r, "shell_12ga");
            assert_eq!(gun_mut(&mut r).firearm.internal_rounds, n, "shell {n}");
        }
        let_go(&mut r);
        take_support(&mut r);
        pump(&mut r, 1.2);
        assert_eq!(gun_mut(&mut r).firearm.chambers[0], Some(Round::Live));
        // Pumped on a live shell, the action's locked: nothing moves.
        pump(&mut r, 1.2);
        assert_eq!(gun_mut(&mut r).firearm.internal_rounds, 3, "locked");
        while gun_mut(&mut r).firearm.fire_mode() != FireMode::Semi {
            press(&mut r, "stick");
        }
        assert_eq!(fire(&mut r, 1, true).len(), 9, "nine pellets");
        pump(&mut r, 1.2);
        assert_eq!(gun_mut(&mut r).firearm.live_rounds(), 3);
        fire(&mut r, 1, true);
        // Short: far enough to let a shell out, not to throw the hull.
        pump(&mut r, 0.78);
        assert_eq!(
            gun_mut(&mut r).firearm.malfunction,
            Some(Malfunction::DoubleFeed),
            "a shell on the case"
        );
        assert!(fire(&mut r, 1, true).is_empty(), "jammed");
        pump(&mut r, 1.2);
        assert_eq!(gun_mut(&mut r).firearm.malfunction, None, "cleared");
        assert_eq!(fire(&mut r, 1, true).len(), 9);
    }

    /// The pump's action release (A): a live shell pumped out by hand.
    #[test]
    fn the_pump_releases_a_live_shell_with_its_button() {
        let mut r = hold("mp133", false, false);
        thumb_in(&mut r, "shell_12ga");
        thumb_in(&mut r, "shell_12ga");
        let_go(&mut r);
        take_support(&mut r);
        pump(&mut r, 1.2);
        assert_eq!(gun_mut(&mut r).firearm.internal_rounds, 1);
        press(&mut r, "a");
        pump(&mut r, 1.2);
        let f = &gun_mut(&mut r).firearm;
        assert_eq!(f.internal_rounds, 0, "the next one fed");
        assert_eq!(f.chambers[0], Some(Round::Live));
    }

    /// The left hand on the TOZ-8's bolt: lift and draw it back, a .22
    /// into the open chamber, close it and fire. A misfire (the round
    /// still in, the striker down) wants the bolt lifted and dropped.
    #[test]
    fn the_toz8_loads_its_chamber_fires_and_strikes_again() {
        let mut r = hold("toz8", false, false);
        let handle = r.vr.point(r.gun, |p| *p == GripRole::Handle(0));
        let (up, back) = {
            let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            (gt.up().as_vec3(), gt.back().as_vec3())
        };
        let travel = gun_mut(&mut r).layout.handle.as_ref().unwrap().travel;
        let work = |r: &mut Range, open: bool| {
            r.vr.hand(Side::Left, handle, Quat::IDENTITY, 0.0, 0.0);
            r.vr.step(30);
            r.vr.trigger(Side::Left, 1.0);
            r.vr.step(2);
            assert!(r.vr.hand_state(Side::Left).handle.is_some(), "on the bolt");
            let lifted = handle + up * 0.045;
            let drawn = lifted + back * travel * 1.15;
            if open {
                glide_side(r, Side::Left, handle, lifted, 15);
                glide_side(r, Side::Left, lifted, drawn, 15);
            } else {
                glide_side(r, Side::Left, handle, handle - back * travel * 1.1, 15);
                let home = handle - back * travel * 1.1;
                glide_side(r, Side::Left, home, home - up * 0.05, 15);
            }
            let_go(r);
        };
        work(&mut r, true);
        assert!(gun_mut(&mut r).firearm.bolt_travel > 0.9, "open");
        thumb_in(&mut r, "round_22lr");
        assert_eq!(gun_mut(&mut r).firearm.chambers[0], Some(Round::Live));
        work(&mut r, false);
        assert!(gun_mut(&mut r).firearm.is_closed(), "closed and down");
        assert_eq!(fire(&mut r, 1, true).len(), 1);
        // A misfire as it leaves one.
        {
            let mut g = gun_mut(&mut r);
            g.firearm.chambers[0] = Some(Round::Live);
            g.firearm.hammers_cocked[0] = false;
        }
        assert!(fire(&mut r, 1, true).is_empty(), "the striker's down");
        // Lift and drop.
        r.vr.hand(Side::Left, handle, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(30);
        r.vr.trigger(Side::Left, 1.0);
        r.vr.step(2);
        glide_side(&mut r, Side::Left, handle, handle + up * 0.045, 15);
        glide_side(&mut r, Side::Left, handle + up * 0.045, handle, 15);
        let_go(&mut r);
        assert_eq!(fire(&mut r, 1, true).len(), 1, "struck again");
    }

    /// The SVD: its box seated, racked by hand, ten aimed shots and the
    /// carrier holds open on the empty box.
    #[test]
    fn the_svd_runs_its_box_dry_and_holds_open() {
        let mut r = hold("svd", false, false);
        {
            let mut g = gun_mut(&mut r);
            g.firearm.condition = 1.0;
            g.firearm.fouling = 0.0;
        }
        seat_mag(&mut r, "svd_mag", 10);
        rack(&mut r);
        while gun_mut(&mut r).firearm.fire_mode() != FireMode::Semi {
            press(&mut r, "stick");
        }
        for k in 0..10 {
            assert_eq!(fire(&mut r, 1, true).len(), 1, "shot {k}");
        }
        assert!(gun_mut(&mut r).firearm.locked_back, "held open");
    }

    /// The RPK's bipod: the left hand takes the legs and swings them down;
    /// held so its feet are on a wall top (and nothing else is), the gun
    /// is rested; folded at the same height, it isn't.
    #[test]
    fn the_rpk_rests_on_its_bipod() {
        let mut r = hold("rpk74", false, false);
        let legs = r.vr.point(r.gun, |p| *p == GripRole::Handle(BIPOD_HANDLE));
        r.vr.hand(Side::Left, legs, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(20);
        r.vr.trigger(Side::Left, 1.0);
        r.vr.step(2);
        assert_eq!(
            r.vr.hand_state(Side::Left).handle.map(|h| h.1),
            Some(BIPOD_HANDLE),
            "on the legs"
        );
        let down =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .down()
                .as_vec3();
        glide_side(&mut r, Side::Left, legs, legs + down * 0.07, 10);
        let_go(&mut r);
        r.vr.step(30);
        let b = *r.vr.app.world().get::<Bipod>(r.gun).unwrap();
        assert!(b.deployed && b.swing >= 1.0, "{b:?}");
        // A block whose top is just under the feet, clear of the rest of
        // the gun.
        let feet = {
            let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            gt.transform_point(b.feet_now())
        };
        r.vr.app.world_mut().spawn((
            Transform::from_translation(feet - Vec3::Y * 0.11),
            avian3d::prelude::RigidBody::Static,
            avian3d::prelude::Collider::cuboid(0.3, 0.2, 0.12),
            avian3d::prelude::CollisionLayers::new(
                crate::world::Layer::World,
                avian3d::prelude::LayerMask::ALL,
            ),
        ));
        r.vr.step(10);
        let braced = |r: &mut Range| {
            r.vr.app
                .world()
                .get::<super::super::feel::Handling>(r.gun)
                .unwrap()
                .hold
                .braced
        };
        assert!(braced(&mut r), "rested on its feet");
        r.vr.app
            .world_mut()
            .get_mut::<Bipod>(r.gun)
            .unwrap()
            .deployed = false;
        r.vr.step(90);
        assert!(!braced(&mut r), "folded, nothing touches");
    }
}
