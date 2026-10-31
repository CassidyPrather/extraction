//! How each named part of a gun model moves with the mechanism.
//!
//! A model's top-level objects are named for how they move (`static*`,
//! `bolt*`, `turnbolt*`, `trigger*`, `selector*`, ...: [`moves_for`]); each
//! one's origin is where it moves from. [`delta`] says where a part stands
//! for a given [`Motion`] (the mechanism's state, plus what the hands are
//! doing to it), as a [`Delta`] applied to its rest pose. The game and the
//! bench both apply it, so the bench shows exactly what the game does.
//!
//! Engine-free: rotations are named by axis and angle; the caller builds
//! its own quaternions (`Quat::from_rotation_x(angle)` and so on).

use crate::firearm::{ActionKind, FireMode, Firearm};

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
    /// A crossbolt safety (the MP-133's): pushed through to the right on
    /// safe, slid left along -X to fire, baring its red band.
    Crossbolt,
    /// A door-bolt safety (the crossbow's): shot forward on safe, drawn back
    /// along +Z to fire.
    SafetyBolt,
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
    /// A turn-bolt's cocking piece: rides with the bolt, and stands back
    /// from it while the striker is cocked (the tell a bolt gun gives).
    CockingPiece,
    /// A cocking indicator pin over barrel N's lock: proud while it's
    /// cocked, flush when not.
    Indicator(u8),
    /// A crossbow's string at rest: shown until the draw starts.
    StringRest,
    /// A crossbow's string drawn into a V: its origin on the line through
    /// the prod's tips, stretched along Z with the draw.
    StringDrawn,
    /// A paddle magazine catch (the AK's): tips about its pin (the origin,
    /// local X), its paddle forward, as the thumb pushes it.
    MagCatch,
    /// A heel magazine catch (the PM's): drawn back along +Z out of the
    /// butt by the thumb.
    HeelCatch,
    /// A dust cover: lifts off the receiver's rear and slides back, the
    /// first step of stripping the gun (it has no part in firing).
    DustCover,
}

/// The model object names and how each moves. `turnbolt` before `bolt`:
/// neither is a prefix of the other, but the more specific names go first.
const NAMES: &[(&str, Moves)] = &[
    ("turnbolt", Moves::TurnBolt),
    ("cocking_piece", Moves::CockingPiece),
    ("indicator_0", Moves::Indicator(0)),
    ("indicator_1", Moves::Indicator(1)),
    ("bolt", Moves::WithBolt),
    ("barrels", Moves::Barrels),
    ("trigger", Moves::Trigger),
    ("selector", Moves::Selector),
    ("crossbolt", Moves::Crossbolt),
    ("safety_bolt", Moves::SafetyBolt),
    ("hammer", Moves::Hammer),
    ("cylinder", Moves::Cylinder),
    ("gate", Moves::Gate),
    ("ejector", Moves::Rod),
    ("bayonet", Moves::Bayonet),
    ("bipod", Moves::Bipod),
    ("string_rest", Moves::StringRest),
    ("string_drawn", Moves::StringDrawn),
    ("mag_catch_heel", Moves::HeelCatch),
    ("mag_catch", Moves::MagCatch),
    ("dust_cover", Moves::DustCover),
    ("static", Moves::Static),
];

/// How a model object named `name` moves; `None` if the name matches no
/// motion (and the game won't draw it).
pub fn moves_for(name: &str) -> Option<Moves> {
    NAMES
        .iter()
        .find(|(p, _)| name.starts_with(p))
        .map(|(_, m)| *m)
}

/// How far a cocked turn-bolt's cocking piece stands back off the bolt,
/// metres (a Mosin's, a TOZ-8's: a thumb's width you can see and feel).
pub const COCKED_BACK: f32 = 0.012;
/// How far a cocking indicator stands proud when its lock is cocked (m).
pub const INDICATOR_UP: f32 = 0.003;
/// How far a crossbolt safety slides left to fire, metres.
pub const CROSSBOLT_PUSH: f32 = 0.008;
/// How far a door-bolt safety draws back to fire, metres.
pub const SAFETY_BOLT_DRAW: f32 = 0.014;
/// How far back a cocked hammer tips, radians.
pub const HAMMER_BACK: f32 = 0.5;
/// How far a revolver's gate swings open, radians.
pub const GATE_OPEN: f32 = 1.4;
/// A revolver's ejector rod's stroke, metres.
pub const ROD_STROKE: f32 = 0.042;
/// How far a trigger moves back at full pull, metres.
pub const TRIGGER_TRAVEL: f32 = 0.006;
/// How far a selector turns per step from safe, radians.
pub const SELECTOR_STEP: f32 = 0.5;
/// How far a break action's barrels tip at full open, radians.
pub const BARRELS_OPEN: f32 = 0.6;
/// How far a folding bipod's legs swing down, radians.
pub const BIPOD_DEPLOY: f32 = -1.279;
/// How far a paddle magazine catch tips forward when pressed, radians.
pub const MAG_CATCH_TIP: f32 = 0.22;
/// How far a heel magazine catch draws back, metres.
pub const HEEL_CATCH_DRAW: f32 = 0.003;
/// How far a dust cover lifts at its rear and slides back, off (m).
pub const DUST_COVER_OFF: (f32, f32) = (0.02, 0.06);

