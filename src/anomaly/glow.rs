//! Night glow: at night a burner's coals light the ground round it, and a
//! charging snare's crackle does a little. It's for you and against you:
//! you see where it is, and a machine sees you further when you stand in
//! its light. The light model is here (which anomalies light what, how
//! much, from their kind and charge: `extraction_core::anomaly::night_glow`),
//! and machines' sight reads it through [`AnomalyLight::lit`] (Robot
//! minds'). It's a table read, no render: no new lights, nothing in your
//! own view.

use bevy::prelude::*;
use extraction_core::anomaly::night_glow;

use super::Anomaly;

/// This tick's anomaly light: each glowing anomaly's (centre, reach,
/// strength), its strength already scaled by how dark it is.
#[derive(Resource, Debug, Default, Clone)]
pub struct AnomalyLight {
    pub lamps: Vec<(Vec3, f32, f32)>,
}

impl AnomalyLight {
    /// How lit `p` is by anomalies, 0..1 (0 by day, and out of their reach):
    /// what machines' sight multiplies how far it sees someone standing
    /// there by.
    pub fn lit(&self, p: Vec3) -> f32 {
        self.lamps
            .iter()
            .map(|(c, reach, s)| {
                let d = Vec2::new(p.x - c.x, p.z - c.z).length();
                s * (1.0 - (d / reach).clamp(0.0, 1.0)).powi(2)
            })
            .fold(0.0, f32::max)
    }
}

/// Gathers the glowing anomalies, scaled by the dark (`Conditions::light`).
pub fn gather(
    anomalies: Query<(&Anomaly, &GlobalTransform), Without<super::surge::Forming>>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    mut light: ResMut<AnomalyLight>,
) {
    let dark = 1.0 - conditions.as_ref().map_or(1.0, |c| c.light).clamp(0.0, 1.0);
    light.lamps.clear();
    if dark < 0.05 {
        return;
    }
    for (a, g) in &anomalies {
        if let Some((reach, s)) = night_glow(a.kind, a.charge) {
            light
                .lamps
                .push((g.translation(), a.radius * reach, s * dark));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burner_lights_the_ground_round_it_by_night_only() {
        let lamp = |dark: f32| AnomalyLight {
            lamps: vec![(Vec3::ZERO, 2.0 * 2.5, 1.0 * dark)],
        };
        let night = lamp(1.0);
        assert!(
            night.lit(Vec3::new(2.5, 0.0, 0.0)) > 0.2,
            "lit by it at night"
        );
        assert!(
            night.lit(Vec3::new(1.0, 0.0, 0.0)) > night.lit(Vec3::new(4.0, 0.0, 0.0)),
            "brighter nearer"
        );
        assert_eq!(
            night.lit(Vec3::new(8.0, 0.0, 0.0)),
            0.0,
            "not past its reach"
        );
        assert_eq!(
            AnomalyLight::default().lit(Vec3::ZERO),
            0.0,
            "by day, nothing"
        );
        assert!(
            night_glow(extraction_core::anomaly::AnomalyKind::Hush, 1.0).is_none(),
            "a hush gives no light"
        );
    }

    /// In the game: at night the gathered light has the burner in it; by day
    /// it's empty.
    #[test]
    fn the_light_is_gathered_at_night() {
        for (hour, lit) in [("night", true), ("day", false)] {
            let mut app = crate::headless_app(
                crate::launch::LaunchOptions {
                    robots: false,
                    hour: Some(hour.into()),
                    ..Default::default()
                },
                false,
            );
            app.finish();
            app.cleanup();
            app.update();
            let zone = app.world().resource::<crate::world::Zone>().clone();
            let at = zone.on_ground(0.0, 30.0);
            super::super::spawn_anomaly(
                app.world_mut(),
                extraction_core::anomaly::AnomalyKind::Burner,
                at,
                2.0,
                None,
            );
            app.update();
            app.update();
            let l = app
                .world()
                .resource::<AnomalyLight>()
                .lit(at + Vec3::X * 2.0);
            assert_eq!(l > 0.1, lit, "{hour}: lit {l:.2}");
        }
    }
}
