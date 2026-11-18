//! Steam: where a burner's heat meets a frost's cold, a bank of steam
//! stands between them. It hides what's in it and behind it, from you and
//! from machines (their sight reads [`Steams::blocks`]): something to
//! cross unseen, or something a hound waits in.
//!
//! A pair is a burner and a frost whose reaches come within
//! [`STEAM_GAP`] of each other; the bank sits in the gap between their
//! rims. Pairs are rare, by placement (Map design's).

use bevy::prelude::*;
use extraction_core::anomaly::AnomalyKind;

use super::Anomaly;

/// How near a burner's and a frost's rims must come for steam, m.
pub const STEAM_GAP: f32 = 4.0;
/// The bank's radius, m, and how high its heart stands over the ground.
pub const STEAM_RADIUS: f32 = 3.0;
pub const STEAM_HEIGHT: f32 = 1.3;
/// A line of sight must cross this much of a bank to be lost in it, m.
pub const STEAM_THICK: f32 = 1.2;

/// This tick's steam banks: (centre, radius).
#[derive(Resource, Debug, Default, Clone)]
pub struct Steams {
    pub banks: Vec<(Vec3, f32)>,
}

impl Steams {
    /// Whether a line of sight from `a` to `b` is lost in steam: it
    /// passes through a bank for at least `STEAM_THICK` (an eye inside one
    /// sees nothing past a little way either).
    pub fn blocks(&self, a: Vec3, b: Vec3) -> bool {
        self.banks
            .iter()
            .any(|(c, r)| chord(a, b, *c, *r) >= STEAM_THICK)
    }
}

/// How much of the segment `a`-`b` lies inside the sphere (`c`, `r`), m.
fn chord(a: Vec3, b: Vec3, c: Vec3, r: f32) -> f32 {
    let d = b - a;
    let len = d.length();
    if len < 1e-4 {
        return 0.0;
    }
    let u = d / len;
    let t = (c - a).dot(u);
    let off = (a + u * t - c).length_squared();
    if off >= r * r {
        return 0.0;
    }
    let half = (r * r - off).sqrt();
    ((t + half).min(len) - (t - half).max(0.0)).max(0.0)
}

/// A steam bank in the world (for its look): where its burner and frost
/// meet.
#[derive(Component, Debug, Clone, Copy)]
pub struct SteamBank {
    pub burner: Entity,
    pub frost: Entity,
    pub radius: f32,
}

/// Where a burner (`b`, radius `rb`) and a frost (`f`, `rf`) make steam,
/// if they're near enough: the middle of the gap between their rims.
pub fn bank_between(b: Vec3, rb: f32, f: Vec3, rf: f32) -> Option<Vec3> {
    let flat = Vec3::new(f.x - b.x, 0.0, f.z - b.z);
    let d = flat.length();
    if d - rb - rf > STEAM_GAP || d < 1e-3 {
        return None;
    }
    let u = flat / d;
    let from = b + u * rb;
    let to = f - u * rf;
    Some((from + to) * 0.5)
}

/// Finds the pairs each tick (anomalies creep, bloom and fade) and keeps
/// a bank entity for each.
pub fn pair_up(
    mut commands: Commands,
    anomalies: Query<(Entity, &Anomaly, &GlobalTransform), Without<super::surge::Forming>>,
    mut banks: Query<(Entity, &SteamBank, &mut Transform)>,
    mut steams: ResMut<Steams>,
    zone: Res<crate::world::Zone>,
) {
    steams.banks.clear();
    let burners: Vec<_> = anomalies
        .iter()
        .filter(|(_, a, _)| a.kind == AnomalyKind::Burner)
        .map(|(e, a, g)| (e, g.translation(), a.radius))
        .collect();
    let frosts: Vec<_> = anomalies
        .iter()
        .filter(|(_, a, _)| a.kind == AnomalyKind::Frost)
        .map(|(e, a, g)| (e, g.translation(), a.radius))
        .collect();
    let mut live = Vec::new();
    for (be, b, rb) in &burners {
        for (fe, f, rf) in &frosts {
            let Some(mid) = bank_between(*b, *rb, *f, *rf) else {
                continue;
            };
            let at = zone.on_ground(mid.x, mid.z) + Vec3::Y * STEAM_HEIGHT;
            steams.banks.push((at, STEAM_RADIUS));
            live.push((*be, *fe, at));
        }
    }
    // Keep, move or drop the bank entities to match.
    let mut kept = Vec::new();
    for (e, bank, mut t) in &mut banks {
        match live
            .iter()
            .find(|(b, f, _)| *b == bank.burner && *f == bank.frost)
        {
            Some((.., at)) => {
                t.translation = *at - Vec3::Y * STEAM_HEIGHT;
                kept.push((bank.burner, bank.frost));
            }
            None => commands.entity(e).despawn(),
        }
    }
    for (b, f, at) in live {
        if !kept.contains(&(b, f)) {
            commands.spawn((
                Name::new("Steam bank"),
                SteamBank {
                    burner: b,
                    frost: f,
                    radius: STEAM_RADIUS,
                },
                Transform::from_translation(at - Vec3::Y * STEAM_HEIGHT),
                Visibility::default(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burner_and_a_frost_near_each_other_steam_between() {
        let b = Vec3::ZERO;
        let f = Vec3::new(2.0 + 3.0 + 2.0, 0.0, 0.0);
        let at = bank_between(b, 2.0, f, 3.0).expect("steam");
        assert!((at.x - 3.0).abs() < 1e-4, "in the gap's middle: {at}");
        assert!(
            bank_between(b, 2.0, Vec3::new(20.0, 0.0, 0.0), 3.0).is_none(),
            "too far apart"
        );
    }

    #[test]
    fn steam_hides_what_is_behind_it_not_beside_it() {
        let s = Steams {
            banks: vec![(Vec3::new(0.0, 1.3, 0.0), STEAM_RADIUS)],
        };
        let eye = Vec3::new(-10.0, 1.6, 0.0);
        assert!(s.blocks(eye, Vec3::new(10.0, 1.0, 0.0)), "through it");
        assert!(s.blocks(eye, Vec3::new(0.5, 1.0, 0.0)), "into it");
        assert!(!s.blocks(eye, Vec3::new(10.0, 1.0, 8.0)), "past it");
        assert!(!s.blocks(eye, Vec3::new(-3.5, 1.0, 0.0)), "short of it");
    }

    /// In the game: a burner and a frost put down a few metres apart make
    /// a bank, and it goes when they part.
    #[test]
    fn a_pair_in_the_world_makes_a_bank() {
        let mut app = crate::headless_app(Default::default(), false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let b = zone.on_ground(0.0, 30.0);
        let f = zone.on_ground(8.0, 30.0);
        let be = super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Burner, b, 2.0, None);
        let fe = super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Frost, f, 3.0, None);
        for _ in 0..3 {
            app.update();
        }
        let n = |app: &mut App| {
            app.world_mut()
                .query::<&SteamBank>()
                .iter(app.world())
                .filter(|s| s.burner == be && s.frost == fe)
                .count()
        };
        assert_eq!(n(&mut app), 1, "a bank between them");
        app.world_mut().entity_mut(fe).despawn();
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(n(&mut app), 0, "gone with the frost");
    }
}
