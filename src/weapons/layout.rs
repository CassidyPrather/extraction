//! Where everything is on each gun: grips, muzzle, ejection port, sights,
//! magazine well, the moving parts, and the blockout geometry.
//!
//! Gun space: the origin is the primary grip (where the palm's grip point
//! sits), `-Z` is down the bore, `+Y` up, `+X` to the right. Dimensions
//! follow the real guns. When the modelled versions land, they will carry
//! the same named points as empties.

use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mat {
    Steel,
    Blued,
    Wood,
    Polymer,
    Bakelite,
    Black,
}

/// What a piece of geometry does when the mechanism moves (the gunsmith's).
pub use ::gunsmith::parts::Moves;

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Box(Vec3),
    /// Radius, length (along local Z).
    Cylinder(f32, f32),
}

#[derive(Clone, Debug)]
pub struct Part {
    pub shape: Shape,
    pub at: Transform,
    pub mat: Mat,
    pub moves: Moves,
}

fn part(shape: Shape, at: Transform, mat: Mat, moves: Moves) -> Part {
    Part {
        shape,
        at,
        mat,
        moves,
    }
}

fn bx(x: f32, y: f32, z: f32) -> Shape {
    Shape::Box(Vec3::new(x, y, z))
}

fn at(x: f32, y: f32, z: f32) -> Transform {
    Transform::from_xyz(x, y, z)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandleKind {
    /// A pistol slide: the whole top moves.
    Slide,
    /// A charging handle on the bolt carrier.
    ChargingHandle,
    /// A turn-bolt: lift, then pull.
    TurnBolt,
    /// A break-action's barrels: push down to open.
    BreakBarrels,
    /// A pump's forend: the supporting hand works it, back and forward
    /// along the bore (`grab` is the forend, where the support is).
    Pump,
    /// A crossbow's string: drawn back to the latch by hand.
    String,
}

#[derive(Clone, Debug)]
pub struct HandleLayout {
    pub kind: HandleKind,
    /// Where a hand takes hold.
    pub grab: Vec3,
    /// Full travel, metres, along +Z.
    pub travel: f32,
}

#[derive(Clone, Debug)]
pub struct GunLayout {
    /// The palm on the pistol grip (or the wrist of the stock).
    pub grip: Transform,
    /// The second hand's place.
    pub support: Option<Vec3>,
    pub muzzle: Vec3,
    /// Ejection port and the direction cases fly.
    pub port: Vec3,
    pub eject: Vec3,
    /// Eye point on the sight line, behind the rear sight.
    pub sight: Transform,
    /// Seat of an inserted magazine.
    pub magwell: Option<Transform>,
    pub handle: Option<HandleLayout>,
    /// Where loose rounds go: breeches, the internal magazine.
    pub load_port: Vec3,
    /// Hinge for break actions.
    pub hinge: Vec3,
    pub parts: Vec<Part>,
}

impl GunLayout {
    /// The blockout's bounds, (low, high) corners in gun space.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        self.parts.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), p| {
                let half = match p.shape {
                    Shape::Box(s) => s * 0.5,
                    Shape::Cylinder(r, l) => Vec3::new(r, r, l * 0.5),
                };
                (
                    lo.min(p.at.translation - half),
                    hi.max(p.at.translation + half),
                )
            },
        )
    }
}

/// Where the other hand takes a handgun's grip from below, in the grip's
/// frame: by the magazine's foot, so a hand closing there cups the gun
/// rather than taking its magazine (`weapons::reload`
/// `a_grab_takes_the_gun_never_the_magazine_in_it`, the playtest's B2).
pub const CUP: Vec3 = Vec3::new(0.0, -0.065, 0.02);

/// Where the cupping palm is drawn, in the grip's frame (`render::props`
/// `support_pose`): palm up against the butt, its fingers forward and up
/// the front, short of the gun hand's
/// (`a_cupping_hand_keeps_its_fingers_out_of_the_gun_hand`). Not the grab
/// point: a hand takes hold where the magazine can't win, and is drawn
/// where a hand looks right.
pub const CUP_DRAWN: Vec3 = Vec3::new(0.0, -0.05, -0.02);

/// A handgun's cup (gun space): a gun with no place for a second hand on
/// it is steadied by one under its grip.
pub fn cup(layout: &GunLayout) -> Option<Vec3> {
    layout
        .support
        .is_none()
        .then(|| layout.grip.transform_point(CUP))
}

/// The cup as a hand's pose (gun space): where the cupping palm is, turned
/// as the gun is.
pub fn cup_pose(layout: &GunLayout) -> Option<Transform> {
    cup(layout).map(Transform::from_translation)
}

