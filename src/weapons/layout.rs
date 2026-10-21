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

/// What a piece of geometry does when the mechanism moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moves {
    Static,
    /// Slides back along +Z with the bolt travel.
    WithBolt,
    /// Turns with the bolt-handle lift (around the bore), and slides.
    TurnBolt,
    /// Tips down around the hinge as the action breaks open.
    Barrels,
    /// Pivots with the trigger pull.
    Trigger,
    /// Rotates through the selector positions.
    Selector,
    /// An exposed hammer: tips back about its pin (the origin, local X)
    /// when cocked.
    Hammer,
    /// A revolver's cylinder: turns about its axis (the origin, local Z) a
    /// chamber at a time.
    Cylinder,
    /// A revolver's loading gate: swings about its upright hinge (the
    /// origin, local Y) out to the right when open.
    Gate,
    /// A revolver's ejector rod: strokes back along +Z as it pushes a case.
    Rod,
    /// A bayonet: a folding one turns about its pin (the origin, local X)
    /// from folded to fixed; a fixed one stays as modelled.
    Bayonet,
    /// A folding bipod: turns about its pivot (the origin, local X) from
    /// folded to deployed.
    Bipod,
    /// A crossbow's string at rest: shown until the draw starts.
    StringRest,
    /// A crossbow's string drawn into a V: its origin on the line through
    /// the prod's tips, stretched along Z with the draw.
    StringDrawn,
}

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

/// Where the other hand's palm cups a handgun's grip from below, in the
/// grip's frame: under the butt of the grip, wrapping the gun hand.
pub const CUP: Vec3 = Vec3::new(0.0, -0.065, 0.02);