/// An axis of gun space (+X right, +Y up, -Z down the bore).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Where a part stands, against its rest pose:
/// `translation = pivot + turn * (rest - pivot) + shift`,
/// `rotation = turn * rest`, `scale = rest * scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Delta {
    pub pivot: [f32; 3],
    /// A turn about `pivot`, if any.
    pub turn: Option<(Axis, f32)>,
    pub shift: [f32; 3],
    /// Multiplies the rest scale per axis (zero hides the part).
    pub scale: [f32; 3],
}

impl Delta {
    /// The rest pose itself.
    pub const REST: Delta = Delta {
        pivot: [0.0; 3],
        turn: None,
        shift: [0.0; 3],
        scale: [1.0; 3],
    };

    fn turn(pivot: [f32; 3], axis: Axis, angle: f32) -> Self {
        Self {
            pivot,
            turn: Some((axis, angle)),
            ..Self::REST
        }
    }

    fn shift(shift: [f32; 3]) -> Self {
        Self {
            shift,
            ..Self::REST
        }
    }
}

/// What moves a gun's parts: its mechanism, and what the hands are doing.
#[derive(Clone, Copy, Debug)]
pub struct Motion<'a> {
    pub firearm: &'a Firearm,
    /// The handle's full travel (m): the layout's.
    pub travel: f32,
    /// A self-loader's carrier or slide running back and home after a
    /// shot, 0..1 (faster than the mechanism's state shows it).
    pub stroke: f32,
    /// The trigger's pull, 0..1.
    pub trigger: f32,
    /// A break action's hinge, gun space.
    pub hinge: [f32; 3],
    /// A bayonet fixed (a folding one swung out).
    pub bayonet_fixed: bool,
    /// A bipod's legs swung down, 0..1.
    pub bipod: f32,
    /// Where the hand takes a crossbow's string (its Z), gun space.
    pub string_grab_z: Option<f32>,
    /// The magazine catch pressed, 0..1.
    pub mag_catch: f32,
    /// The dust cover off, 0..1.
    pub dust_cover: f32,
}

impl<'a> Motion<'a> {
    /// The gun at rest in the hand: nothing pressed, nothing swung.
    pub fn new(firearm: &'a Firearm, travel: f32) -> Self {
        Self {
            firearm,
            travel,
            stroke: 0.0,
            trigger: 0.0,
            hinge: [0.0; 3],
            bayonet_fixed: false,
            bipod: 0.0,
            string_grab_z: None,
            mag_catch: 0.0,
            dust_cover: 0.0,
        }
    }
}