pub fn layout(id: &str) -> Option<GunLayout> {
    match id {
        "pm_pistol" => Some(pm()),
        "ak74" => Some(ak74()),
        "mosin" => Some(mosin()),
        "toz_double" => Some(toz()),
        "sks" => Some(sks()),
        "nagant" => Some(nagant()),
        "ppsh" => Some(ppsh()),
        "mp133" => Some(mp133()),
        "svd" => Some(svd()),
        "toz8" => Some(toz8()),
        "rpk74" => Some(rpk74()),
        "flare_pistol" => Some(flare_pistol()),
        "crossbow" => Some(crossbow()),
        _ => None,
    }
}

fn pm() -> GunLayout {
    use Mat::*;
    use Moves::*;
    // The grip's bottom sits back toward the shooter (+Z).
    let rake = Quat::from_rotation_x(-0.28);
    // Above the grip the model is 8.3% shorter along the bore than first
    // made (docs/craft/guns/pm_pistol): the muzzle, port, sights and slide
    // come in with it.
    GunLayout {
        grip: Transform::IDENTITY,
        support: None,
        muzzle: Vec3::new(0.0, 0.062, -0.1192),
        port: Vec3::new(0.012, 0.068, -0.0413),
        eject: Vec3::new(1.0, 0.6, 0.15),
        // The eye line: through the rear notch (its floor 79.8 mm) onto the
        // front blade's top.
        sight: at(0.0, 0.0805, 0.0275),
        // The well's mouth, at the top of the grip; magazines hang from it,
        // raked back with the grip.
        magwell: Some(
            Transform::from_xyz(0.0, 0.0475, -0.002).with_rotation(Quat::from_rotation_x(-0.216)),
        ),
        handle: Some(HandleLayout {
            kind: HandleKind::Slide,
            grab: Vec3::new(0.0, 0.064, 0.0165),
            travel: 0.03,
        }),
        load_port: Vec3::new(0.0, 0.066, -0.0367),
        hinge: Vec3::ZERO,
        parts: vec![
            // Grip frame with bakelite panels, raked back.
            part(
                bx(0.026, 0.09, 0.034),
                Transform::from_xyz(0.0, -0.005, 0.004).with_rotation(rake),
                Black,
                Static,
            ),
            part(
                bx(0.03, 0.07, 0.028),
                Transform::from_xyz(0.0, -0.005, 0.006).with_rotation(rake),
                Bakelite,
                Static,
            ),
            // Frame under the slide, and the trigger guard.
            part(
                bx(0.024, 0.016, 0.12),
                at(0.0, 0.042, -0.058),
                Blued,
                Static,
            ),
            part(
                bx(0.006, 0.004, 0.05),
                at(0.0, 0.018, -0.045),
                Blued,
                Static,
            ),
            part(bx(0.006, 0.026, 0.004), at(0.0, 0.03, -0.07), Blued, Static),
            part(
                bx(0.004, 0.022, 0.006),
                at(0.0, 0.03, -0.035),
                Black,
                Trigger,
            ),
            // Barrel (visible at the muzzle) and the slide over it.
            part(
                Shape::Cylinder(0.006, 0.083),
                at(0.0, 0.062, -0.078),
                Blued,
                Static,
            ),
            part(
                bx(0.028, 0.029, 0.145),
                at(0.0, 0.0625, -0.0485),
                Blued,
                WithBolt,
            ),
            // Sights.
            part(
                bx(0.004, 0.006, 0.006),
                at(0.0, 0.079, -0.1146),
                Blued,
                WithBolt,
            ),
            part(
                bx(0.014, 0.006, 0.005),
                at(0.0, 0.079, 0.0183),
                Blued,
                WithBolt,
            ),
            // Hammer and safety lever.
            part(
                bx(0.008, 0.014, 0.008),
                at(0.0, 0.062, 0.0275),
                Blued,
                Static,
            ),
            part(
                bx(0.003, 0.006, 0.014),
                at(-0.016, 0.066, 0.011),
                Blued,
                Selector,
            ),
        ],
    }
}

