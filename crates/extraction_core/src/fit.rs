//! How each building meets the ground: a QA gauge over the settled
//! terrain. The game sits a building on the lowest ground under its rough
//! footprint, lifted by what its foundation allows (`Heightmap::footing`);
//! from there:
//!
//! - **through**: how far the ground inside the walls rises above that
//!   footing (pokes through a low floor);
//! - **exposed**: how far the ground just outside the walls drops below it,
//!   past what its foundation covers (the building floats);
//! - **banked**: how far the ground just outside rises above it (earth
//!   heaped against a wall);
//! - **wet**: whether water stands inside the walls;
//! - **road**: how much of the floor a road's surface covers (roads drive
//!   into halls and barns; nowhere else).
//!
//! `cargo test -p extraction_core fit -- --nocapture` prints every
//! placement; the test holds them inside the limits below.

use crate::map::{MapDef, prefab_footprint, prefab_wants_pad};
use crate::terrain::Heightmap;

#[derive(Debug, Clone)]
pub struct Fit {
    pub prefab: String,
    pub at: [f32; 2],
    pub through: f32,
    pub exposed: f32,
    pub banked: f32,
    pub wet: bool,
    /// Share of the floor a road's surface covers.
    pub road: f32,
}

/// Points round the footprint's rim `out` metres beyond the walls, and a
/// grid inside it, in world space.
fn samples(at: [f32; 2], rot_deg: f32, w: f32, d: f32, out: f32, inside: bool) -> Vec<[f32; 2]> {
    let (s, c) = rot_deg.to_radians().sin_cos();
    let world = |lx: f32, lz: f32| [at[0] + lx * c - lz * s, at[1] + lx * s + lz * c];
    let mut pts = Vec::new();
    let step = 0.5;
    if inside {
        let (hw, hd) = (w * 0.5 - 0.3, d * 0.5 - 0.3);
        let (nx, nz) = ((2.0 * hw / step) as i32, (2.0 * hd / step) as i32);
        for i in 0..=nx {
            for j in 0..=nz {
                let (lx, lz) = (-hw + i as f32 * step, -hd + j as f32 * step);
                pts.push(world(lx, lz));
            }
        }
    } else {
        let (hw, hd) = (w * 0.5 + out, d * 0.5 + out);
        let (nx, nz) = ((2.0 * hw / step) as i32, (2.0 * hd / step) as i32);
        for i in 0..=nx {
            let lx = -hw + i as f32 * step;
            pts.push(world(lx, -hd));
            pts.push(world(lx, hd));
        }
        for j in 0..=nz {
            let lz = -hd + j as f32 * step;
            pts.push(world(-hw, lz));
            pts.push(world(hw, lz));
        }
    }
    pts
}

