//! How far a valley's ground lies from its Gate, where every heavy comes
//! from (`robots::heavy_mind`: the director calls one out of the
//! `RobotGate`, its horn sounds there, and twenty seconds later it walks
//! out to wherever you are). The horn has to reach anywhere you can be;
//! the game's gauge `every_heavy_reads_before_it_reaches_you` holds the
//! horn's heard range against [`reach`].

use crate::geom::P2;
use crate::map::{MapDef, MarkerKind};
use crate::terrain::Heightmap;

/// The grid the ground is sampled on (m).
pub const STEP: f32 = 10.0;

/// A valley's ground as seen from its Gate.
#[derive(Clone, Debug)]
pub struct Reach {
    /// The Gate (the first `RobotGate`).
    pub gate: P2,
    /// Every walkable, dry spot inside the fence, on a [`STEP`] grid, and
    /// its distance from the Gate (m, on the level).
    pub ground: Vec<(P2, f32)>,
}

impl Reach {
    /// The spots further than `heard` from the Gate, furthest first.
    pub fn beyond(&self, heard: f32) -> Vec<(P2, f32)> {
        let mut out: Vec<_> = self
            .ground
            .iter()
            .filter(|g| g.1 > heard)
            .copied()
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }

    /// The furthest spot and its distance.
    pub fn furthest(&self) -> Option<(P2, f32)> {
        self.ground
            .iter()
            .copied()
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }
}

/// The ground of `map` against its Gate, or `None` with no Gate.
pub fn reach(map: &MapDef, hm: &Heightmap) -> Option<Reach> {
    let gate = map.markers_of(&MarkerKind::RobotGate).next()?.at;
    let walk = crate::map_nav::zone_grid(map, hm, crate::map_nav::MAX_SLOPE);
    let lo = walk.origin;
    let n = ((walk.cell * walk.n as f32) / STEP).floor() as usize;
    let mut ground = Vec::new();
    for i in 0..n {
        for j in 0..n {
            let p = [lo + i as f32 * STEP, lo + j as f32 * STEP];
            if walk.walkable(p)
                && hm.water_level(p).is_none()
                && crate::map_lint::inside_fence(map, p)
            {
                ground.push((p, (p[0] - gate[0]).hypot(p[1] - gate[1])));
            }
        }
    }
    Some(Reach { gate, ground })
}

/// The named marker nearest `p`, and how far (for saying where a spot is).
pub fn near(map: &MapDef, p: P2) -> Option<(&str, f32)> {
    map.markers
        .iter()
        .filter(|m| !m.name.is_empty())
        .map(|m| (m.name.as_str(), (m.at[0] - p[0]).hypot(m.at[1] - p[1])))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_valley_has_ground_round_its_gate() {
        for (id, text) in crate::map::VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let r = reach(&map, &hm).unwrap_or_else(|| panic!("{id}: no RobotGate"));
            assert!(r.ground.len() > 500, "{id}: {} spots", r.ground.len());
            // (The insertion is ground you stand on.)
            let ins = map.markers_of(&MarkerKind::Insertion).next().unwrap().at;
            let far = r.furthest().unwrap().1;
            let d = (ins[0] - r.gate[0]).hypot(ins[1] - r.gate[1]);
            assert!(
                d < far + STEP,
                "{id}: insertion {d:.0} m out, ground to {far:.0}"
            );
        }
    }
}