fn ak74() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let rake = Quat::from_rotation_x(0.3);
    let bore = 0.058;
    // Measured against the real gun (docs/craft/guns/ak74): the receiver
    // 32 mm further back than it was modelled, the barrel 53 mm shorter. The
    // support hand on the middle of the lower handguard.
    GunLayout {
        grip: Transform::IDENTITY,
        support: Some(Vec3::new(0.0, 0.03, -0.29)),
        muzzle: Vec3::new(0.0, bore, -0.637),
        port: Vec3::new(0.025, 0.07, -0.105),
        eject: Vec3::new(1.0, 0.35, 0.1),
        // The eye line: over the rear notch's floor onto the front post's
        // top (`capture::sightline`; it ran 2.6 mm under the notch).
        sight: at(0.0, 0.1071, 0.0),
        magwell: Some(
            Transform::from_xyz(0.0, 0.02, -0.12).with_rotation(Quat::from_rotation_x(-0.12)),
        ),
        handle: Some(HandleLayout {
            kind: HandleKind::ChargingHandle,
            grab: Vec3::new(0.045, 0.062, -0.138),
            travel: 0.11,
        }),
        load_port: Vec3::new(0.0, 0.07, -0.105),
        hinge: Vec3::ZERO,
        parts: vec![
            // Pistol grip.
            part(
                bx(0.03, 0.1, 0.038),
                Transform::from_xyz(0.0, -0.01, 0.005).with_rotation(rake),
                Polymer,
                Static,
            ),
            // Stamped receiver and dust cover.
            part(bx(0.044, 0.062, 0.25), at(0.0, 0.05, -0.1), Blued, Static),
            part(bx(0.04, 0.02, 0.22), at(0.0, 0.088, -0.09), Blued, Static),
            // Trigger guard and trigger.
            part(bx(0.006, 0.005, 0.07), at(0.0, 0.006, -0.04), Blued, Static),
            part(
                bx(0.005, 0.025, 0.006),
                at(0.0, 0.02, -0.03),
                Black,
                Trigger,
            ),
            // Barrel, front sight block, muzzle brake.
            part(
                Shape::Cylinder(0.0105, 0.42),
                at(0.0, bore, -0.43),
                Blued,
                Static,
            ),
            // (The barrel ahead of the gas block 53 mm shorter, as the
            // model's: the collider ends at the brake's face.)
            part(bx(0.025, 0.05, 0.03), at(0.0, 0.08, -0.547), Blued, Static),
            part(
                Shape::Cylinder(0.014, 0.08),
                at(0.0, bore, -0.602),
                Blued,
                Static,
            ),
            // Wooden lower and upper handguards, gas tube.
            part(bx(0.048, 0.042, 0.2), at(0.0, 0.045, -0.33), Wood, Static),
            part(bx(0.036, 0.024, 0.17), at(0.0, 0.088, -0.31), Wood, Static),
            // Rear sight block.
            part(bx(0.03, 0.02, 0.05), at(0.0, 0.096, -0.21), Blued, Static),
            // Bolt carrier with its charging handle (right side).
            part(
                bx(0.02, 0.02, 0.1),
                at(0.012, 0.072, -0.13),
                Steel,
                WithBolt,
            ),
            part(
                bx(0.03, 0.012, 0.016),
                at(0.035, 0.064, -0.17),
                Steel,
                WithBolt,
            ),
            // Selector lever, right side.
            part(
                bx(0.004, 0.012, 0.11),
                at(0.024, 0.06, -0.08),
                Blued,
                Selector,
            ),
            // Stock: wood, dropping toward the shoulder.
            part(
                bx(0.04, 0.095, 0.27),
                Transform::from_xyz(0.0, 0.012, 0.16).with_rotation(Quat::from_rotation_x(0.12)),
                Wood,
                Static,
            ),
            part(bx(0.044, 0.11, 0.014), at(0.0, -0.012, 0.3), Black, Static),
        ],
    }
}

fn mosin() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let bore = 0.045;
    GunLayout {
        grip: Transform::from_rotation(Quat::from_rotation_x(0.35)),
        support: Some(Vec3::new(0.0, 0.0, -0.45)),
        muzzle: Vec3::new(0.0, bore, -0.90),
        port: Vec3::new(0.0, 0.07, -0.136),
        eject: Vec3::new(0.7, 0.8, 0.2),
        sight: at(0.0, 0.084, -0.02),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::TurnBolt,
            grab: Vec3::new(0.065, 0.03, -0.103),
            travel: 0.09,
        }),
        load_port: Vec3::new(0.0, 0.07, -0.136),
        hinge: Vec3::ZERO,
        parts: vec![
            // The long wooden stock, wrist at the origin.
            part(bx(0.042, 0.06, 0.72), at(0.0, 0.0, -0.45), Wood, Static),
            part(
                bx(0.04, 0.12, 0.3),
                Transform::from_xyz(0.0, -0.03, 0.17).with_rotation(Quat::from_rotation_x(0.1)),
                Wood,
                Static,
            ),
            part(bx(0.036, 0.04, 0.12), at(0.0, -0.005, 0.0), Wood, Static),
            // Receiver and magazine box.
            part(bx(0.036, 0.04, 0.21), at(0.0, 0.045, -0.12), Blued, Static),
            part(bx(0.03, 0.05, 0.08), at(0.0, -0.01, -0.12), Blued, Static),
            part(
                bx(0.005, 0.022, 0.006),
                at(0.0, -0.005, -0.04),
                Black,
                Trigger,
            ),
            // Barrel and its bands; the upper handguard.
            part(
                Shape::Cylinder(0.011, 0.70),
                at(0.0, bore, -0.55),
                Blued,
                Static,
            ),
            part(bx(0.032, 0.02, 0.43), at(0.0, 0.057, -0.585), Wood, Static),
            part(bx(0.046, 0.07, 0.012), at(0.0, 0.02, -0.40), Blued, Static),
            part(bx(0.046, 0.07, 0.012), at(0.0, 0.02, -0.725), Blued, Static),
            part(bx(0.006, 0.02, 0.012), at(0.0, 0.07, -0.88), Blued, Static),
            part(bx(0.03, 0.018, 0.08), at(0.0, 0.065, -0.325), Blued, Static),
            // The bolt: body on the bore axis, the straight handle to the right.
            part(
                Shape::Cylinder(0.009, 0.14),
                at(0.0, 0.064, -0.10),
                Steel,
                TurnBolt,
            ),
            part(
                bx(0.06, 0.008, 0.008),
                at(0.035, 0.064, -0.103),
                Steel,
                TurnBolt,
            ),
            part(
                Shape::Cylinder(0.009, 0.014),
                Transform::from_xyz(0.066, 0.064, -0.103)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
                Steel,
                TurnBolt,
            ),
        ],
    }
}

