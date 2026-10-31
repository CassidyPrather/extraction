//! The room inside each body slot's piece of the rig (`art/kit/rig.py`),
//! for logic to fit what's stowed in it (`interact::body`): each cavity's
//! floor centre and half extents, in its slot's space (origin the slot,
//! +Y up, -Z out from the chest), inside the piece's walls.
//! `capture::stowed` holds the rig's models to these numbers, so the art
//! and the numbers can't drift apart.

use bevy::math::Vec3;
use extraction_core::inventory::SlotKind;

/// A slot's cavity: the middle of its floor, and its half extents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cavity {
    pub floor: Vec3,
    pub half: Vec3,
}

impl Cavity {
    /// The cavity's middle.
    pub fn centre(&self) -> Vec3 {
        self.floor + Vec3::Y * self.half.y
    }
    /// Its top: the mouth (what's in it may stand out of it, up).
    pub fn mouth(&self) -> f32 {
        self.floor.y + 2.0 * self.half.y
    }
}

/// A chest pouch: a magazine stands in it, thin side across X.
pub const POUCH: Cavity = Cavity {
    floor: Vec3::new(0.0, -0.105, -0.0065),
    half: Vec3::new(0.018, 0.065, 0.0535),
};

/// A hip pocket: rounds, a clip, a cell.
pub const POCKET: Cavity = Cavity {
    floor: Vec3::new(0.0, -0.059, -0.0005),
    half: Vec3::new(0.029, 0.0355, 0.0135),
};

/// The holster: a pistol hung barrel down, slide forward (-Z), its grip
/// back and up out of the mouth.
pub const HOLSTER: Cavity = Cavity {
    floor: Vec3::new(0.0, -0.152, -0.0465),
    half: Vec3::new(0.016, 0.082, 0.0505),
};

/// The chest mount: an open loop on the strap (a light or radio slides
/// down through it; open at the bottom, so `floor` is only the loop's).
pub const MOUNT: Cavity = Cavity {
    floor: Vec3::new(0.0, -0.05, -0.008),
    half: Vec3::new(0.019, 0.026, 0.019),
};

/// A slot's cavity (`None`: the slings, the vest and the head, which hold
/// what's made to go there).
pub fn cavity(slot: SlotKind) -> Option<Cavity> {
    match slot {
        SlotKind::Pouch => Some(POUCH),
        SlotKind::Pocket => Some(POCKET),
        SlotKind::Holster => Some(HOLSTER),
        SlotKind::Mount => Some(MOUNT),
        SlotKind::Sling | SlotKind::Vest | SlotKind::Head => None,
    }
}

/// Models whose bounds' middle isn't their origin, among what goes in a
/// body slot (more than 5 mm off): the middle less the origin, in the
/// model's own space. Fitting an item by its catalog size
/// (`ItemDef::size_m`) centres its box; this puts its model there.
/// Everything else is centred on its origin. `capture::stowed` holds this
/// to the models.
pub const ORIGIN_OFFSET: &[(&str, Vec3)] = &[
    ("pm_pistol", Vec3::new(-0.001, 0.015, -0.044)),
    ("nagant", Vec3::new(0.0, -0.004, -0.061)),
    ("flare_pistol", Vec3::new(0.0, 0.0, -0.074)),
    ("rpk_mag", Vec3::new(0.0, 0.001, -0.026)),
    ("ammo_762x25", Vec3::new(0.0, -0.006, -0.001)),
    ("bolt_bundle", Vec3::new(0.0, -0.009, 0.0)),
    ("robot_ammo_belt", Vec3::new(0.0, -0.021, -0.002)),
    ("artifact_adapter", Vec3::new(-0.003, 0.015, 0.0)),
    ("map_case", Vec3::new(0.005, 0.003, 0.011)),
    ("lead_case", Vec3::new(0.0, 0.009, 0.0)),
    ("emp_trap", Vec3::new(0.0, -0.012, 0.0)),
    ("electronics", Vec3::new(0.0, -0.017, 0.002)),
    ("brass", Vec3::new(0.0, -0.01, 0.0)),
    ("rotor_assembly", Vec3::new(-0.006, -0.006, 0.011)),
    ("field_recorder", Vec3::new(0.0, 0.006, 0.0)),
];

/// An item's model's middle off its origin (zero for most).
pub fn origin_offset(id: &str) -> Vec3 {
    ORIGIN_OFFSET
        .iter()
        .find(|(k, _)| *k == id)
        .map_or(Vec3::ZERO, |(_, v)| *v)
}
