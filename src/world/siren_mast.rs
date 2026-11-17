//! A siren mast (`siren_mast`, `extraction_core::siren_mast`): a painted
//! steel pole on a concrete plinth, a ladder up its back, a cage at the
//! top with the horn out over it and an amber lamp on the cap. Only the
//! pole and its plinth are solid.
//!
//! The lamp burns a steady amber, a hazard's, not a yard lamp's warm white
//! nor a landmark's red: [`SirenLamp`] marks its halo, for whoever sounds
//! the alarm to swell or flash.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use extraction_core::siren_mast::{HEIGHT, HORN, LAMP, PREFAB};

use super::buildings::{Builder, Building, Geo};
use super::halos::{Halo, HaloAngle, HaloReach};
use crate::render::materials::Mat;

pub struct SirenMastPlugin;

impl Plugin for SirenMastPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, halo_siren_lamps);
    }
}

/// The halo on a siren mast's lamp.
#[derive(Component)]
pub struct SirenLamp;

/// The lamp's amber (linear, as the tonemapper gets it), its halo's size,
/// and how far off it shows (m): across the bed from the gauge house.
pub const LAMP_AMBER: LinearRgba = LinearRgba::rgb(1.4, 0.45, 0.02);
const LAMP_R: f32 = 0.35;
const LAMP_REACH: f32 = 400.0;
const LAMP_ANGLE: f32 = 0.05;

fn v(p: [f32; 3]) -> Vec3 {
    Vec3::from_array(p)
}

pub fn geo(id: &str) -> Option<Geo> {
    if id != PREFAB {
        return None;
    }
    let mut b = Builder::default();
    let pole = 0.22;
    // The plinth, and the pole up to the cage's floor.
    b.bx(
        Vec3::new(0.9, 0.4, 0.9),
        Vec3::new(0.0, 0.2, 0.0),
        Mat::Concrete,
    );
    let top = HORN[1] - 0.8;
    b.bx(
        Vec3::new(pole, top, pole),
        Vec3::new(0.0, top * 0.5, 0.0),
        Mat::PaintedMetal,
    );
    // A ladder up its back (+Z), its rails and every third rung.
    for x in [-0.2f32, 0.2] {
        b.deco(
            Vec3::new(0.04, top - 2.4, 0.04),
            Vec3::new(x, 2.4 + (top - 2.4) * 0.5, 0.32),
            Mat::DarkMetal,
        );
    }
    let mut y = 2.6;
    while y < top {
        b.deco(
            Vec3::new(0.4, 0.03, 0.03),
            Vec3::new(0.0, y, 0.32),
            Mat::DarkMetal,
        );
        y += 0.9;
    }
    // The cage: a floor and a rail round it.
    b.deco(
        Vec3::new(1.2, 0.06, 1.2),
        Vec3::new(0.0, top, 0.0),
        Mat::DarkMetal,
    );
    for (size, at) in [
        (Vec3::new(1.2, 0.04, 0.04), Vec3::new(0.0, top + 1.0, 0.6)),
        (Vec3::new(1.2, 0.04, 0.04), Vec3::new(0.0, top + 1.0, -0.6)),
        (Vec3::new(0.04, 0.04, 1.2), Vec3::new(0.6, top + 1.0, 0.0)),
        (Vec3::new(0.04, 0.04, 1.2), Vec3::new(-0.6, top + 1.0, 0.0)),
    ] {
        b.deco(size, at, Mat::DarkMetal);
    }
    // The pole on through the cage to the cap, the horn's box on it, its
    // flare out to the front, and the lamp's hood on top.
    b.deco(
        Vec3::new(0.14, HEIGHT - top, 0.14),
        Vec3::new(0.0, top + (HEIGHT - top) * 0.5, 0.0),
        Mat::PaintedMetal,
    );
    let horn = v(HORN);
    b.deco(
        Vec3::new(0.5, 0.5, 0.5),
        Vec3::new(0.0, horn.y, -0.2),
        Mat::PaintedMetal,
    );
    b.deco(Vec3::new(0.8, 0.8, 0.12), horn, Mat::DarkMetal);
    b.deco(
        Vec3::new(0.3, 0.06, 0.3),
        v(LAMP) + Vec3::Y * 0.22,
        Mat::DarkMetal,
    );
    b.deco(Vec3::new(0.2, 0.3, 0.2), v(LAMP), Mat::Glass);
    Some(b.finish())
}

/// A siren mast's lamp: its halo, amber. No light: a lamp to see from
/// across the bed, not to light the yard.
fn halo_siren_lamps(mut commands: Commands, masts: Query<(Entity, &Building), Added<Building>>) {
    for (e, b) in &masts {
        if b.prefab != PREFAB {
            continue;
        }
        commands.entity(e).with_child((
            Name::new("Siren lamp"),
            SirenLamp,
            Transform::from_translation(v(LAMP)),
            Visibility::default(),
            NotShadowCaster,
            Halo {
                color: LAMP_AMBER,
                radius: LAMP_R,
            },
            HaloReach(LAMP_REACH),
            HaloAngle(LAMP_ANGLE),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The horn and the lamp are on the mast as built: the lamp inside its
    /// glass, the horn's mouth on the front of the mast, the whole as tall
    /// as the core says.
    #[test]
    fn the_horn_and_lamp_are_where_the_core_says() {
        let geo = geo(PREFAB).unwrap();
        let (lo, hi) = geo.pieces.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), p| match p.shape {
                crate::world::buildings::Shape::Box(s) => (
                    lo.min(p.at.translation - s * 0.5),
                    hi.max(p.at.translation + s * 0.5),
                ),
                _ => (lo, hi),
            },
        );
        assert!(
            (hi.y - HEIGHT).abs() < 0.05,
            "mast {:.2} m, not {HEIGHT}",
            hi.y
        );
        assert!(lo.y > -0.01);
        assert!(HORN[2] < 0.0 && LAMP[1] > HORN[1]);
    }
}