/// The SKS carbine (art/guns/sks.py): a semi-auto with a fixed ten-round
/// magazine, loaded from clips through the open action at `load_port`.
fn sks() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let bore = 0.048;
    GunLayout {
        grip: Transform::from_rotation(Quat::from_rotation_x(0.3)),
        support: Some(Vec3::new(0.0, 0.01, -0.38)),
        muzzle: Vec3::new(0.0, bore, -0.713),
        // The open top of the receiver, between the rear sight and the cover.
        port: Vec3::new(0.0, 0.068, -0.179),
        eject: Vec3::new(0.6, 0.9, 0.1),
        sight: at(0.0, 0.078, -0.02),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::ChargingHandle,
            grab: Vec3::new(0.042, 0.054, -0.155),
            travel: 0.1,
        }),
        load_port: Vec3::new(0.0, 0.075, -0.179),
        hinge: Vec3::ZERO,
        parts: vec![
            // One-piece stock: forend, wrist and butt.
            part(bx(0.036, 0.03, 0.47), at(0.0, 0.017, -0.205), Wood, Static),
            part(bx(0.034, 0.05, 0.12), at(0.0, -0.002, 0.0), Wood, Static),
            part(
                bx(0.04, 0.11, 0.26),
                Transform::from_xyz(0.0, -0.025, 0.17).with_rotation(Quat::from_rotation_x(0.1)),
                Wood,
                Static,
            ),
            // Receiver and its cover, the fixed magazine box under it.
            part(bx(0.031, 0.043, 0.27), at(0.0, 0.04, -0.1), Blued, Static),
            part(
                bx(0.029, 0.024, 0.075),
                at(0.0, -0.02, -0.177),
                Blued,
                Static,
            ),
            part(
                bx(0.005, 0.022, 0.006),
                at(0.0, -0.022, -0.03),
                Black,
                Trigger,
            ),
            // Barrel, handguard over the gas tube, front sight.
            part(
                Shape::Cylinder(0.009, 0.51),
                at(0.0, bore, -0.458),
                Blued,
                Static,
            ),
            part(bx(0.029, 0.02, 0.166), at(0.0, 0.066, -0.363), Wood, Static),
            part(bx(0.018, 0.03, 0.02), at(0.0, 0.066, -0.69), Blued, Static),
            // Bolt carrier and the charging handle on the right.
            part(
                bx(0.017, 0.022, 0.11),
                at(0.0, 0.054, -0.14),
                Steel,
                WithBolt,
            ),
            part(
                bx(0.035, 0.012, 0.012),
                at(0.03, 0.054, -0.155),
                Steel,
                WithBolt,
            ),
        ],
    }
}

/// The Nagant revolver (art/guns/nagant.py): no handle; the gate, the
/// cylinder and the ejector rod work from the gun hand's buttons. Loose
/// rounds go in, and cases come out, at the loading gate on the right.
fn nagant() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let bore = 0.074;
    let gate = Vec3::new(0.016, 0.053, -0.012);
    GunLayout {
        // The palm on the grip, which sits back behind the hammer
        // (art/guns/nagant.py). Placed as on the PM: 29 mm ahead of the
        // backstrap at the palm's height, so the trigger lies 17 mm
        // further out than the PM's, as on the real guns (backstrap to
        // trigger 85.5 mm here, the PM's 68). A hand on a raked revolver
        // grip sits high, the web up under the hammer (y 0.014): the
        // bore's lever over the palm is 60 mm, and the Nagant's kick
        // (items.ron) is a straight shove so it doesn't read as the PM's
        // (`weapons::signature`).
        grip: Transform::from_xyz(0.0, 0.014, 0.018),
        support: None,
        muzzle: Vec3::new(0.0, bore, -0.178),
        port: gate,
        eject: Vec3::new(1.0, -0.2, 0.3),
        sight: at(0.0, 0.0835, 0.012),
        magwell: None,
        handle: None,
        load_port: gate,
        hinge: Vec3::ZERO,
        parts: vec![
            // Frame and grip.
            part(bx(0.022, 0.06, 0.11), at(0.0, 0.055, -0.02), Blued, Static),
            // The grip, set back behind the hammer and raked.
            part(
                bx(0.027, 0.077, 0.034),
                Transform::from_xyz(0.0, -0.004, 0.038).with_rotation(Quat::from_rotation_x(-0.22)),
                Wood,
                Static,
            ),
            // Barrel and cylinder.
            part(
                Shape::Cylinder(0.0082, 0.11),
                at(0.0, bore, -0.123),
                Blued,
                Static,
            ),
            part(
                Shape::Cylinder(0.0198, 0.04),
                at(0.0, 0.0618, -0.039),
                Blued,
                Static,
            ),
            part(
                bx(0.004, 0.024, 0.005),
                at(0.0, 0.016, -0.036),
                Black,
                Trigger,
            ),
        ],
    }
}

