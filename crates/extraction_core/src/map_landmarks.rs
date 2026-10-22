//! Landmarks as a player sees them: whole structures, not placements.
//!
//! A landmark is a structure at least `map_lint::LANDMARK_HEIGHT` tall.
//! What you steer by is its top, so sight lines go to the top of each of
//! its tall parts ([`tall_parts`]): a 265 m dam face is seen along its
//! whole parapet (sampled every [`TOP_STEP`]) and from its turrets, a
//! chapel by its bell tower at the east end, a blast furnace by its stack
//! and its stoves' chimney. A part hidden by a hill doesn't hide the rest.
//! Tops are where the game puts them: on the settled ground (`footing`)
//! plus the placement's `y`, or at `y` for an `absolute` one (the dam's
//! faces stand on the valley floor and the bed, not on the crest).
//!
//! Pieces of one kind within [`JOIN`] of each other are one structure (the
//! dam's four faces are the dam), and a kind is what a thing looks like
//! from afar ([`kind`]): a chapel's tower is a steeple, a drowned chapel is
//! a chapel.

use crate::geom::{self, P2, dist};
use crate::map::{MapDef, Placement, prefab_base, prefab_footprint};
use crate::map_lint::{LANDMARK_HEIGHT, sees};
use crate::terrain::Heightmap;

/// A long wall's top is sampled at most this far apart, metres.
pub const TOP_STEP: f32 = 20.0;
/// Pieces of one kind whose footprints are closer than this are one
/// structure, metres.
pub const JOIN: f32 = 30.0;

/// What a landmark looks like from afar: two of one kind in sight and you
/// can't tell which is which.
pub fn kind(prefab: &str) -> &str {
    match prefab_base(prefab) {
        "dam_face" | "dam_face_end" | "dam_back" | "dam_back_end" => "dam",
        // The chapel's bell tower is the steeple, built and dressed as one.
        "chapel" | "steeple" => "steeple",
        b => b,
    }
}

/// The tops of a prefab's tall parts, in its own space: `(x, z, height
/// over its base)`. Measured from the game's recipes
/// (`src/world/buildings.rs`, `src/world/dam.rs`); anything not listed is
/// its footprint's height at its middle.
pub fn tall_parts(prefab: &str) -> Vec<[f32; 3]> {
    let [w, h, _] = prefab_footprint(prefab);
    // A wall the length of the footprint, its top sampled along x.
    let wall = |len: f32, h: f32| -> Vec<[f32; 3]> {
        let n = (len / TOP_STEP).ceil().max(1.0) as usize;
        (0..=n)
            .map(|i| [-len * 0.5 + len * i as f32 / n as f32, 0.0, h])
            .collect()
    };
    match prefab_base(prefab) {
        // The downstream face: its parapet along the crest, and a turret
        // 3 m in from each end to 28.6 m (dam.rs `face`).
        "dam_face" => {
            let mut v = wall(w, h);
            v.extend([[-w * 0.5 + 3.0, -0.5, 28.6], [w * 0.5 - 3.0, -0.5, 28.6]]);
            v
        }
        "dam_face_end" | "dam_back" | "dam_back_end" => wall(w, h),
        // The bell tower off the nave's east end (buildings.rs `chapel`).
        "chapel" => vec![[0.0, 9.175, h]],
        // The tower at the race's head (MR_SHIFT).
        "mill_race" => vec![[0.0, -19.3, h]],
        // The stack's bleeders at the furnace (BF_SHIFT) and the stoves'
        // chimney (BF_CHIMNEY).
        "blast_furnace" => vec![[-5.7, -10.6, 33.0], [12.8, 0.9, h]],
        _ => vec![[0.0, 0.0, h]],
    }
}

/// Where the tops of a placement's tall parts are: map position and
/// height above sea level.
pub fn tops(hm: &Heightmap, p: &Placement) -> Vec<(P2, f32)> {
    let base = if p.absolute {
        p.y
    } else {
        hm.footing(&p.prefab, p.at, p.rot) + p.y
    };
    let (s, c) = p.rot.to_radians().sin_cos();
    tall_parts(&p.prefab)
        .into_iter()
        .map(|[x, z, h]| ([p.at[0] + x * c - z * s, p.at[1] + x * s + z * c], base + h))
        .collect()
}

