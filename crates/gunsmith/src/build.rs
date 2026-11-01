//! What a gun has been made into at the vice (the rules of cutting and
//! swapping live with the game's gunsmithing, `extraction_core::gunsmith`).

use serde::{Deserialize, Serialize};

/// What a gun has been made into. The default is as it left the factory.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Build {
    /// Its barrel cut to this (mm); `None` as made.
    pub barrel_mm: Option<f32>,
    /// Its butt stock on.
    pub stock: bool,
    /// Its bolt out of the receiver: it neither cycles nor fires.
    pub bolt_out: bool,
    /// Its bolt's condition, against the gun's.
    pub bolt_dev: f32,
    /// Its barrel's condition, against the gun's.
    pub barrel_dev: f32,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            barrel_mm: None,
            stock: true,
            bolt_out: false,
            bolt_dev: 0.0,
            barrel_dev: 0.0,
        }
    }
}