/// The PPSh-41 (art/guns/ppsh.py): an open-bolt gun, its charging handle
/// pulled right back to the sear; the drum seats under the receiver.
fn ppsh() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let bore = 0.052;
    GunLayout {
        grip: Transform::from_rotation(Quat::from_rotation_x(0.3)),
        support: Some(Vec3::new(0.0, 0.03, -0.3)),
        muzzle: Vec3::new(0.0, bore, -0.5143),
        port: Vec3::new(0.004, 0.062, -0.16),
        eject: Vec3::new(0.9, 0.7, 0.1),
        sight: at(0.0, 0.08, 0.0),
        magwell: Some(Transform::from_xyz(0.0, 0.03, -0.187)),
        handle: Some(HandleLayout {
            kind: HandleKind::ChargingHandle,
            grab: Vec3::new(0.04, 0.046, -0.115),
            travel: 0.1,
        }),
        load_port: Vec3::new(0.004, 0.062, -0.16),
        hinge: Vec3::ZERO,
        parts: vec![
            // Stock from behind the well to the butt.
            part(bx(0.04, 0.04, 0.13), at(0.0, 0.015, -0.065), Wood, Static),
            part(
                bx(0.04, 0.115, 0.27),
                Transform::from_xyz(0.0, -0.04, 0.18).with_rotation(Quat::from_rotation_x(0.1)),
                Wood,
                Static,
            ),
            // Receiver and the perforated jacket.
            part(bx(0.033, 0.06, 0.244), at(0.0, 0.04, -0.11), Blued, Static),
            part(
                Shape::Cylinder(0.0175, 0.288),
                at(0.0, bore, -0.370),
                Blued,
                Static,
            ),
            part(bx(0.005, 0.022, 0.006), at(0.0, 0.0, -0.04), Black, Trigger),
            // Bolt and its handle.
            part(
                bx(0.025, 0.022, 0.12),
                at(0.0, 0.047, -0.14),
                Steel,
                WithBolt,
            ),
            part(
                bx(0.03, 0.012, 0.012),
                at(0.03, 0.046, -0.115),
                Steel,
                WithBolt,
            ),
        ],
    }
}

/// The Baikal MP-133 (art/guns/mp133.py): a pump. The forend is both the
/// support and the handle; shells go into the tube through the loading
/// port under the receiver.
fn mp133() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let bore = 0.045;
    let forend = Vec3::new(0.0, 0.012, -0.42);
    GunLayout {
        grip: Transform::from_rotation(Quat::from_rotation_x(0.35)),
        support: Some(forend),
        muzzle: Vec3::new(0.0, bore, -0.795),
        port: Vec3::new(0.022, 0.05, -0.12),
        eject: Vec3::new(1.0, 0.5, 0.1),
        sight: at(0.0, 0.0665, -0.03),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::Pump,
            grab: forend,
            travel: 0.09,
        }),
        load_port: Vec3::new(0.0, 0.0, -0.13),
        hinge: Vec3::ZERO,
        parts: vec![
            // Stock and wrist.
            part(bx(0.038, 0.045, 0.14), at(0.0, -0.005, 0.03), Wood, Static),
            part(
                bx(0.042, 0.13, 0.26),
                Transform::from_xyz(0.0, -0.04, 0.22).with_rotation(Quat::from_rotation_x(0.12)),
                Wood,
                Static,
            ),
            // Receiver and trigger.
            part(bx(0.044, 0.06, 0.21), at(0.0, 0.035, -0.13), Blued, Static),
            part(
                bx(0.005, 0.022, 0.006),
                at(0.0, -0.01, -0.05),
                Black,
                Trigger,
            ),
            // Barrel over the tube.
            part(
                Shape::Cylinder(0.011, 0.56),
                at(0.0, bore, -0.515),
                Blued,
                Static,
            ),
            part(
                Shape::Cylinder(0.012, 0.39),
                at(0.0, 0.016, -0.43),
                Blued,
                Static,
            ),
            // The forend and the bolt it drives.
            part(bx(0.046, 0.04, 0.2), at(0.0, 0.016, -0.42), Wood, WithBolt),
            part(
                bx(0.02, 0.025, 0.08),
                at(0.0, 0.045, -0.13),
                Steel,
                WithBolt,
            ),
        ],
    }
}

