//! Where Rime Fell's works make their sounds, for Audio to voice
//! (`docs/FIFTH_VALLEY.md`). The works are derelict: nothing runs, the
//! funicular's car never moves, so every emitter is the wind working on
//! something left behind. Worked out from the map (placements by prefab,
//! the funicular by its road, the galleries by their spans), so they follow
//! anything that moves.

use bevy::prelude::*;
use extraction_core::map::MapDef;
use extraction_core::terrain::Heightmap;

/// What's making the sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A loose sheet of corrugated iron on the mill's headhouse, banging in
    /// the wind.
    MillSheet,
    /// Wind through the mill's broken windows, one per bay.
    MillBay,
    /// The top station's bull wheel: the cut rope's end creaking on it.
    BullWheel,
    /// The funicular's rope on its rollers, ankle-high along the line.
    Rope,
    /// The car stopped at the passing loop: its doors swinging.
    Car,
    /// Meltwater dripping under a gallery's roof; wind along it.
    Gallery,
}

#[derive(Clone, Copy, Debug)]
pub struct Emitter {
    pub source: Source,
    /// World position, metres.
    pub at: Vec3,
}

/// Height of a point the way the game places things: absolute, or over
/// the ground.
fn placed(hm: &Heightmap, p: &extraction_core::map::Placement, local: Vec3) -> Vec3 {
    let r = Quat::from_rotation_y(-p.rot.to_radians());
    let base = Vec3::new(p.at[0], 0.0, p.at[1]);
    let w = base + r * Vec3::new(local.x, 0.0, local.z);
    let floor = if p.absolute {
        p.y
    } else {
        hm.height(p.at) + p.y
    };
    Vec3::new(w.x, floor + local.y, w.z)
}

/// Every emitter on a map (none on a map without these works).
pub fn emitters(map: &MapDef, hm: &Heightmap) -> Vec<Emitter> {
    let mut out = Vec::new();
    for p in &map.placements {
        match p.prefab.as_str() {
            "ore_mill" => {
                // The headhouse's west eave over the top bay.
                out.push(Emitter {
                    source: Source::MillSheet,
                    at: placed(hm, p, Vec3::new(9.5, 19.5, 15.0)),
                });
                for (k, f) in super::fell::MILL_FLOORS.iter().enumerate() {
                    let z = -15.0 + 10.0 * k as f32;
                    out.push(Emitter {
                        source: Source::MillBay,
                        at: placed(hm, p, Vec3::new(0.0, f + 2.5, z)),
                    });
                }
            }
            "funicular_upper" => out.push(Emitter {
                source: Source::BullWheel,
                at: placed(hm, p, Vec3::new(0.0, 8.0, 0.0)),
            }),
            "funicular_car" => out.push(Emitter {
                source: Source::Car,
                at: placed(hm, p, Vec3::new(0.0, 1.5, 0.0)),
            }),
            _ => {}
        }
    }
    // The rope, every 40 m along the funicular's line.
    if let Some(rl) = hm
        .road_lines
        .iter()
        .zip(&map.roads)
        .find(|(_, r)| r.name == super::roadworks::FUNICULAR)
        .map(|(rl, _)| rl)
    {
        let len: f32 = rl
            .points
            .windows(2)
            .map(|w| extraction_core::geom::dist(w[0], w[1]))
            .sum();
        let mut s = 20.0;
        while s < len - 10.0 {
            let (q, _) = extraction_core::geom::polyline_at(&rl.points, s);
            out.push(Emitter {
                source: Source::Rope,
                at: Vec3::new(q[0], hm.height(q) + 0.3, q[1]),
            });
            s += 40.0;
        }
    }
    // Each gallery, at its middle under the roof.
    for g in super::roadworks::gallery_plans(map, hm) {
        if let Some(st) = g.stations.get(g.stations.len() / 2) {
            out.push(Emitter {
                source: Source::Gallery,
                at: st.at + Vec3::Y * 3.0,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rime() -> (MapDef, Heightmap) {
        let map = MapDef::from_ron(extraction_core::map::RIME_FELL).unwrap();
        let hm = Heightmap::build(&map);
        (map, hm)
    }

    /// Rime Fell's works all speak: a sheet and four bays at the mill, the
    /// bull wheel, the car, the rope down the line, both galleries; each
    /// emitter above the ground and on the map.
    #[test]
    fn rime_fells_works_have_their_sounds() {
        let (map, hm) = rime();
        let e = emitters(&map, &hm);
        let n = |s: Source| e.iter().filter(|x| x.source == s).count();
        assert_eq!(n(Source::MillSheet), 1);
        assert_eq!(n(Source::MillBay), 4);
        assert_eq!(n(Source::BullWheel), 1);
        assert_eq!(n(Source::Car), 1);
        assert!(n(Source::Rope) >= 4, "{}", n(Source::Rope));
        assert_eq!(n(Source::Gallery), 2);
        for x in &e {
            let ground = hm.height([x.at.x, x.at.z]);
            assert!(x.at.y > ground - 0.01, "{x:?} under the ground ({ground})");
            assert!(x.at.x.abs() < 320.0 && x.at.z.abs() < 320.0, "{x:?}");
        }
        // The sheet is up on the headhouse, well over the mill's front.
        let sheet = e.iter().find(|x| x.source == Source::MillSheet).unwrap();
        assert!(sheet.at.y > 105.0, "{sheet:?}");
    }

    /// The other valleys have none of these works.
    #[test]
    fn other_valleys_are_silent_here() {
        for (id, text) in extraction_core::map::VALLEYS {
            if id == "rime_fell" {
                continue;
            }
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            assert!(emitters(&map, &hm).is_empty(), "{id}");
        }
    }
}
