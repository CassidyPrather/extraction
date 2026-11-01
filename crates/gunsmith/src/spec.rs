//! What the bench needs to know about each gun beyond its model, and the
//! real gun's measurements its model is checked against.
//!
//! The handle's travel and the hinge are the game's layout's
//! (`src/weapons/layout.rs`); a test there holds the two in step.

/// One gun on the bench.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    /// The catalog id, and the model's file name (`models/guns/<id>.glb`).
    pub id: &'static str,
    /// The handle's (charging handle, slide, bolt, string) full travel, m.
    pub travel: f32,
    /// A break action's hinge, gun space.
    pub hinge: [f32; 3],
    /// The real gun's measurements, where they've been looked up.
    pub real: Option<Real>,
}

/// A real gun's measurements (millimetres, kilograms), from its manual
/// or a measured example; `docs/craft/guns/<id>/` cites each.
#[derive(Clone, Copy, Debug)]
pub struct Real {
    /// Muzzle (brake included) to the butt's back.
    pub length: f32,
    /// The barrel, breech face to muzzle (brake excluded).
    pub barrel: f32,
    /// Front sight's blade to the rear sight's notch.
    pub sight_radius: f32,
    /// Trigger face to the butt's back (long guns).
    pub length_of_pull: Option<f32>,
    /// Overall height without a magazine (pistols: the whole gun).
    pub height: Option<f32>,
    /// Across the widest point (pistols).
    pub width: Option<f32>,
    /// Unloaded, with the magazine empty, as the manuals give it.
    pub mass: f32,
}

pub const SPECS: &[Spec] = &[
    Spec {
        id: "pm_pistol",
        travel: 0.03,
        hinge: [0.0; 3],
        // 9x18 PM: the Russian manual's figures as Wikipedia (ru, en)
        // carry them: 161 x 126.75 x 29.4 mm, 93.5 mm barrel, 130 mm
        // sight line, 730 g with the magazine empty.
        real: Some(Real {
            length: 161.0,
            barrel: 93.5,
            sight_radius: 130.0,
            length_of_pull: None,
            height: Some(126.75),
            width: Some(29.4),
            mass: 0.73,
        }),
    },
    Spec {
        id: "ak74",
        travel: 0.11,
        hinge: [0.0; 3],
        // 5.45 mm AK-74, fixed stock: 943 mm with the brake, 415 mm
        // barrel (Wikipedia, the Small Arms Survey sheet says 945); 379 mm
        // sight line (the Russian manual's, unconfirmed here); 3.3 kg
        // with the magazine empty. Length of pull: about 13.5 in.
        real: Some(Real {
            length: 943.0,
            barrel: 415.0,
            sight_radius: 379.0,
            length_of_pull: Some(345.0),
            height: None,
            width: None,
            mass: 3.3,
        }),
    },
    Spec {
        id: "mosin",
        travel: 0.09,
        hinge: [0.0; 3],
        real: None,
    },
    Spec {
        id: "sks",
        travel: 0.1,
        hinge: [0.0; 3],
        real: None,
    },
    Spec {
        id: "nagant",
        travel: 0.0,
        hinge: [0.0; 3],
        real: None,
    },
    Spec {
        id: "ppsh",
        travel: 0.1,
        hinge: [0.0; 3],
        real: None,
    },
    Spec {
        id: "mp133",
        travel: 0.09,
        hinge: [0.0; 3],
        real: None,
    },
    Spec {
        id: "svd",
        travel: 0.13,
        hinge: [0.0; 3],
        real: None,
    },
    Spec {
        id: "toz8",
        travel: 0.075,
        hinge: [0.0; 3],
        real: None,
    },
    Spec {
        id: "rpk74",
        travel: 0.11,
        hinge: [0.0; 3],
        real: None,
    },
    Spec {
        id: "flare_pistol",
        travel: 0.0,
        hinge: [0.0, 0.032, -0.065],
        real: None,
    },
    Spec {
        id: "crossbow",
        travel: 0.26,
        hinge: [0.0; 3],
        real: None,
    },
    Spec {
        id: "toz_double",
        travel: 0.0,
        hinge: [0.0, 0.012, -0.11],
        real: None,
    },
];

/// The bench's spec for gun `id`.
pub fn spec(id: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.id == id)
}