/// The SVD (art/guns/svd.py): a self-loader from a ten-round box, its
/// charging handle on the right, a side rail for the PSO-1 on the left.
fn svd() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let bore = 0.058;
    GunLayout {
        grip: Transform::IDENTITY,
        support: Some(Vec3::new(0.0, 0.03, -0.4)),
        muzzle: Vec3::new(0.0, bore, -0.955),
        port: Vec3::new(0.0175, 0.0705, -0.148),
        eject: Vec3::new(1.0, 0.4, 0.1),
        sight: at(0.0, 0.1071, 0.0),
        magwell: Some(Transform::from_xyz(0.0, 0.025, -0.185)),
        handle: Some(HandleLayout {
            kind: HandleKind::ChargingHandle,
            grab: Vec3::new(0.047, 0.0645, -0.2),
            travel: 0.13,
        }),
        load_port: Vec3::new(0.0175, 0.0705, -0.148),
        hinge: Vec3::ZERO,
        parts: vec![
            // Thumbhole stock and its grip.
            part(
                bx(0.03, 0.1, 0.04),
                Transform::from_xyz(0.0, -0.01, 0.0).with_rotation(Quat::from_rotation_x(0.3)),
                Wood,
                Static,
            ),
            part(bx(0.04, 0.12, 0.25), at(0.0, 0.02, 0.15), Wood, Static),
            // Receiver, trigger, handguard, barrel.
            part(bx(0.044, 0.06, 0.3), at(0.0, 0.05, -0.12), Blued, Static),
            part(
                bx(0.005, 0.022, 0.006),
                at(0.0, 0.015, -0.035),
                Black,
                Trigger,
            ),
            part(bx(0.05, 0.05, 0.275), at(0.0, 0.055, -0.425), Wood, Static),
            part(
                Shape::Cylinder(0.01, 0.44),
                at(0.0, bore, -0.735),
                Blued,
                Static,
            ),
            // Carrier and handle.
            part(
                bx(0.02, 0.02, 0.1),
                at(0.012, 0.072, -0.17),
                Steel,
                WithBolt,
            ),
            part(
                bx(0.03, 0.012, 0.016),
                at(0.035, 0.066, -0.2),
                Steel,
                WithBolt,
            ),
            part(
                bx(0.004, 0.012, 0.11),
                at(0.024, 0.06, -0.08),
                Blued,
                Selector,
            ),
        ],
    }
}

/// The TOZ-8M (art/guns/toz8.py): a single-shot .22 turn-bolt; the round
/// goes into the open chamber at `load_port`.
fn toz8() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let bore = 0.04;
    GunLayout {
        grip: Transform::from_rotation(Quat::from_rotation_x(0.35)),
        support: Some(Vec3::new(0.0, 0.0, -0.4)),
        muzzle: Vec3::new(0.0, bore, -0.81),
        port: Vec3::new(0.0, 0.05, -0.15),
        eject: Vec3::new(0.7, 0.8, 0.2),
        sight: at(0.0, 0.062, -0.03),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::TurnBolt,
            grab: Vec3::new(0.05, 0.035, -0.06),
            travel: 0.075,
        }),
        load_port: Vec3::new(0.0, 0.05, -0.15),
        hinge: Vec3::ZERO,
        parts: vec![
            part(bx(0.036, 0.05, 0.6), at(0.0, -0.005, -0.33), Wood, Static),
            part(
                bx(0.036, 0.11, 0.28),
                Transform::from_xyz(0.0, -0.03, 0.16).with_rotation(Quat::from_rotation_x(0.1)),
                Wood,
                Static,
            ),
            part(
                Shape::Cylinder(0.014, 0.17),
                at(0.0, bore, -0.115),
                Blued,
                Static,
            ),
            part(
                Shape::Cylinder(0.008, 0.61),
                at(0.0, bore, -0.505),
                Blued,
                Static,
            ),
            part(
                bx(0.005, 0.022, 0.006),
                at(0.0, -0.01, -0.05),
                Black,
                Trigger,
            ),
            part(
                Shape::Cylinder(0.007, 0.1),
                at(0.0, bore, -0.1),
                Steel,
                TurnBolt,
            ),
            part(
                bx(0.05, 0.006, 0.006),
                at(0.027, 0.035, -0.06),
                Steel,
                TurnBolt,
            ),
        ],
    }
}