/// A handgun's cup (gun space): a gun with no place for a second hand on
/// it is steadied by one under its grip.
pub fn cup(layout: &GunLayout) -> Option<Vec3> {
    layout
        .support
        .is_none()
        .then(|| layout.grip.transform_point(CUP))
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
    GunLayout {
        grip: Transform::IDENTITY,
        support: None,
        muzzle: Vec3::new(0.0, 0.062, -0.13),
        port: Vec3::new(0.012, 0.068, -0.045),
        eject: Vec3::new(1.0, 0.6, 0.15),
        sight: at(0.0, 0.079, 0.03),
        // The well's mouth, at the top of the grip; magazines hang from it,
        // raked back with the grip.
        magwell: Some(
            Transform::from_xyz(0.0, 0.0475, -0.002).with_rotation(Quat::from_rotation_x(-0.216)),
        ),
        handle: Some(HandleLayout {
            kind: HandleKind::Slide,
            grab: Vec3::new(0.0, 0.064, 0.018),
            travel: 0.03,
        }),
        load_port: Vec3::new(0.0, 0.066, -0.04),
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
                Shape::Cylinder(0.006, 0.09),
                at(0.0, 0.062, -0.085),
                Blued,
                Static,
            ),
            part(
                bx(0.028, 0.029, 0.158),
                at(0.0, 0.0625, -0.053),
                Blued,
                WithBolt,
            ),
            // Sights.
            part(
                bx(0.004, 0.006, 0.006),
                at(0.0, 0.079, -0.125),
                Blued,
                WithBolt,
            ),
            part(
                bx(0.014, 0.006, 0.005),
                at(0.0, 0.079, 0.02),
                Blued,
                WithBolt,
            ),
            // Hammer and safety lever.
            part(bx(0.008, 0.014, 0.008), at(0.0, 0.062, 0.03), Blued, Static),
            part(
                bx(0.003, 0.006, 0.014),
                at(-0.016, 0.066, 0.012),
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
    GunLayout {
        grip: Transform::IDENTITY,
        support: Some(Vec3::new(0.0, 0.03, -0.33)),
        muzzle: Vec3::new(0.0, bore, -0.69),
        port: Vec3::new(0.025, 0.07, -0.13),
        eject: Vec3::new(1.0, 0.35, 0.1),
        sight: at(0.0, 0.104, 0.0),
        magwell: Some(
            Transform::from_xyz(0.0, 0.02, -0.12).with_rotation(Quat::from_rotation_x(-0.12)),
        ),
        handle: Some(HandleLayout {
            kind: HandleKind::ChargingHandle,
            grab: Vec3::new(0.045, 0.062, -0.17),
            travel: 0.11,
        }),
        load_port: Vec3::new(0.0, 0.07, -0.13),
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
                Shape::Cylinder(0.0105, 0.48),
                at(0.0, bore, -0.46),
                Blued,
                Static,
            ),
            part(bx(0.025, 0.05, 0.03), at(0.0, 0.08, -0.6), Blued, Static),
            part(
                Shape::Cylinder(0.014, 0.08),
                at(0.0, bore, -0.655),
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
        muzzle: Vec3::new(0.0, bore, -0.98),
        port: Vec3::new(0.0, 0.07, -0.09),
        eject: Vec3::new(0.7, 0.8, 0.2),
        sight: at(0.0, 0.085, -0.02),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::TurnBolt,
            grab: Vec3::new(0.065, 0.03, -0.035),
            travel: 0.09,
        }),
        load_port: Vec3::new(0.0, 0.07, -0.09),
        hinge: Vec3::ZERO,
        parts: vec![
            // The long wooden stock, wrist at the origin.
            part(bx(0.042, 0.06, 0.62), at(0.0, 0.0, -0.4), Wood, Static),
            part(
                bx(0.04, 0.12, 0.3),
                Transform::from_xyz(0.0, -0.03, 0.17).with_rotation(Quat::from_rotation_x(0.1)),
                Wood,
                Static,
            ),
            part(bx(0.036, 0.04, 0.12), at(0.0, -0.005, 0.0), Wood, Static),
            // Receiver and magazine box.
            part(bx(0.036, 0.04, 0.17), at(0.0, 0.045, -0.1), Blued, Static),
            part(bx(0.03, 0.05, 0.08), at(0.0, -0.01, -0.12), Blued, Static),
            part(
                bx(0.005, 0.022, 0.006),
                at(0.0, -0.005, -0.04),
                Black,
                Trigger,
            ),
            // Barrel and its bands; the upper handguard.
            part(
                Shape::Cylinder(0.011, 0.82),
                at(0.0, bore, -0.57),
                Blued,
                Static,
            ),
            part(bx(0.032, 0.02, 0.3), at(0.0, 0.057, -0.3), Wood, Static),
            part(bx(0.046, 0.07, 0.012), at(0.0, 0.02, -0.5), Blued, Static),
            part(bx(0.046, 0.07, 0.012), at(0.0, 0.02, -0.7), Blued, Static),
            part(bx(0.006, 0.02, 0.012), at(0.0, 0.07, -0.96), Blued, Static),
            part(bx(0.03, 0.018, 0.08), at(0.0, 0.065, -0.2), Blued, Static),
            // The bolt: body on the bore axis, the straight handle to the right.
            part(
                Shape::Cylinder(0.009, 0.14),
                at(0.0, 0.064, -0.08),
                Steel,
                TurnBolt,
            ),
            part(
                bx(0.06, 0.008, 0.008),
                at(0.035, 0.064, -0.035),
                Steel,
                TurnBolt,
            ),
            part(
                Shape::Cylinder(0.009, 0.014),
                Transform::from_xyz(0.066, 0.064, -0.035)
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
        muzzle: Vec3::new(0.0, bore, -0.735),
        // The open top of the receiver, between the rear sight and the cover.
        port: Vec3::new(0.0, 0.068, -0.19),
        eject: Vec3::new(0.6, 0.9, 0.1),
        sight: at(0.0, 0.0875, -0.02),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::ChargingHandle,
            grab: Vec3::new(0.042, 0.054, -0.205),
            travel: 0.1,
        }),
        load_port: Vec3::new(0.0, 0.075, -0.2),
        hinge: Vec3::ZERO,
        parts: vec![
            // One-piece stock: forend, wrist and butt.
            part(bx(0.036, 0.04, 0.5), at(0.0, 0.01, -0.3), Wood, Static),
            part(bx(0.034, 0.05, 0.12), at(0.0, -0.002, 0.0), Wood, Static),
            part(
                bx(0.04, 0.115, 0.26),
                Transform::from_xyz(0.0, -0.04, 0.17).with_rotation(Quat::from_rotation_x(0.1)),
                Wood,
                Static,
            ),
            // Receiver and its cover, the fixed magazine box under it.
            part(
                bx(0.031, 0.043, 0.235),
                at(0.0, 0.04, -0.137),
                Blued,
                Static,
            ),
            part(bx(0.029, 0.034, 0.13), at(0.0, -0.03, -0.15), Blued, Static),
            part(
                bx(0.005, 0.022, 0.006),
                at(0.0, -0.022, -0.05),
                Black,
                Trigger,
            ),
            // Barrel, handguard over the gas tube, front sight.
            part(
                Shape::Cylinder(0.011, 0.48),
                at(0.0, bore, -0.5),
                Blued,
                Static,
            ),
            part(bx(0.029, 0.024, 0.2), at(0.0, 0.072, -0.37), Wood, Static),
            part(bx(0.018, 0.035, 0.02), at(0.0, 0.07, -0.72), Blued, Static),
            // Bolt carrier and the charging handle on the right.
            part(
                bx(0.017, 0.022, 0.11),
                at(0.0, 0.054, -0.19),
                Steel,
                WithBolt,
            ),
            part(
                bx(0.035, 0.012, 0.012),
                at(0.03, 0.054, -0.205),
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
        grip: Transform::IDENTITY,
        support: None,
        muzzle: Vec3::new(0.0, bore, -0.178),
        port: gate,
        eject: Vec3::new(1.0, -0.2, 0.3),
        sight: at(0.0, 0.086, 0.012),
        magwell: None,
        handle: None,
        load_port: gate,
        hinge: Vec3::ZERO,
        parts: vec![
            // Frame and grip.
            part(bx(0.022, 0.08, 0.08), at(0.0, 0.045, -0.03), Blued, Static),
            part(
                bx(0.026, 0.085, 0.032),
                Transform::from_xyz(0.0, -0.04, 0.012).with_rotation(Quat::from_rotation_x(-0.2)),
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
                bx(0.004, 0.02, 0.005),
                at(0.0, -0.005, -0.024),
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
        muzzle: Vec3::new(0.0, bore, -0.52),
        port: Vec3::new(0.004, 0.062, -0.16),
        eject: Vec3::new(0.9, 0.7, 0.1),
        sight: at(0.0, 0.08, 0.0),
        magwell: Some(Transform::from_xyz(0.0, 0.03, -0.152)),
        handle: Some(HandleLayout {
            kind: HandleKind::ChargingHandle,
            grab: Vec3::new(0.04, 0.046, -0.115),
            travel: 0.1,
        }),
        load_port: Vec3::new(0.004, 0.062, -0.16),
        hinge: Vec3::ZERO,
        parts: vec![
            // Stock from the well to the butt.
            part(bx(0.04, 0.04, 0.2), at(0.0, 0.015, -0.1), Wood, Static),
            part(
                bx(0.04, 0.115, 0.27),
                Transform::from_xyz(0.0, -0.04, 0.18).with_rotation(Quat::from_rotation_x(0.1)),
                Wood,
                Static,
            ),
            // Receiver and the perforated jacket.
            part(bx(0.033, 0.04, 0.21), at(0.0, 0.05, -0.094), Blued, Static),
            part(
                Shape::Cylinder(0.0175, 0.34),
                at(0.0, bore, -0.375),
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
        sight: at(0.0, 0.068, -0.03),
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
        sight: at(0.0, 0.1068, 0.0),
        magwell: Some(Transform::from_xyz(0.0, 0.025, -0.124)),
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
            part(bx(0.05, 0.05, 0.23), at(0.0, 0.055, -0.405), Wood, Static),
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
        muzzle: Vec3::new(0.0, bore, -0.84),
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
                Shape::Cylinder(0.008, 0.64),
                at(0.0, bore, -0.52),
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
    l.muzzle = Vec3::new(0.0, bore, -0.82);
    l.support = Some(Vec3::new(0.0, 0.03, -0.34));
    for p in &mut l.parts {
        // The barrel and its front sight and brake go further out.
        if p.at.translation.z < -0.55 {
            p.at.translation.z -= 0.13;
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
/// pistol; the barrel tips down about the hinge under its breech.
fn flare_pistol() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let bore = 0.055;
    let hinge = Vec3::new(0.0, 0.032, -0.065);
    GunLayout {
        grip: Transform::IDENTITY,
        support: None,
        muzzle: Vec3::new(0.0, bore, -0.19),
        port: Vec3::new(0.0, bore, -0.04),
        eject: Vec3::new(0.0, 0.6, 1.0),
        sight: at(0.0, 0.085, 0.01),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::BreakBarrels,
            grab: Vec3::new(0.0, bore, -0.15),
            travel: 0.0,
        }),
        load_port: Vec3::new(0.0, bore, -0.04),
        hinge,
        parts: vec![
            part(
                bx(0.03, 0.1, 0.04),
                Transform::from_xyz(0.0, -0.01, 0.01).with_rotation(Quat::from_rotation_x(-0.25)),
                Bakelite,
                Static,
            ),
            part(bx(0.026, 0.04, 0.08), at(0.0, 0.04, -0.02), Blued, Static),
            part(
                bx(0.005, 0.02, 0.005),
                at(0.0, 0.012, -0.03),
                Black,
                Trigger,
            ),
            part(
                Shape::Cylinder(0.017, 0.155),
                at(0.0, bore, -0.112),
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
        support: Some(Vec3::new(0.0, 0.0, -0.28)),
        muzzle: Vec3::new(0.0, rail, -0.46),
        port: Vec3::new(0.0, 0.07, -0.2),
        eject: Vec3::new(0.0, 1.0, 0.0),
        sight: at(0.0, 0.085, -0.027),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::String,
            grab: Vec3::new(0.0, 0.065, -0.33),
            travel: 0.26,
        }),
        load_port: Vec3::new(0.0, 0.07, -0.2),
        hinge: Vec3::ZERO,
        parts: vec![
            // Plank stock and tiller, the rail along its top.
            part(bx(0.035, 0.04, 0.5), at(0.0, 0.03, -0.2), Wood, Static),
            part(
                bx(0.035, 0.11, 0.3),
                Transform::from_xyz(0.0, -0.03, 0.2).with_rotation(Quat::from_rotation_x(0.1)),
                Wood,
                Static,
            ),
            part(
                bx(0.02, 0.01, 0.42),
                at(0.0, rail - 0.005, -0.25),
                Steel,
                Static,
            ),
            // The leaf-spring prod across the front.
            part(bx(0.6, 0.012, 0.04), at(0.0, 0.05, -0.4), Steel, Static),
            part(bx(0.005, 0.022, 0.006), at(0.0, 0.0, -0.05), Black, Trigger),
            // The string's middle.
            part(
                bx(0.06, 0.004, 0.004),
                at(0.0, 0.065, -0.33),
                Black,
                WithBolt,
            ),
        ],
    }
}

fn toz() -> GunLayout {
    use Mat::*;
    use Moves::*;
    let hinge = Vec3::new(0.0, 0.012, -0.11);
    GunLayout {
        grip: Transform::from_rotation(Quat::from_rotation_x(0.35)),
        support: Some(Vec3::new(0.0, 0.0, -0.36)),
        muzzle: Vec3::new(0.0, 0.035, -0.82),
        port: Vec3::new(0.0, 0.04, -0.12),
        eject: Vec3::new(0.0, 0.9, 0.9),
        sight: at(0.0, 0.07, -0.05),
        magwell: None,
        handle: Some(HandleLayout {
            kind: HandleKind::BreakBarrels,
            grab: Vec3::new(0.0, 0.0, -0.36),
            travel: 0.0,
        }),
        load_port: Vec3::new(0.0, 0.04, -0.12),
        hinge,
        parts: vec![
            // Stock and wrist.
            part(bx(0.04, 0.045, 0.14), at(0.0, -0.005, 0.02), Wood, Static),
            part(
                bx(0.042, 0.13, 0.28),
                Transform::from_xyz(0.0, -0.035, 0.22).with_rotation(Quat::from_rotation_x(0.12)),
                Wood,
                Static,
            ),
            // Action (the receiver block) and trigger guard.
            part(bx(0.048, 0.055, 0.1), at(0.0, 0.015, -0.075), Steel, Static),
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
                Shape::Cylinder(0.0125, 0.7),
                at(-0.0128, 0.035, -0.47),
                Blued,
                Barrels,
            ),
            part(
                Shape::Cylinder(0.0125, 0.7),
                at(0.0128, 0.035, -0.47),
                Blued,
                Barrels,
            ),
            part(bx(0.008, 0.006, 0.68), at(0.0, 0.05, -0.46), Blued, Barrels),
            part(bx(0.044, 0.035, 0.24), at(0.0, 0.008, -0.31), Wood, Barrels),
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