/// A placement's footprint corners, its middle and its tops: the points
/// that say which region it stands in.
fn spots(hm: &Heightmap, p: &Placement) -> Vec<P2> {
    let [w, _, d] = prefab_footprint(&p.prefab);
    let (s, c) = p.rot.to_radians().sin_cos();
    let mut v: Vec<P2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .map(|(a, b)| {
            let (x, z) = (a * w * 0.5, b * d * 0.5);
            [p.at[0] + x * c - z * s, p.at[1] + x * s + z * c]
        })
        .collect();
    v.push(p.at);
    v.extend(tops(hm, p).into_iter().map(|t| t.0));
    v
}

/// One structure to steer by.
#[derive(Clone, Debug)]
pub struct Landmark {
    /// What it looks like ([`kind`]).
    pub kind: String,
    /// Its prefab, or its kind when it's built of several.
    pub name: String,
    /// Its placements, by index into the map's.
    pub pieces: Vec<usize>,
    /// The tops of its tall parts: position and height above sea level.
    pub tops: Vec<(P2, f32)>,
}

impl Landmark {
    /// How far off its nearest top in sight from eye height at `from` is,
    /// within `range`; `None` if none is.
    pub fn seen(&self, hm: &Heightmap, from: P2, range: f32) -> Option<f32> {
        let mut near: Vec<(P2, f32, f32)> = self
            .tops
            .iter()
            .map(|(at, top)| (*at, *top, dist(from, *at)))
            .filter(|t| t.2 <= range)
            .collect();
        near.sort_by(|a, b| a.2.total_cmp(&b.2));
        near.into_iter()
            .find(|(at, top, _)| sees(hm, from, *at, top - hm.height(*at)))
            .map(|t| t.2)
    }

    /// The closest two of their tops are, metres.
    pub fn apart(&self, other: &Landmark) -> f32 {
        self.tops
            .iter()
            .flat_map(|a| other.tops.iter().map(move |b| dist(a.0, b.0)))
            .fold(f32::INFINITY, f32::min)
    }

    /// Where it is, for a finding: the middle of its tops.
    pub fn at(&self) -> P2 {
        let n = self.tops.len().max(1) as f32;
        let s = self.tops.iter().fold([0.0, 0.0], |s, t| geom::add(s, t.0));
        geom::scale(s, 1.0 / n)
    }
}