/// The RPK-74 (art/guns/rpk74.py): an AK with a long, heavy barrel, a
/// club-foot stock and a folding bipod near the muzzle.
fn rpk74() -> GunLayout {
    let mut l = ak74();
    let bore = 0.058;
    l.muzzle = Vec3::new(0.0, bore, -0.795);
    l.support = Some(Vec3::new(0.0, 0.03, -0.34));
    // Its own model (art/guns/rpk74.py) keeps the receiver where the AK's
    // was before the AK was measured.
    l.port = Vec3::new(0.025, 0.07, -0.13);
    l.load_port = Vec3::new(0.0, 0.07, -0.13);
    if let Some(h) = l.handle.as_mut() {
        h.grab = Vec3::new(0.045, 0.062, -0.17);
    }
    for p in &mut l.parts {
        // The barrel and its front sight and brake go further out (from
        // the AK's, which is 53 mm shorter there than this model's).
        if p.at.translation.z < -0.5 {
            p.at.translation.z -= 0.183;
        }
    }
    // The bipod, folded under the barrel (its pivot at the clamp).
    l.parts.push(part(
        bx(0.1, 0.012, 0.24),
        at(0.0, 0.032, -0.72),
        Mat::Steel,
        Moves::Bipod,
    ));
    l
}

/// The SPSh-44 (art/guns/flare_pistol.py): a 26 mm break-open signal
/// pistol; the barrel tips down about the hinge forward of its breech.
fn flare_pistol() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let bore = 0.055;
    // Forward, at the front of the frame's side plate (art/guns/flare_pistol.py).
    let hinge = Vec3::new(0.0, 0.028, -0.0755);
    GunLayout {
        // The palm 29 mm ahead of the backstrap at its height, as on every
        // gun (docs/craft/GUNSMITHING.md): the backstrap is at z 0.035.
        grip: Transform::from_xyz(0.0, 0.0, 0.006),
        support: None,
        muzzle: Vec3::new(0.0, bore, -0.162),
        port: Vec3::new(0.0, bore, -0.017),
        eject: Vec3::new(0.0, 0.6, 1.0),
        sight: at(0.0, 0.085, 0.01),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::BreakBarrels,
            grab: Vec3::new(0.0, bore, -0.13),
            travel: 0.0,
        }),
        load_port: Vec3::new(0.0, bore, -0.017),
        hinge,
        parts: vec![
            // Grip, raked back about 20 degrees.
            part(
                bx(0.03, 0.1, 0.045),
                Transform::from_xyz(0.0, -0.035, 0.025).with_rotation(Quat::from_rotation_x(-0.35)),
                Bakelite,
                Static,
            ),
            // Side plate and breech.
            part(bx(0.025, 0.05, 0.11), at(0.0, 0.04, -0.03), Blued, Static),
            part(
                bx(0.005, 0.03, 0.01),
                at(0.0, 0.002, -0.019),
                Black,
                Trigger,
            ),
            part(
                Shape::Cylinder(0.0155, 0.15),
                at(0.0, bore, -0.087),
                Blued,
                Barrels,
            ),
        ],
    }
}

/// The home-made crossbow (art/guns/crossbow.py): a leaf-spring prod on a
/// plank stock. The string is drawn back by hand to the latch; a bolt
/// goes in the groove at `load_port`.
fn crossbow() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let rail = 0.06;
    GunLayout {
        grip: Transform::from_rotation(Quat::from_rotation_x(0.3)),
        support: Some(Vec3::new(0.0, 0.0, -0.17)),
        muzzle: Vec3::new(0.0, rail, -0.35),
        port: Vec3::new(0.0, 0.07, -0.15),
        eject: Vec3::new(0.0, 1.0, 0.0),
        sight: at(0.0, 0.095, -0.017),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::String,
            grab: Vec3::new(0.0, 0.065, -0.22),
            // A 150 mm draw: what a leaf-spring prod gives
            // (docs/craft/guns/crossbow).
            travel: 0.15,
        }),
        load_port: Vec3::new(0.0, 0.07, -0.15),
        hinge: Vec3::ZERO,
        parts: vec![
            // Plank stock and tiller, the rail along its top.
            part(bx(0.035, 0.04, 0.29), at(0.0, 0.03, -0.135), Wood, Static),
            part(
                bx(0.035, 0.11, 0.3),
                Transform::from_xyz(0.0, -0.03, 0.2).with_rotation(Quat::from_rotation_x(0.1)),
                Wood,
                Static,
            ),
            part(
                bx(0.02, 0.01, 0.33),
                at(0.0, rail - 0.005, -0.185),
                Steel,
                Static,
            ),
            // The leaf-spring prod across the front.
            part(bx(0.6, 0.012, 0.04), at(0.0, 0.05, -0.29), Steel, Static),
            part(bx(0.005, 0.022, 0.006), at(0.0, 0.0, -0.05), Black, Trigger),
            // The string's middle.
            part(
                bx(0.06, 0.004, 0.004),
                at(0.0, 0.065, -0.22),
                Black,
                WithBolt,
            ),
        ],
    }
}

