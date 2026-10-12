//! Anomalies: where they lie each Surge cycle and what they leave behind.
//!
//! Every cycle the Surge rearranges the Zone's anomalies. Most cluster in
//! the map's artifact fields; a few lie loose elsewhere, off the roads and
//! out of the water. The layout is a pure function of the map, the seed and
//! the cycle, so a saved game rebuilds the same anomalies without saving
//! them, and the director can add temporary "blooms" on top.

use crate::geom;
use crate::map::{MapDef, MarkerKind};
use crate::noise::Rng;
use crate::terrain::Heightmap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnomalyKind {
    /// Gravity runs the wrong way: things float, swirl, and get flung.
    Lift,
    /// Heat with no fire: burns whatever stands in it.
    Burner,
    /// A static snare: discharges into whatever touches it, then recharges.
    Snare,
}

impl AnomalyKind {
    pub const ALL: [AnomalyKind; 3] = [AnomalyKind::Lift, AnomalyKind::Burner, AnomalyKind::Snare];

    /// The artifact this kind grows, by item id.
    pub fn artifact(self) -> &'static str {
        match self {
            AnomalyKind::Lift => "art_featherstone",
            AnomalyKind::Burner => "art_hearth",
            AnomalyKind::Snare => "art_aegis",
        }
    }

    /// (min, max) radius in metres.
    pub fn radius_range(self) -> (f32, f32) {
        match self {
            AnomalyKind::Lift => (2.5, 4.0),
            AnomalyKind::Burner => (1.4, 2.4),
            AnomalyKind::Snare => (1.8, 3.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnomalySpec {
    pub kind: AnomalyKind,
    /// Ground position (x, z).
    pub at: [f32; 2],
    pub radius: f32,
    /// An artifact has grown at its heart this cycle.
    pub artifact: Option<&'static str>,
}

/// Chance that an anomaly in a field holds an artifact this cycle.
pub const FIELD_ARTIFACT_CHANCE: f32 = 0.35;
/// Chance for a loose anomaly elsewhere.
pub const LOOSE_ARTIFACT_CHANCE: f32 = 0.12;
/// The rarest find: a glass eye, in place of the usual artifact.
pub const GLASS_EYE_CHANCE: f32 = 0.1;

/// Somewhere an anomaly can sit: dry, off-road, not too steep.
pub fn placeable(map: &MapDef, hm: &Heightmap, p: [f32; 2]) -> bool {
    let half = map.size * 0.5 - map.fence_inset - 4.0;
    if p[0].abs() > half || p[1].abs() > half {
        return false;
    }
    if hm.water_level(p).is_some() || hm.road_at(p).kind.is_some() {
        return false;
    }
    if hm.slope_deg(p) > 24.0 {
        return false;
    }
    // Not inside a building.
    !map.placements.iter().any(|pl| {
        let f = crate::map::prefab_footprint(&pl.prefab);
        geom::dist(pl.at, p) < f[0].max(f[2]) * 0.6 + 2.0
    })
}

/// The anomalies of a Surge cycle.
pub fn layout(map: &MapDef, hm: &Heightmap, seed: u64, cycle: u32) -> Vec<AnomalySpec> {
    let mut rng = Rng::new(seed ^ 0xA70A ^ (cycle as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mut out: Vec<AnomalySpec> = Vec::new();
    let fits = |out: &[AnomalySpec], p: [f32; 2], r: f32| {
        out.iter().all(|a| geom::dist(a.at, p) > a.radius + r + 2.5)
    };
    let mut place =
        |rng: &mut Rng, out: &mut Vec<AnomalySpec>, centre: [f32; 2], spread: f32, chance: f32| {
            for _ in 0..30 {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let d = spread * rng.f32().sqrt();
                let p = [centre[0] + a.cos() * d, centre[1] + a.sin() * d];
                let kind = AnomalyKind::ALL[rng.below(3)];
                let (lo, hi) = kind.radius_range();
                let radius = rng.range(lo, hi);
                if !placeable(map, hm, p) || !fits(out, p, radius) {
                    continue;
                }
                let artifact = rng.chance(chance).then(|| {
                    if rng.chance(GLASS_EYE_CHANCE) {
                        "art_glasseye"
                    } else {
                        kind.artifact()
                    }
                });
                out.push(AnomalySpec {
                    kind,
                    at: p,
                    radius,
                    artifact,
                });
                return;
            }
        };
    for m in map.markers_of(&MarkerKind::ArtifactField) {
        let n = 5 + rng.below(4);
        for _ in 0..n {
            place(&mut rng, &mut out, m.at, 28.0, FIELD_ARTIFACT_CHANCE);
        }
    }
    let half = map.size * 0.5 - map.fence_inset - 10.0;
    for _ in 0..(4 + rng.below(3)) {
        let c = [rng.range(-half, half), rng.range(-half, half)];
        place(&mut rng, &mut out, c, 0.0, LOOSE_ARTIFACT_CHANCE);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::MARROW_CREEK;

    fn marrow() -> (MapDef, Heightmap) {
        let map = MapDef::from_ron(MARROW_CREEK).unwrap();
        let hm = Heightmap::build(&map);
        (map, hm)
    }

    #[test]
    fn layout_is_deterministic_and_changes_with_the_cycle() {
        let (map, hm) = marrow();
        let a = layout(&map, &hm, 1, 0);
        let b = layout(&map, &hm, 1, 0);
        let c = layout(&map, &hm, 1, 1);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(
            a.len() >= 10,
            "fields are populated ({} anomalies)",
            a.len()
        );
    }

    #[test]
    fn anomalies_keep_off_roads_water_and_each_other() {
        let (map, hm) = marrow();
        for cycle in 0..5 {
            let all = layout(&map, &hm, 7, cycle);
            for (i, a) in all.iter().enumerate() {
                assert!(placeable(&map, &hm, a.at), "{a:?}");
                for b in &all[i + 1..] {
                    assert!(
                        geom::dist(a.at, b.at) > a.radius + b.radius,
                        "{a:?} overlaps {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn some_cycles_grow_artifacts() {
        let (map, hm) = marrow();
        let n: usize = (0..10)
            .map(|c| {
                layout(&map, &hm, 3, c)
                    .iter()
                    .filter(|a| a.artifact.is_some())
                    .count()
            })
            .sum();
        assert!(
            n >= 10,
            "about a handful of artifacts per cycle (got {n} over 10)"
        );
    }
}
#[cfg(test)]
mod print_layout {
    #[test]
    #[ignore]
    fn print() {
        let map = crate::map::MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let hm = crate::terrain::Heightmap::build(&map);
        for a in super::layout(&map, &hm, 1, 0) {
            eprintln!(
                "{:?} {:.1?} r{:.1} {:?} h{:.1}",
                a.kind,
                a.at,
                a.radius,
                a.artifact,
                hm.height(a.at)
            );
        }
    }
}