/// Every landmark on the map: placements at least `LANDMARK_HEIGHT` tall,
/// pieces of one kind within [`JOIN`] joined into one.
pub fn landmarks(map: &MapDef, hm: &Heightmap) -> Vec<Landmark> {
    let tall: Vec<usize> = (0..map.placements.len())
        .filter(|i| prefab_footprint(&map.placements[*i].prefab)[1] >= LANDMARK_HEIGHT)
        .collect();
    let outline: Vec<Vec<P2>> = tall
        .iter()
        .map(|i| spots(hm, &map.placements[*i]))
        .collect();
    // Union-find over the tall placements.
    let mut root: Vec<usize> = (0..tall.len()).collect();
    fn find(root: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    for a in 0..tall.len() {
        for b in a + 1..tall.len() {
            let (pa, pb) = (&map.placements[tall[a]], &map.placements[tall[b]]);
            if kind(&pa.prefab) != kind(&pb.prefab) {
                continue;
            }
            let gap = outline[a]
                .iter()
                .flat_map(|p| outline[b].iter().map(move |q| dist(*p, *q)))
                .fold(f32::INFINITY, f32::min);
            if gap < JOIN {
                let (ra, rb) = (find(&mut root, a), find(&mut root, b));
                root[ra] = rb;
            }
        }
    }
    let mut out: Vec<(usize, Landmark)> = Vec::new();
    for (k, i) in tall.iter().enumerate() {
        let r = find(&mut root, k);
        let p = &map.placements[*i];
        let t = tops(hm, p);
        if let Some((_, m)) = out.iter_mut().find(|(rr, _)| *rr == r) {
            m.pieces.push(*i);
            m.tops.extend(t);
            m.name = m.kind.clone();
        } else {
            out.push((
                r,
                Landmark {
                    kind: kind(&p.prefab).to_owned(),
                    name: p.prefab.clone(),
                    pieces: vec![*i],
                    tops: t,
                },
            ));
        }
    }
    let mut out: Vec<Landmark> = out.into_iter().map(|(_, m)| m).collect();
    // Two of a name (far enough apart to tell by where they are): say
    // which.
    let names: Vec<String> = out.iter().map(|m| m.name.clone()).collect();
    for m in out.iter_mut() {
        if names.iter().filter(|n| **n == m.name).count() > 1 {
            let at = m.at();
            m.name = format!("{} ({:.0}, {:.0})", m.name, at[0], at[1]);
        }
    }
    out
}

/// A tree's crown, for sight lines: from this far up its trunk...
const CROWN_BASE: f32 = 3.0;
/// ...to this tall (times its scale; the spruce run 15-22 m)...
const CROWN_TOP: f32 = 16.0;
/// ...and this wide each way.
const CROWN_RADIUS: f32 = 2.2;
/// A line through this many crowns is hidden; through fewer it glimpses.
const CROWNS_HIDE: usize = 2;

/// The valley's tree crowns, bucketed for sight lines through them.
pub struct Canopy {
    cells: std::collections::HashMap<(i32, i32), Vec<(P2, f32, f32)>>,
}

impl Canopy {
    const CELL: f32 = 8.0;

    pub fn new(map: &MapDef, hm: &Heightmap) -> Self {
        let mut cells: std::collections::HashMap<(i32, i32), Vec<(P2, f32, f32)>> =
            Default::default();
        for t in crate::forest::scatter_trees(map, hm) {
            let at = [t.pos[0], t.pos[2]];
            let foot = t.pos[1];
            cells.entry(Self::key(at)).or_default().push((
                at,
                foot + CROWN_BASE,
                foot + CROWN_TOP * t.scale,
            ));
        }
        Canopy { cells }
    }

    fn key(p: P2) -> (i32, i32) {
        (
            (p[0] / Self::CELL).floor() as i32,
            (p[1] / Self::CELL).floor() as i32,
        )
    }

    /// Whether the crowns hide a line from eye height at `from` to the
    /// point `top` metres up at `to`.
    pub fn hides(&self, hm: &Heightmap, from: P2, to: P2, top: f32) -> bool {
        let eye = hm.height(from) + 1.7;
        let d = dist(from, to);
        let n = (d / 2.0).ceil().max(1.0) as usize;
        let mut hit: Vec<P2> = Vec::new();
        for i in 1..n {
            let t = i as f32 / n as f32;
            let q = geom::lerp(from, to, t);
            let y = eye + (top - eye) * t;
            let (ci, cj) = Self::key(q);
            for di in -1..=1 {
                for dj in -1..=1 {
                    let Some(v) = self.cells.get(&(ci + di, cj + dj)) else {
                        continue;
                    };
                    for (at, lo, hi) in v {
                        if y > *lo
                            && y < *hi
                            && dist(*at, q) < CROWN_RADIUS
                            && !hit.iter().any(|h| dist(*h, *at) < 0.1)
                        {
                            hit.push(*at);
                            if hit.len() >= CROWNS_HIDE {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

/// A landmark reads from this far per metre it stands over its ground
/// (angular size), between the ordinary range and twice it: a 12 m tower
/// at the ordinary 200 m, a 45 m mast at 400.
const SIGHT_PER_METRE: f32 = 10.0;

impl Landmark {
    /// How far off it reads, given the ordinary `range`: further the taller
    /// it stands over its own ground.
    pub fn sight(&self, hm: &Heightmap, range: f32) -> f32 {
        let tall = self
            .tops
            .iter()
            .map(|(at, top)| top - hm.height(*at))
            .fold(0.0, f32::max);
        (tall * SIGHT_PER_METRE).clamp(range, range * 2.0)
    }

    /// As [`Landmark::seen`], with the trees in the way too.
    pub fn seen_through(
        &self,
        hm: &Heightmap,
        canopy: &Canopy,
        from: P2,
        range: f32,
    ) -> Option<f32> {
        let mut near: Vec<(P2, f32, f32)> = self
            .tops
            .iter()
            .map(|(at, top)| (*at, *top, dist(from, *at)))
            .filter(|t| t.2 <= range)
            .collect();
        near.sort_by(|a, b| a.2.total_cmp(&b.2));
        near.into_iter()
            .find(|(at, top, _)| {
                sees(hm, from, *at, top - hm.height(*at)) && !canopy.hides(hm, from, *at, *top)
            })
            .map(|t| t.2)
    }
}

/// How many landmarks you can tell apart from eye height at `p`, through
/// the land and the trees, each within its own [`Landmark::sight`] of the
/// ordinary `range`: two of a kind count once.
/// Counts up to `enough`.
pub fn kinds_in_sight(
    marks: &[Landmark],
    hm: &Heightmap,
    canopy: &Canopy,
    p: P2,
    range: f32,
    enough: usize,
) -> usize {
    let mut kinds: Vec<&str> = Vec::new();
    for m in marks {
        if kinds.contains(&m.kind.as_str()) {
            continue;
        }
        if m.seen_through(hm, canopy, p, m.sight(hm, range)).is_some() {
            kinds.push(&m.kind);
            if kinds.len() >= enough {
                break;
            }
        }
    }
    kinds.len()
}

/// How many lights you can name are in sight from eye height at `p`
/// through the land and the trees, counting up to `enough`: the
/// landmarks' lamps by their [`character`]s (each on its highest top,
/// within `lamp_range`), each rendezvous beacon (`beacons`: where it
/// stands, its own character) within `beacon_range`.
/// At night that's all there is to see; in fog, with `lamp_range` the fog's
/// reach, a landmark's whole height (any top) counts, the structure being
/// as visible as its lamp that near.
pub fn lights_in_sight(
    marks: &[Landmark],
    beacons: &[P2],
    hm: &Heightmap,
    canopy: &Canopy,
    p: P2,
    (lamp_range, beacon_range): (f32, f32),
    any_top: bool,
    enough: usize,
) -> usize {
    // Lamps you can name: one per character (two chimneys' steady lamps
    // in sight are one thing to steer by), each beacon its own.
    let mut seen_chars: Vec<&str> = Vec::new();
    let mut n = 0;
    for m in marks {
        let name = character(&m.kind).name;
        if seen_chars.contains(&name) {
            continue;
        }
        let seen = if any_top {
            m.seen_through(hm, canopy, p, lamp_range).is_some()
        } else {
            m.tops
                .iter()
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .is_some_and(|(at, top)| {
                    dist(p, *at) <= lamp_range
                        && sees(hm, p, *at, top - hm.height(*at))
                        && !canopy.hides(hm, p, *at, *top)
                })
        };
        if seen {
            seen_chars.push(name);
            n += 1;
            if n >= enough {
                return n;
            }
        }
    }
    for b in beacons {
        let top = hm.height(*b) + BEACON_HEIGHT;
        if dist(p, *b) <= beacon_range
            && sees(hm, p, *b, BEACON_HEIGHT)
            && !canopy.hides(hm, p, *b, top)
        {
            n += 1;
            if n >= enough {
                return n;
            }
        }
    }
    n
}

/// Whether `p` is further than `edge` into a wood.
pub fn deep_in_woods(map: &MapDef, p: P2, edge: f32) -> bool {
    map.forests.iter().any(|f| {
        geom::point_in_polygon(&f.polygon, p) && geom::polygon_edge_distance(&f.polygon, p) > edge
    })
}

/// Where orientation is gauged: walkable ground (on `walk`, every `step`
/// metres) inside the valley's regions, less what's deeper than `edge`
/// into a wood (nothing's in sight there in any valley: you steer by its
/// edge, and the wood is where you are), which is counted apart.
pub fn gauge_cells(
    map: &MapDef,
    walk: &crate::nav::NavGrid,
    step: f32,
    edge: f32,
) -> (Vec<P2>, usize) {
    let (mut out, mut deep) = (Vec::new(), 0);
    let stride = (step / walk.cell).round().max(1.0) as usize;
    for j in (0..walk.n).step_by(stride) {
        for i in (0..walk.n).step_by(stride) {
            let p = walk.centre(i, j);
            if walk.cost[j * walk.n + i] == crate::nav::BLOCKED
                || !map
                    .regions
                    .iter()
                    .any(|reg| geom::point_in_polygon(&reg.polygon, p))
            {
                continue;
            }
            if deep_in_woods(map, p, edge) {
                deep += 1;
                continue;
            }
            out.push(p);
        }
    }
    (out, deep)
}

/// A lamp's character, as the lights of masts and lights at sea have
/// them: when in each period it's lit. A landmark's is its kind's
/// ([`character`]), the same in every valley so it's learnt once, and no
/// two kinds in one valley share one, so two lamps in sight after dark
/// are two you can name: a fix, not only a bearing. The field map's key
/// lists them (`scripts/field_map.py`, held to these by a test).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Character {
    /// As the light lists write it: `Fl(2) 6s`.
    pub name: &'static str,
    /// Seconds.
    pub period: f32,
    /// When in the period it's lit, seconds from its start.
    pub lit: &'static [(f32, f32)],
}

impl Character {
    pub fn lit_at(&self, t: f32) -> bool {
        let u = t.rem_euclid(self.period);
        self.lit.iter().any(|(a, b)| u >= *a && u < *b)
    }
}

/// Steady: the chimneys' (an obstruction light).
pub const STEADY: Character = Character {
    name: "F",
    period: 1.0,
    lit: &[(0.0, 1.0)],
};
/// A quick flash, every second: the masts'.
pub const QUICK: Character = Character {
    name: "Q",
    period: 1.0,
    lit: &[(0.0, 0.35)],
};
pub const FLASH: Character = Character {
    name: "Fl 5s",
    period: 5.0,
    lit: &[(0.0, 0.6)],
};
pub const FLASH_2: Character = Character {
    name: "Fl(2) 6s",
    period: 6.0,
    lit: &[(0.0, 0.5), (1.2, 1.7)],
};
pub const FLASH_3: Character = Character {
    name: "Fl(3) 9s",
    period: 9.0,
    lit: &[(0.0, 0.5), (1.2, 1.7), (2.4, 2.9)],
};
pub const FLASH_4: Character = Character {
    name: "Fl(4) 12s",
    period: 12.0,
    lit: &[(0.0, 0.5), (1.2, 1.7), (2.4, 2.9), (3.6, 4.1)],
};
/// Lit, put out for a moment.
pub const OCCULTING: Character = Character {
    name: "Oc 4s",
    period: 4.0,
    lit: &[(0.0, 3.0)],
};
pub const OCCULTING_2: Character = Character {
    name: "Oc(2) 8s",
    period: 8.0,
    lit: &[(0.0, 5.0), (5.8, 7.2)],
};
/// As long lit as dark.
pub const ISOPHASE: Character = Character {
    name: "Iso 4s",
    period: 4.0,
    lit: &[(0.0, 2.0)],
};
pub const LONG_FLASH: Character = Character {
    name: "LFl 8s",
    period: 8.0,
    lit: &[(0.0, 2.0)],
};

/// A landmark kind's lamp ([`kind`]).
pub fn character(kind: &str) -> Character {
    match kind {
        "chimney" => STEADY,
        "radio_mast" => QUICK,
        "steeple" => FLASH,
        "ranger_tower" => FLASH_2,
        "survey_tower" => FLASH_3,
        "robot_gate" => FLASH_4,
        "water_tower" => OCCULTING,
        "draw_off_tower" | "mill_race" | "ropeway_station" | "observatory" | "factory_hall" => {
            OCCULTING_2
        }
        "headframe" | "sorting_gantry" | "funicular_upper" => ISOPHASE,
        "dam" | "blast_furnace" | "ropeway_pylon" | "ore_mill" => LONG_FLASH,
        _ => STEADY,
    }
}

/// The rendezvous beacons are green, each flashing its number (the map's
/// order of them: 1, 2, 3...), so each says which way out it is.
pub fn beacon_character(i: usize) -> Character {
    const B: [Character; 4] = [
        Character {
            name: "G Fl 5s",
            period: 5.0,
            lit: &[(0.0, 0.8)],
        },
        Character {
            name: "G Fl(2) 7s",
            period: 7.0,
            lit: &[(0.0, 0.8), (1.6, 2.4)],
        },
        Character {
            name: "G Fl(3) 9s",
            period: 9.0,
            lit: &[(0.0, 0.8), (1.6, 2.4), (3.2, 4.0)],
        },
        Character {
            name: "G Fl(4) 11s",
            period: 11.0,
            lit: &[(0.0, 0.8), (1.6, 2.4), (3.2, 4.0), (4.8, 5.6)],
        },
    ];
    B[i.min(B.len() - 1)]
}

/// A rendezvous's beacon: a pole with a green lamp flashing its number
/// ([`beacon_character`]; the game's `render::rv_beacons`), so the way out
/// can be found from the valley side before the truck's there. It stands this far off the side of where the
/// truck parks (`raid::truck::rv_pose`: its bed toward the map's middle)...
pub const BEACON_OFF: f32 = 7.0;
/// ...its lamp this high.
pub const BEACON_HEIGHT: f32 = 6.5;

/// Where the beacon of the rendezvous at `rv` stands.
pub fn beacon_at(rv: P2) -> P2 {
    let l = (rv[0] * rv[0] + rv[1] * rv[1]).sqrt();
    let inward = if l > 1e-3 {
        [-rv[0] / l, -rv[1] / l]
    } else {
        [0.0, 1.0]
    };
    [
        rv[0] - inward[1] * BEACON_OFF,
        rv[1] + inward[0] * BEACON_OFF,
    ]
}

/// Whether any landmark's top is in sight from `p` within `range`.
pub fn in_sight(marks: &[Landmark], hm: &Heightmap, p: P2, range: f32) -> bool {
    marks.iter().any(|m| m.seen(hm, p, range).is_some())
}

/// Every landmark in sight from `p` within `range`, nearest first, with
/// how far off it is.
pub fn seen_from(marks: &[Landmark], hm: &Heightmap, p: P2, range: f32) -> Vec<(String, f32)> {
    let mut v: Vec<(String, f32)> = marks
        .iter()
        .filter_map(|m| m.seen(hm, p, range).map(|d| (m.name.clone(), d)))
        .collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    v
}

/// The tallest thing standing in a region: any placement whose footprint,
/// middle or a top lies inside it (a dam whose middle is over the line
/// still stands in the region its end is in).
pub fn tallest_in(map: &MapDef, hm: &Heightmap, polygon: &[P2]) -> f32 {
    map.placements
        .iter()
        .filter(|p| {
            spots(hm, p)
                .iter()
                .any(|q| geom::point_in_polygon(polygon, *q))
        })
        .map(|p| prefab_footprint(&p.prefab)[1])
        .fold(0.0, f32::max)
}

/// Each main route's landmarks: for every landmark seen from it, the share
/// of the route it's in sight from in ordinary weather and in fog, and the
/// stretch (metres along the route) it's first and last seen over.
pub fn along(
    marks: &[Landmark],
    hm: &Heightmap,
    points: &[P2],
    clear: f32,
    fog: f32,
) -> RouteSight {
    let mut run = vec![0.0; points.len()];
    for i in 1..points.len() {
        run[i] = run[i - 1] + dist(points[i - 1], points[i]);
    }
    let n = points.len().max(1) as f32;
    let seen: Vec<Vec<Option<f32>>> = marks
        .iter()
        .map(|m| points.iter().map(|p| m.seen(hm, *p, clear)).collect())
        .collect();
    let mut out = RouteSight::default();
    for (m, seen) in marks.iter().zip(&seen) {
        let c = seen.iter().filter(|s| s.is_some()).count();
        if c == 0 {
            continue;
        }
        let f = seen.iter().filter(|s| s.is_some_and(|d| d <= fog)).count();
        let first = seen.iter().position(|s| s.is_some()).unwrap();
        let last = seen.iter().rposition(|s| s.is_some()).unwrap();
        out.each.push((
            m.name.clone(),
            c as f32 / n,
            f as f32 / n,
            run[first],
            run[last],
        ));
    }
    out.each.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (a, ma) in marks.iter().enumerate() {
        for (b, mb) in marks.iter().enumerate().skip(a + 1) {
            if ma.kind != mb.kind {
                continue;
            }
            let both: Vec<usize> = (0..points.len())
                .filter(|i| seen[a][*i].is_some() && seen[b][*i].is_some())
                .collect();
            if let Some(i) = both.first() {
                out.twins.push((
                    format!("{} and {}", ma.name, mb.name),
                    ma.kind.clone(),
                    both.len() as f32 / n,
                    points[*i],
                ));
            }
        }
    }
    out
}

/// What a main route sees ([`along`]).
#[derive(Clone, Debug, Default)]
pub struct RouteSight {
    /// Each landmark in sight from it in ordinary weather: its name, the
    /// share of the route it's in sight from, the share within fog range,
    /// and how far along the route it's first and last seen, metres.
    pub each: Vec<(String, f32, f32, f32, f32)>,
    /// Two of a kind in sight at once: which, their kind, the share of the
    /// route they're both in sight from, and where that starts.
    pub twins: Vec<(String, String, f32, P2)>,
}

#[cfg(test)]
mod tests {

    /// After dark two lamps in sight are two you can name: in every valley
    /// no two kinds of landmark share a lamp's character, and every kind
    /// has one of its own choosing (not the steady lamp by default).
    #[test]
    fn every_valleys_lamps_tell_apart() {
        let named = [
            "chimney",
            "radio_mast",
            "steeple",
            "ranger_tower",
            "survey_tower",
            "robot_gate",
            "water_tower",
            "draw_off_tower",
            "mill_race",
            "ropeway_station",
            "observatory",
            "factory_hall",
            "headframe",
            "sorting_gantry",
            "funicular_upper",
            "dam",
            "blast_furnace",
            "ropeway_pylon",
            "ore_mill",
        ];
        for (id, text) in crate::map::VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let mut by: Vec<(String, &str)> = Vec::new();
            for m in landmarks(&map, &hm) {
                assert!(
                    named.contains(&m.kind.as_str()),
                    "{id}: {} has no lamp character of its own",
                    m.kind
                );
                let c = character(&m.kind).name;
                if let Some((k, _)) = by.iter().find(|(k, n)| *n == c && *k != m.kind) {
                    panic!("{id}: {} and {k} both {c}", m.kind);
                }
                by.push((m.kind.clone(), c));
            }
        }
    }

    /// The characters are what they say: lit for some of the period, dark
    /// for some (but the steady one), and the beacons all different.
    #[test]
    fn characters_flash_as_written() {
        for c in [
            STEADY,
            QUICK,
            FLASH,
            FLASH_2,
            FLASH_3,
            FLASH_4,
            OCCULTING,
            OCCULTING_2,
            ISOPHASE,
            LONG_FLASH,
        ] {
            let lit = (0..1000)
                .filter(|i| c.lit_at(*i as f32 * c.period / 1000.0))
                .count();
            if c.name == "F" {
                assert_eq!(lit, 1000);
            } else {
                assert!(lit > 0 && lit < 1000, "{}: lit {lit} of 1000", c.name);
            }
            let flashes = c
                .name
                .split(['(', ')'])
                .nth(1)
                .and_then(|n| n.parse::<usize>().ok());
            if let Some(n) = flashes {
                assert_eq!(c.lit.len(), n, "{}", c.name);
            }
        }
        let b: Vec<&str> = (0..4).map(|i| beacon_character(i).name).collect();
        for (i, x) in b.iter().enumerate() {
            assert!(!b[..i].contains(x));
        }
    }
    use super::*;
    use crate::map::{SALLOW_RESERVOIR, TRAINING_YARD, TerrainFeature};

    /// A bare plain 640 m across, nothing on it.
    fn plain() -> MapDef {
        let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
        map.size = 640.0;
        map.noise.amplitude = 0.0;
        map.rim.height = 0.0;
        map.features.clear();
        map.roads.clear();
        map.water.clear();
        map.forests.clear();
        map.placements.clear();
        map
    }

    fn place(map: &mut MapDef, prefab: &str, at: P2, rot: f32) {
        map.placements.push(Placement {
            prefab: prefab.into(),
            at,
            rot,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        });
    }

    /// Sallow's dam is one landmark of four pieces; the draw-off tower is
    /// its own, its top where its absolute foot puts it; and the dam's top
    /// is the parapet's, just over the crest.
    #[test]
    fn the_dam_is_one_landmark() {
        let map = MapDef::from_ron(SALLOW_RESERVOIR).unwrap();
        let hm = Heightmap::build(&map);
        let marks = landmarks(&map, &hm);
        let dams: Vec<&Landmark> = marks.iter().filter(|m| m.kind == "dam").collect();
        assert_eq!(dams.len(), 1, "{marks:?}");
        assert_eq!(dams[0].pieces.len(), 4);
        assert!(dams[0].tops.len() > 20, "{}", dams[0].tops.len());
        // The crest is at 52 m; the parapet and the turrets stand over it.
        for (at, top) in &dams[0].tops {
            assert!(*top > 52.0 && *top < 60.0, "{at:?} {top}");
        }
        let tower = marks.iter().find(|m| m.kind == "draw_off_tower").unwrap();
        assert!((tower.tops[0].1 - 65.5).abs() < 0.01, "{:?}", tower.tops);
    }

    /// A long wall is seen from off its end, where its middle is out of
    /// range, and over a hill that hides its middle.
    #[test]
    fn a_long_wall_is_seen_along_its_length() {
        let mut map = plain();
        place(&mut map, "dam_face", [0.0, 0.0], 0.0);
        // A hill 40 m out in front of the wall's middle.
        map.features.push(TerrainFeature::Hill {
            at: [0.0, 40.0],
            radius: 25.0,
            height: 30.0,
        });
        let hm = Heightmap::build(&map);
        let marks = landmarks(&map, &hm);
        assert_eq!(marks.len(), 1);
        let middle_only = Landmark {
            tops: vec![([0.0, 0.0], marks[0].tops[0].1)],
            ..marks[0].clone()
        };
        // 30 m out past its east end (x 132.5), 160 m from its middle.
        let off_end = [160.0, 20.0];
        let d = marks[0].seen(&hm, off_end, 150.0);
        assert!(d.is_some_and(|d| d < 40.0), "{d:?}");
        assert!(middle_only.seen(&hm, off_end, 150.0).is_none());
        // Behind the hill, the middle is hidden and the ends are not.
        let behind = [0.0, 110.0];
        assert!(middle_only.seen(&hm, behind, 320.0).is_none());
        assert!(marks[0].seen(&hm, behind, 320.0).is_some());
    }

    /// Pieces of one kind close together are one structure; a chapel and a
    /// steeple are two of a kind (the same tower), steered by at the tower.
    #[test]
    fn kinds_and_joins() {
        assert_eq!(kind("chapel_drowned"), kind("steeple"));
        assert_eq!(kind("dam_back_end"), kind("dam_face"));
        assert_ne!(kind("chimney"), kind("blast_furnace"));
        let mut map = plain();
        place(&mut map, "chapel", [-60.0, 0.0], 0.0);
        place(&mut map, "steeple", [60.0, 0.0], 0.0);
        place(&mut map, "radio_mast", [0.0, 60.0], 0.0);
        place(&mut map, "radio_mast", [10.0, 60.0], 0.0);
        let hm = Heightmap::build(&map);
        let marks = landmarks(&map, &hm);
        let steeples: Vec<&Landmark> = marks.iter().filter(|m| m.kind == "steeple").collect();
        assert_eq!(steeples.len(), 2, "{marks:?}");
        assert!((steeples[0].apart(steeples[1]) - 120.0).abs() < 2.0);
        let masts: Vec<&Landmark> = marks.iter().filter(|m| m.kind == "radio_mast").collect();
        assert_eq!(masts.len(), 1);
        assert_eq!(masts[0].pieces.len(), 2);
        assert_eq!(masts[0].name, "radio_mast");
        let chapel = steeples.iter().find(|m| m.name == "chapel").unwrap();
        assert!(
            (chapel.tops[0].0[1] - 9.175).abs() < 0.01,
            "{:?}",
            chapel.tops
        );
    }

    /// A long thing whose middle is outside a region but whose end is in
    /// it stands in it.
    #[test]
    fn a_wall_stands_in_every_region_it_reaches() {
        let mut map = plain();
        place(&mut map, "dam_back", [0.0, 0.0], 0.0);
        let hm = Heightmap::build(&map);
        let east = [[100.0, -20.0], [160.0, -20.0], [160.0, 20.0], [100.0, 20.0]];
        assert!(tallest_in(&map, &hm, &east) >= 20.0);
        let north = [[-20.0, -80.0], [20.0, -80.0], [20.0, -40.0], [-20.0, -40.0]];
        assert_eq!(tallest_in(&map, &hm, &north), 0.0);
    }
}