fn toz() -> GunLayout {
    use Mat::*;
    use Moves::*;
    // At the front of the action's bar (art/guns/toz_double.py).
    let hinge = Vec3::new(0.0, 0.010, -0.176);
    GunLayout {
        grip: Transform::from_rotation(Quat::from_rotation_x(0.35)),
        // On the slim forend's underside, mid-length.
        support: Some(Vec3::new(0.0, 0.011, -0.30)),
        muzzle: Vec3::new(0.0, 0.035, -0.8385),
        port: Vec3::new(0.0, 0.04, -0.12),
        eject: Vec3::new(0.0, 0.9, 0.9),
        sight: at(0.0, 0.0534, -0.05),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::BreakBarrels,
            grab: Vec3::new(0.0, 0.011, -0.30),
            travel: 0.0,
        }),
        load_port: Vec3::new(0.0, 0.04, -0.12),
        hinge,
        parts: vec![
            // Stock and wrist.
            part(bx(0.04, 0.045, 0.14), at(0.0, -0.005, 0.02), Wood, Static),
            part(
                bx(0.042, 0.13, 0.23),
                Transform::from_xyz(0.0, -0.035, 0.195).with_rotation(Quat::from_rotation_x(0.12)),
                Wood,
                Static,
            ),
            // Action (the receiver block) and trigger guard.
            part(bx(0.048, 0.055, 0.1), at(0.0, 0.015, -0.075), Steel, Static),
            // The action's bar under the chambers, out to the hinge.
            part(bx(0.048, 0.02, 0.07), at(0.0, 0.011, -0.15), Steel, Static),
            part(
                bx(0.006, 0.005, 0.06),
                at(0.0, -0.025, -0.06),
                Blued,
                Static,
            ),
            part(
                bx(0.005, 0.022, 0.006),
                at(0.0, -0.012, -0.05),
                Black,
                Trigger,
            ),
            // Top lever.
            part(bx(0.01, 0.006, 0.04), at(0.0, 0.045, -0.04), Blued, Static),
            // Barrels, rib and forend: all of it tips at the hinge.
            part(
                Shape::Cylinder(0.0125, 0.72),
                at(-0.0128, 0.035, -0.478),
                Blued,
                Barrels,
            ),
            part(
                Shape::Cylinder(0.0125, 0.72),
                at(0.0128, 0.035, -0.478),
                Blued,
                Barrels,
            ),
            part(bx(0.008, 0.006, 0.70), at(0.0, 0.05, -0.47), Blued, Barrels),
            part(bx(0.044, 0.02, 0.21), at(0.0, 0.016, -0.29), Wood, Barrels),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gun_in_the_catalog_has_a_layout() {
        let cat =
            extraction_core::items::ItemCatalog::from_ron(extraction_core::items::BUILTIN_CATALOG)
                .unwrap();
        for (_, def) in cat.iter() {
            if def.firearm.is_some() {
                assert!(layout(&def.id).is_some(), "{} has no layout", def.id);
            }
        }
    }

    /// The gunsmith's bench works each gun as the game does: the same
    /// handle travel and hinge (`gunsmith::spec`), and a gun for every one.
    #[test]
    fn the_bench_works_each_gun_as_the_game_does() {
        let cat =
            extraction_core::items::ItemCatalog::from_ron(extraction_core::items::BUILTIN_CATALOG)
                .unwrap();
        for (_, def) in cat.iter() {
            if def.firearm.is_none() {
                continue;
            }
            let l = layout(&def.id).unwrap();
            let s = ::gunsmith::spec::spec(&def.id)
                .unwrap_or_else(|| panic!("{} isn't on the bench", def.id));
            let travel = l.handle.as_ref().map_or(0.0, |h| h.travel);
            assert_eq!(s.travel, travel, "{}: travel", def.id);
            assert_eq!(s.hinge, l.hinge.to_array(), "{}: hinge", def.id);
        }
    }

    #[test]
    fn muzzles_are_ahead_of_grips() {
        for id in [
            "pm_pistol",
            "ak74",
            "mosin",
            "toz_double",
            "sks",
            "nagant",
            "ppsh",
            "mp133",
            "svd",
            "toz8",
            "rpk74",
            "flare_pistol",
            "crossbow",
        ] {
            let l = layout(id).unwrap();
            assert!(l.muzzle.z < l.grip.translation.z - 0.1, "{id}");
            if let Some(s) = l.support {
                assert!(s.z < l.grip.translation.z, "{id} support behind grip");
            }
        }
    }
}