/// The fit of every padded placement on a map.
pub fn fits(map: &MapDef, hm: &Heightmap) -> Vec<Fit> {
    map.placements
        .iter()
        .filter(|p| !p.absolute && prefab_wants_pad(&p.prefab))
        .map(|p| {
            let fp = prefab_footprint(&p.prefab);
            let y = hm.footing(&p.prefab, p.at, p.rot);
            let inner = samples(p.at, p.rot, fp[0], fp[2], 0.0, true);
            let rim = samples(p.at, p.rot, fp[0], fp[2], 0.5, false);
            let through = inner.iter().map(|q| hm.height(*q) - y).fold(0.0, f32::max);
            let cover = crate::map::prefab_foundation(&p.prefab) - 0.3;
            let exposed = rim
                .iter()
                .map(|q| y - hm.height(*q) - cover.max(0.0))
                .fold(0.0, f32::max);
            let banked = rim.iter().map(|q| hm.height(*q) - y).fold(0.0, f32::max);
            let wet = inner.iter().any(|q| hm.water_level(*q).is_some());
            let road = inner
                .iter()
                .filter(|q| hm.road_at(**q).weight > 128)
                .count() as f32
                / inner.len().max(1) as f32;
            Fit {
                prefab: p.prefab.clone(),
                at: p.at,
                through,
                exposed,
                banked,
                wet,
                road,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ground may rise this far inside the walls (floors stand at least
    /// this high on their plinths).
    const THROUGH: f32 = 0.1;
    /// Ground below what the foundation covers, and earth against a wall.
    const EXPOSED: f32 = 0.1;
    const BANKED: f32 = 0.6;
    /// Prefabs a road may run into.
    const DRIVE_IN: &[&str] = &["bus_depot", "rail_shed", "factory_hall", "sawmill", "barn"];

    #[test]
    fn buildings_meet_the_ground() {
        let mut bad = Vec::new();
        for (name, text) in [
            ("marrow_creek", crate::map::MARROW_CREEK),
            ("kettle_pass", crate::map::KETTLE_PASS),
            ("cinder_works", crate::map::CINDER_WORKS),
        ] {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            for f in fits(&map, &hm) {
                let flag = f.through > THROUGH
                    || f.exposed > EXPOSED
                    || f.banked > BANKED
                    || f.wet
                    || (f.road > 0.05 && !DRIVE_IN.contains(&f.prefab.as_str()));
                println!(
                    "{name:13} {:15} at ({:5.0},{:5.0}): through {:4.2} exposed {:4.2} banked {:4.2}{}{}{}",
                    f.prefab,
                    f.at[0],
                    f.at[1],
                    f.through,
                    f.exposed,
                    f.banked,
                    if f.wet { " WET" } else { "" },
                    if f.road > 0.0 {
                        format!(" road {:.0}%", f.road * 100.0)
                    } else {
                        String::new()
                    },
                    if flag { "  <--" } else { "" }
                );
                if flag {
                    bad.push(format!("{name} {} at {:?}", f.prefab, f.at));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "buildings that don't meet the ground: {bad:#?}"
        );
    }
}

/// A boxcar against the rails it should stand on: how far its centre is
/// off the nearest rail line, how far its heading is off the line's, and
/// where on the line (and at what heading) it would stand.
#[derive(Debug, Clone)]
pub struct OnRails {
    pub at: [f32; 2],
    pub off: f32,
    pub turn_deg: f32,
    pub snap_at: [f32; 2],
    pub snap_rot: f32,
}

/// Every boxcar on a map, against its nearest rail line.
pub fn on_rails(map: &MapDef, hm: &Heightmap) -> Vec<OnRails> {
    let rails: Vec<&crate::terrain::RoadLine> = hm
        .road_lines
        .iter()
        .filter(|l| l.kind == crate::map::RoadKind::Rail)
        .collect();
    map.placements
        .iter()
        .filter(|p| p.prefab == "boxcar")
        .filter_map(|p| {
            let (line, (off, s, _)) = rails
                .iter()
                .map(|l| (l, crate::geom::polyline_closest(&l.points, p.at)))
                .min_by(|a, b| a.1.0.total_cmp(&b.1.0))?;
            let (q, t) = crate::geom::polyline_at(&line.points, s);
            // The car's length runs along its local Z, which the game's
            // yaw (-rot about Y) turns to (-sin rot, cos rot).
            let snap_rot = (-t[0]).atan2(t[1]).to_degrees();
            let along = [-(p.rot.to_radians().sin()), p.rot.to_radians().cos()];
            let cos = (along[0] * t[0] + along[1] * t[1]).abs().min(1.0);
            Some(OnRails {
                at: p.at,
                off,
                turn_deg: cos.acos().to_degrees(),
                snap_at: q,
                snap_rot,
            })
        })
        .collect()
}

#[cfg(test)]
mod rail_tests {
    use super::*;

    #[test]
    fn boxcars_stand_on_rails() {
        let mut bad = Vec::new();
        for (name, text) in [
            ("marrow_creek", crate::map::MARROW_CREEK),
            ("kettle_pass", crate::map::KETTLE_PASS),
            ("cinder_works", crate::map::CINDER_WORKS),
        ] {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            for r in on_rails(&map, &hm) {
                // Far from any line it's an abandoned car, not one on a
                // siding.
                let ok = r.off > 30.0 || (r.off < 0.3 && r.turn_deg < 2.0);
                println!(
                    "{name:13} boxcar at ({:6.1},{:6.1}): {:5.2} m off the rails, {:4.1} deg askew; on them: at ({:.1}, {:.1}), rot {:.1}{}",
                    r.at[0],
                    r.at[1],
                    r.off,
                    r.turn_deg,
                    r.snap_at[0],
                    r.snap_at[1],
                    r.snap_rot,
                    if ok { "" } else { "  <--" }
                );
                if !ok {
                    bad.push(format!("{name} boxcar at {:?}", r.at));
                }
            }
        }
        assert!(bad.is_empty(), "boxcars off their rails: {bad:#?}");
    }
}