/// Where a part that `moves` (its rest position `rest`, its axis through
/// `pivot`) stands for `m`.
pub fn delta(moves: Moves, rest: [f32; 3], pivot: [f32; 3], m: &Motion) -> Delta {
    let f = m.firearm;
    let bolt = f.bolt_travel.max(m.stroke);
    match moves {
        Moves::Static => Delta::REST,
        Moves::WithBolt => Delta::shift([0.0, 0.0, bolt * m.travel]),
        Moves::Cylinder => {
            let n = f.chambers.len().max(1) as f32;
            let turn = f.cylinder as f32 * std::f32::consts::TAU / n;
            Delta::turn(rest, Axis::Z, -turn)
        }
        Moves::Gate => {
            let open = if f.gate_open { GATE_OPEN } else { 0.0 };
            Delta::turn(rest, Axis::Y, open)
        }
        Moves::Bayonet => {
            // A self-loader's (the SKS's) folds; a bolt gun's is fixed.
            let folding = f.def.action == ActionKind::SemiAuto;
            let angle = if folding && m.bayonet_fixed {
                std::f32::consts::PI
            } else {
                0.0
            };
            Delta::turn(rest, Axis::X, angle)
        }
        Moves::StringRest => Delta {
            scale: if f.bolt_travel > 0.02 {
                [0.0; 3]
            } else {
                [1.0; 3]
            },
            ..Delta::REST
        },
        Moves::StringDrawn => {
            // Modelled drawn to the latch; stretched along Z about the
            // tips' line to wherever the string is.
            let rest_len = m.string_grab_z.map_or(0.0, |z| (z - rest[2]).abs());
            let full = rest_len + m.travel;
            let scale = if f.bolt_travel <= 0.02 || full <= 0.0 {
                [0.0; 3]
            } else {
                [1.0, 1.0, (rest_len + f.bolt_travel * m.travel) / full]
            };
            Delta {
                scale,
                ..Delta::REST
            }
        }
        Moves::Bipod => Delta::turn(rest, Axis::X, BIPOD_DEPLOY * m.bipod),
        Moves::Rod => Delta::shift([0.0, 0.0, f.rod * ROD_STROKE]),
        Moves::CockingPiece => {
            let cocked = f.hammers_cocked.first() == Some(&true);
            let back = if cocked { COCKED_BACK } else { 0.0 };
            Delta::shift([0.0, 0.0, f.bolt_travel * m.travel + back])
        }
        Moves::Crossbolt | Moves::SafetyBolt => {
            let shift = if f.fire_mode() == FireMode::Safe {
                [0.0; 3]
            } else if moves == Moves::Crossbolt {
                [-CROSSBOLT_PUSH, 0.0, 0.0]
            } else {
                [0.0, 0.0, SAFETY_BOLT_DRAW]
            };
            Delta::shift(shift)
        }
        Moves::Indicator(i) => {
            let up = f.hammers_cocked.get(i as usize) == Some(&true);
            Delta::shift([0.0, if up { INDICATOR_UP } else { 0.0 }, 0.0])
        }
        Moves::Hammer => {
            // Back with the slide's stroke too, as the slide rides over it.
            let back = if f.hammers_cocked.first() == Some(&true) {
                1.0
            } else {
                m.stroke.min(1.0)
            };
            Delta::turn(rest, Axis::X, HAMMER_BACK * back)
        }
        Moves::TurnBolt => {
            // Up about the bolt's axis, then back along it.
            let axis = [pivot[0], pivot[1], 0.0];
            Delta {
                pivot: axis,
                turn: Some((Axis::Z, f.handle_lift * std::f32::consts::FRAC_PI_2)),
                shift: [0.0, 0.0, f.bolt_travel * m.travel],
                scale: [1.0; 3],
            }
        }
        Moves::Barrels => Delta::turn(m.hinge, Axis::X, -BARRELS_OPEN * f.bolt_travel),
        Moves::Trigger => Delta::shift([0.0, 0.0, m.trigger * TRIGGER_TRAVEL]),
        Moves::Selector => {
            let k = match f.fire_mode() {
                FireMode::Safe => 0.0,
                FireMode::Auto => 0.5,
                FireMode::Semi => 1.0,
            };
            Delta::turn(rest, Axis::X, -SELECTOR_STEP * k)
        }
        Moves::MagCatch => Delta::turn(rest, Axis::X, MAG_CATCH_TIP * m.mag_catch),
        Moves::HeelCatch => Delta::shift([0.0, 0.0, HEEL_CATCH_DRAW * m.mag_catch]),
        Moves::DustCover => {
            let (up, back) = DUST_COVER_OFF;
            let k = m.dust_cover;
            // Up first at the rear (as the thumb presses the guide's button
            // and the cover springs), then back off the rails.
            let lift = (k * 2.0).min(1.0);
            let slide = (k * 2.0 - 1.0).max(0.0);
            Delta::shift([0.0, up * lift, back * slide])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_find_their_motions() {
        assert_eq!(moves_for("turnbolt_mosin"), Some(Moves::TurnBolt));
        assert_eq!(moves_for("bolt_carrier"), Some(Moves::WithBolt));
        assert_eq!(moves_for("selector_safety"), Some(Moves::Selector));
        assert_eq!(moves_for("static_butt"), Some(Moves::Static));
        assert_eq!(moves_for("mag_catch"), Some(Moves::MagCatch));
        assert_eq!(moves_for("dust_cover"), Some(Moves::DustCover));
        assert_eq!(moves_for("mag_catch_heel"), Some(Moves::HeelCatch));
        assert_eq!(moves_for("pt_muzzle"), None);
    }
}
