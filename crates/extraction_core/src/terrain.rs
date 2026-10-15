//! Builds a heightmap (and surface masks) from a [`MapDef`].
//!
//! Order of operations, each pass over the whole grid:
//!
//! 1. base height + fBm noise + tilt + hills + ridges + channels
//! 2. plateaus flatten toward their target height
//! 3. the rim rises toward the edge
//! 4. roads cut and fill to a smoothed grade, and paint the road mask
//! 5. water marks its surface level and wets the banks
//!
//! The result is pure data, identical everywhere, cheap to sample.

use crate::geom::{self, P2};
use crate::map::{MapDef, RoadKind, TerrainFeature};
use crate::noise;

/// What the ground is made of at a point, for footsteps, materials and
/// friction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    Grass,
    Forest,
    Dirt,
    Mud,
    Rock,
    Gravel,
    Asphalt,
    Water,
}

/// Per-cell road paint: which kind of road, and how strongly (0..=255).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoadPaint {
    pub kind: Option<RoadKind>,
    pub weight: u8,
}

#[derive(Clone, Debug)]
pub struct Heightmap {
    /// Samples per side.
    pub n: usize,
    /// Metres between samples.
    pub cell: f32,
    /// World x/z of sample (0, 0).
    pub origin: f32,
    pub heights: Vec<f32>,
    pub roads: Vec<RoadPaint>,
    /// Water surface height where water is, `NaN` elsewhere.
    pub water: Vec<f32>,
    /// 0..=1 wetness from nearby water, for mud and dark banks.
    pub wet: Vec<f32>,
    /// Each road's sampled centreline and graded heights, for road meshes,
    /// bridges and AI routing.
    pub road_lines: Vec<RoadLine>,
}

/// A road as built: its centreline sampled every metre with the height the
/// grade put it at.
#[derive(Clone, Debug)]
pub struct RoadLine {
    /// Index into `MapDef::roads`.
    pub road: usize,
    pub kind: RoadKind,
    pub width: f32,
    pub points: Vec<P2>,
    pub heights: Vec<f32>,
    /// Arc-length ranges `(start, end)` that are bridge decks.
    pub bridges: Vec<(f32, f32)>,
}

impl RoadLine {
    /// Whether arc length `s` is on a bridge.
    pub fn on_bridge(&self, s: f32) -> bool {
        self.bridges.iter().any(|(a, b)| s >= *a && s <= *b)
    }
}

impl Heightmap {
    pub fn build(map: &MapDef) -> Self {
        let n = (map.size / map.cell).round() as usize + 1;
        let origin = -map.half();
        let mut hm = Self {
            n,
            cell: map.cell,
            origin,
            heights: vec![map.base_height; n * n],
            roads: vec![RoadPaint::default(); n * n],
            water: vec![f32::NAN; n * n],
            wet: vec![0.0; n * n],
            road_lines: Vec::new(),
        };
        hm.shape(map);
        hm.plateaus(map);
        hm.pads(map);
        hm.rim(map);
        // Water levels come from the land before roads cut and fill it, so
        // a ford's raised bed doesn't raise the creek.
        for water in &map.water {
            hm.water(water);
        }
        for (i, road) in map.roads.iter().enumerate() {
            hm.road(i, road);
        }
        hm.settle(map);
        hm
    }

    pub fn idx(&self, i: usize, j: usize) -> usize {
        j * self.n + i
    }

    /// World position of sample `(i, j)`.
    pub fn pos(&self, i: usize, j: usize) -> P2 {
        [
            self.origin + i as f32 * self.cell,
            self.origin + j as f32 * self.cell,
        ]
    }

    /// Fractional grid coordinates of a world point, clamped to the grid.
    fn grid(&self, p: P2) -> (f32, f32) {
        let max = (self.n - 1) as f32;
        (
            ((p[0] - self.origin) / self.cell).clamp(0.0, max),
            ((p[1] - self.origin) / self.cell).clamp(0.0, max),
        )
    }

    fn bilinear(&self, data: &[f32], p: P2) -> f32 {
        let (gx, gz) = self.grid(p);
        let i = (gx.floor() as usize).min(self.n - 2);
        let j = (gz.floor() as usize).min(self.n - 2);
        let fx = gx - i as f32;
        let fz = gz - j as f32;
        let h00 = data[self.idx(i, j)];
        let h10 = data[self.idx(i + 1, j)];
        let h01 = data[self.idx(i, j + 1)];
        let h11 = data[self.idx(i + 1, j + 1)];
        let a = h00 + (h10 - h00) * fx;
        let b = h01 + (h11 - h01) * fx;
        a + (b - a) * fz
    }

    /// Ground height at a world point.
    pub fn height(&self, p: P2) -> f32 {
        self.bilinear(&self.heights, p)
    }

    /// Unit surface normal (x, y, z) at a world point.
    pub fn normal(&self, p: P2) -> [f32; 3] {
        let e = self.cell;
        let hx = self.height([p[0] + e, p[1]]) - self.height([p[0] - e, p[1]]);
        let hz = self.height([p[0], p[1] + e]) - self.height([p[0], p[1] - e]);
        let v = [-hx, 2.0 * e, -hz];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l]
    }

    /// Slope in degrees.
    pub fn slope_deg(&self, p: P2) -> f32 {
        self.normal(p)[1].clamp(-1.0, 1.0).acos().to_degrees()
    }

    fn nearest(&self, p: P2) -> usize {
        let (gx, gz) = self.grid(p);
        self.idx(gx.round() as usize, gz.round() as usize)
    }

    /// Water surface height at a point, if it is under water.
    pub fn water_level(&self, p: P2) -> Option<f32> {
        let w = self.water[self.nearest(p)];
        (!w.is_nan() && w > self.height(p)).then_some(w)
    }

    pub fn road_at(&self, p: P2) -> RoadPaint {
        self.roads[self.nearest(p)]
    }

    pub fn wetness(&self, p: P2) -> f32 {
        self.bilinear(&self.wet, p)
    }

    /// What you're standing on. `forest` says whether a forest polygon
    /// covers the point (the caller knows; the heightmap doesn't).
    pub fn surface(&self, p: P2, forest: bool) -> Surface {
        if self.water_level(p).is_some() {
            return Surface::Water;
        }
        let road = self.road_at(p);
        if road.weight > 128 {
            return match road.kind {
                Some(RoadKind::Asphalt) => Surface::Asphalt,
                Some(RoadKind::Gravel | RoadKind::Rail) => Surface::Gravel,
                _ => Surface::Dirt,
            };
        }
        if self.slope_deg(p) > 38.0 {
            return Surface::Rock;
        }
        if self.wetness(p) > 0.6 {
            return Surface::Mud;
        }
        if forest {
            Surface::Forest
        } else {
            Surface::Grass
        }
    }

    fn shape(&mut self, map: &MapDef) {
        let nd = &map.noise;
        let splines: Vec<(Vec<P2>, &TerrainFeature)> = map
            .features
            .iter()
            .filter_map(|f| match f {
                TerrainFeature::Ridge { points, .. } | TerrainFeature::Channel { points, .. } => {
                    Some((geom::catmull_rom(points, 2.0), f))
                }
                _ => None,
            })
            .collect();
        for j in 0..self.n {
            for i in 0..self.n {
                let p = self.pos(i, j);
                let mut h = 0.0;
                if nd.amplitude != 0.0 {
                    h += nd.amplitude
                        * noise::fbm(p[0] / nd.scale, p[1] / nd.scale, nd.seed, nd.octaves, 0.5);
                }
                if nd.detail != 0.0 {
                    h += nd.detail * noise::fbm(p[0] / 6.0, p[1] / 6.0, nd.seed ^ 0xABCD, 2, 0.5);
                }
                for f in &map.features {
                    match f {
                        TerrainFeature::Hill { at, radius, height } => {
                            let d = geom::dist(p, *at) / radius.max(0.01);
                            if d < 1.0 {
                                // Smooth cosine bell.
                                h += height * 0.5 * (1.0 + (d * std::f32::consts::PI).cos());
                            }
                        }
                        TerrainFeature::Tilt { dir, rise_per_100m } => {
                            let l = geom::len(*dir).max(1e-6);
                            h += geom::dot(p, [dir[0] / l, dir[1] / l]) * rise_per_100m / 100.0;
                        }
                        _ => {}
                    }
                }
                for (line, f) in &splines {
                    let (d, _, _) = geom::polyline_closest(line, p);
                    match f {
                        TerrainFeature::Ridge { width, height, .. } => {
                            let t = 1.0 - geom::smoothstep(0.0, *width * 0.5, d);
                            h += height * t;
                        }
                        TerrainFeature::Channel { width, depth, .. } => {
                            // Flat-ish bed, sloped banks.
                            let t = 1.0 - geom::smoothstep(*width * 0.25, *width * 0.5, d);
                            h -= depth * t;
                        }
                        _ => {}
                    }
                }
                let k = self.idx(i, j);
                self.heights[k] += h;
            }
        }
    }

    fn plateaus(&mut self, map: &MapDef) {
        for f in &map.features {
            let TerrainFeature::Plateau {
                polygon,
                height,
                blend,
            } = f
            else {
                continue;
            };
            let (lo, hi) = geom::bounds(polygon);
            let height = height.unwrap_or_else(|| {
                let (mut sum, mut count) = (0.0, 0);
                self.for_cells_in(lo, hi, 0.0, |hm, k, p| {
                    if geom::point_in_polygon(polygon, p) {
                        sum += hm.heights[k];
                        count += 1;
                    }
                });
                if count > 0 { sum / count as f32 } else { 0.0 }
            });
            self.for_cells_in(lo, hi, *blend, |hm, k, p| {
                let inside = geom::point_in_polygon(polygon, p);
                let d = geom::polygon_edge_distance(polygon, p);
                let t = if inside {
                    1.0
                } else {
                    1.0 - geom::smoothstep(0.0, blend.max(0.01), d)
                };
                hm.heights[k] += (height - hm.heights[k]) * t;
            });
        }
    }

    /// Levels the ground under every building to its average height, with
    /// a few metres of blend, so floors meet the ground.
    fn pads(&mut self, map: &MapDef) {
        for p in &map.placements {
            if p.absolute || !crate::map::prefab_wants_pad(&p.prefab) {
                continue;
            }
            let [w, _, d] = crate::map::prefab_footprint(&p.prefab);
            let (half_w, half_d) = (w * 0.5 + 0.8, d * 0.5 + 0.8);
            let yaw = -p.rot.to_radians();
            let (s, c) = yaw.sin_cos();
            // Local (x, z) of a world point, in the building's frame.
            let local = |q: P2| {
                let dx = q[0] - p.at[0];
                let dz = q[1] - p.at[1];
                [dx * c - dz * s, dx * s + dz * c]
            };
            let reach = half_w.max(half_d) + 6.0;
            let lo = [p.at[0] - reach, p.at[1] - reach];
            let hi = [p.at[0] + reach, p.at[1] + reach];
            let (mut sum, mut n) = (0.0, 0);
            self.for_cells_in(lo, hi, 0.0, |hm, k, q| {
                let l = local(q);
                if l[0].abs() <= half_w && l[1].abs() <= half_d {
                    sum += hm.heights[k];
                    n += 1;
                }
            });
            if n == 0 {
                continue;
            }
            let target = sum / n as f32;
            let blend = 5.0;
            self.for_cells_in(lo, hi, 0.0, |hm, k, q| {
                let l = local(q);
                let out = ((l[0].abs() - half_w).max(0.0)).hypot((l[1].abs() - half_d).max(0.0));
                let t = 1.0 - geom::smoothstep(0.0, blend, out);
                hm.heights[k] += (target - hm.heights[k]) * t;
            });
        }
    }

    /// Where a placement stands: the lowest ground under its rough
    /// footprint (four corners and centre), lifted toward the highest by
    /// up to its foundation's allowance (`map::prefab_lift`). The game, the
    /// terrain's settling and the Unity export all use this.
    pub fn footing(&self, prefab: &str, at: P2, rot_deg: f32) -> f32 {
        let [w, _, d] = crate::map::prefab_footprint(prefab);
        let (s, c) = rot_deg.to_radians().sin_cos();
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for (sx, sz) in [
            (-0.5, -0.5),
            (0.5, -0.5),
            (0.5, 0.5),
            (-0.5, 0.5),
            (0.0, 0.0),
        ] {
            let (lx, lz) = (sx * w, sz * d);
            // The game's Quat::from_rotation_y(-rot) applied to (lx, 0, lz).
            let h = self.height([at[0] + lx * c - lz * s, at[1] + lx * s + lz * c]);
            lo = lo.min(h);
            hi = hi.max(h);
        }
        hi.min(lo + crate::map::prefab_lift(prefab))
    }

    /// Roads are graded after the pads, and a road's fill can rise inside
    /// a building: under every floor, the ground goes no higher than the
    /// footing the building stands on (its lowest corner, as the game
    /// places it), so the ground never pokes up through a floor.
    fn settle(&mut self, map: &MapDef) {
        // Whether a point is under some other placement (a gate stands on
        // its lowest corner: dig under a post and the whole gate sinks).
        let under_other = |me: usize, q: P2| {
            map.placements.iter().enumerate().any(|(i, o)| {
                if i == me {
                    return false;
                }
                let [w, _, d] = crate::map::prefab_footprint(&o.prefab);
                let (s, c) = (-o.rot.to_radians()).sin_cos();
                let (dx, dz) = (q[0] - o.at[0], q[1] - o.at[1]);
                let l = [dx * c - dz * s, dx * s + dz * c];
                l[0].abs() <= w * 0.5 + 1.0 && l[1].abs() <= d * 0.5 + 1.0
            })
        };
        for (me, p) in map.placements.iter().enumerate() {
            if p.absolute || !crate::map::prefab_wants_pad(&p.prefab) {
                continue;
            }
            let [w, _, d] = crate::map::prefab_footprint(&p.prefab);
            let yaw = -p.rot.to_radians();
            let (s, c) = yaw.sin_cos();
            let footing = |hm: &Self| hm.footing(&p.prefab, p.at, p.rot);
            // Flat to a cell past the walls (so nothing between samples
            // rises inside), then a ramp no steeper than half a metre per
            // metre back up to the ground as it was (no ditch round it).
            const RAMP: f32 = 0.5;
            let (half_w, half_d) = (w * 0.5 + self.cell, d * 0.5 + self.cell);
            let reach = half_w.max(half_d) + 8.0;
            let lo = [p.at[0] - reach, p.at[1] - reach];
            let hi = [p.at[0] + reach, p.at[1] + reach];
            // Lowering cells can lower the corner the building stands on in
            // turn: settle until it stops moving.
            for _ in 0..8 {
                let y = footing(self);
                self.for_cells_in(lo, hi, 0.0, |hm, k, q| {
                    let (dx, dz) = (q[0] - p.at[0], q[1] - p.at[1]);
                    let l = [dx * c - dz * s, dx * s + dz * c];
                    let out = (l[0].abs() - half_w)
                        .max(0.0)
                        .hypot((l[1].abs() - half_d).max(0.0));
                    // Past its own walls it leaves roads and other
                    // structures be: a booth by the gate would dig a dip in
                    // the road through it.
                    let inside = l[0].abs() <= w * 0.5 && l[1].abs() <= d * 0.5;
                    if !inside && (hm.roads[k].weight > 0 || under_other(me, q)) {
                        return;
                    }
                    hm.heights[k] = hm.heights[k].min(y + out * RAMP);
                });
                if footing(self) >= y - 1e-3 {
                    break;
                }
            }
        }
    }

    fn rim(&mut self, map: &MapDef) {
        let half = map.half();
        let rim = &map.rim;
        if rim.width <= 0.0 {
            return;
        }
        for j in 0..self.n {
            for i in 0..self.n {
                let p = self.pos(i, j);
                let edge = half - p[0].abs().max(p[1].abs());
                let t = 1.0 - geom::smoothstep(0.0, rim.width, edge);
                let k = self.idx(i, j);
                self.heights[k] += rim.height * t * t;
            }
        }
    }

    /// Visits cells inside the bounding box `lo..hi` grown by `pad`.
    fn for_cells_in(&mut self, lo: P2, hi: P2, pad: f32, mut f: impl FnMut(&mut Self, usize, P2)) {
        let (i0, j0) = self.grid([lo[0] - pad, lo[1] - pad]);
        let (i1, j1) = self.grid([hi[0] + pad, hi[1] + pad]);
        for j in j0.floor() as usize..=j1.ceil() as usize {
            for i in i0.floor() as usize..=i1.ceil() as usize {
                if i < self.n && j < self.n {
                    let k = self.idx(i, j);
                    let p = self.pos(i, j);
                    f(self, k, p);
                }
            }
        }
    }

    /// Cuts and fills a road to a smoothed grade and paints the road mask.
    /// Bridge spans keep the grade (interpolated end to end) but leave the
    /// ground under them alone.
    fn road(&mut self, index: usize, road: &crate::map::RoadDef) {
        let line = geom::catmull_rom(&road.points, 1.0);
        if line.len() < 2 {
            return;
        }
        // Arc length of every sample.
        let mut arc = vec![0.0; line.len()];
        for i in 1..line.len() {
            arc[i] = arc[i - 1] + geom::dist(line[i - 1], line[i]);
        }
        let bridges: Vec<(f32, f32)> = road
            .bridges
            .iter()
            .map(|b| {
                let (_, s, _) = geom::polyline_closest(&line, b.at);
                (s - b.length * 0.5, s + b.length * 0.5)
            })
            .collect();
        let on_bridge = |s: f32| bridges.iter().any(|(a, b)| s >= *a && s <= *b);
        // Grade profile: terrain height along the centreline (bridge spans
        // excluded), box-smoothed.
        let raw: Vec<f32> = line.iter().map(|p| self.height(*p)).collect();
        let win = road.smoothing.max(0.0).round() as isize;
        let mut profile: Vec<f32> = (0..raw.len() as isize)
            .map(|i| {
                let a = (i - win).max(0) as usize;
                let b = ((i + win) as usize).min(raw.len() - 1);
                let (mut sum, mut n) = (0.0, 0);
                for k in a..=b {
                    if !on_bridge(arc[k]) {
                        sum += raw[k];
                        n += 1;
                    }
                }
                if n > 0 {
                    sum / n as f32
                } else {
                    raw[i as usize]
                }
            })
            .collect();
        // Decks run straight between their abutments.
        for (a, b) in &bridges {
            let ia = arc.iter().position(|s| s >= a).unwrap_or(0);
            let ib = arc.iter().rposition(|s| s <= b).unwrap_or(arc.len() - 1);
            if ib <= ia {
                continue;
            }
            let (ha, hb) = (profile[ia], profile[ib]);
            for k in ia..=ib {
                let t = (arc[k] - arc[ia]) / (arc[ib] - arc[ia]).max(1e-3);
                profile[k] = ha + (hb - ha) * t;
            }
        }
        let half_w = road.width * 0.5;
        let reach = half_w + road.shoulder;
        // Nearest centreline sample per touched cell.
        let mut best: std::collections::HashMap<usize, (f32, usize)> = Default::default();
        for (s, p) in line.iter().enumerate() {
            let (i0, j0) = self.grid([p[0] - reach, p[1] - reach]);
            let (i1, j1) = self.grid([p[0] + reach, p[1] + reach]);
            for j in j0.floor() as usize..=(j1.ceil() as usize).min(self.n - 1) {
                for i in i0.floor() as usize..=(i1.ceil() as usize).min(self.n - 1) {
                    let d = geom::dist(self.pos(i, j), *p);
                    if d > reach {
                        continue;
                    }
                    let k = self.idx(i, j);
                    let e = best.entry(k).or_insert((f32::INFINITY, 0));
                    if d < e.0 {
                        *e = (d, s);
                    }
                }
            }
        }
        for (k, (d, s)) in best {
            if on_bridge(arc[s]) {
                // Under the deck: cut away any bank that would come up
                // through it (the deck runs straight; the bank needn't).
                // Not at its ends: there the ground is the abutment, built
                // up or cut down to just under the deck's top. Otherwise
                // the grid spreads a cut or a low bank a cell's width past
                // the deck, and the step off the end drops into a ditch you
                // can't climb out of onto the road.
                let end = bridges
                    .iter()
                    .any(|(a, b)| arc[s] < a + 1.5 || arc[s] > b - 1.5);
                if d <= half_w + 0.5 {
                    if end {
                        self.heights[k] = profile[s] - 0.05;
                    } else if self.heights[k] > profile[s] - 0.5 {
                        self.heights[k] = profile[s] - 0.5;
                    }
                }
                continue;
            }
            let target = profile[s];
            let t = 1.0 - geom::smoothstep(half_w, reach, d);
            // Slight camber: the crown sits a few cm above the edges.
            let crown = 0.08 * (1.0 - (d / half_w.max(0.01)).min(1.0));
            self.heights[k] += (target + crown - self.heights[k]) * t;
            let paint = (255.0 * (1.0 - geom::smoothstep(half_w - 0.5, half_w + 0.5, d))) as u8;
            if paint > self.roads[k].weight {
                self.roads[k] = RoadPaint {
                    kind: Some(road.kind),
                    weight: paint,
                };
            }
        }
        self.road_lines.push(RoadLine {
            road: index,
            kind: road.kind,
            width: road.width,
            points: line,
            heights: profile,
            bridges,
        });
    }

    fn water(&mut self, water: &crate::map::WaterDef) {
        let line = geom::catmull_rom(&water.points, 1.0);
        if line.is_empty() {
            return;
        }
        let half_w = water.width * 0.5;
        let bank = 8.0;
        let reach = half_w + bank;
        let (lo, hi) = geom::bounds(&line);
        // Bed profile: the lowest ground across the stream at each sample,
        // then smoothed so the surface runs level instead of following bumps.
        let beds: Vec<f32> = line
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let next = line[(i + 1).min(line.len() - 1)];
                let prev = line[i.saturating_sub(1)];
                let d = geom::sub(next, prev);
                let l = geom::len(d).max(1e-6);
                let nrm = [-d[1] / l, d[0] / l];
                (-4..=4)
                    .map(|k| self.height(geom::add(*p, geom::scale(nrm, k as f32 * half_w / 4.0))))
                    .fold(f32::INFINITY, f32::min)
            })
            .collect();
        let win = 12usize;
        let surface: Vec<f32> = (0..beds.len())
            .map(|i| {
                let a = i.saturating_sub(win);
                let b = (i + win).min(beds.len() - 1);
                beds[a..=b].iter().sum::<f32>() / (b - a + 1) as f32 + water.depth
            })
            .collect();
        self.for_cells_in(lo, hi, reach, |hm, k, p| {
            let (d, _, seg) = geom::polyline_closest(&line, p);
            if d > reach {
                return;
            }
            let level = surface[seg.min(surface.len() - 1)];
            if d <= half_w + 2.0 && level > hm.heights[k] {
                hm.water[k] = level;
            }
            let wet = 1.0 - geom::smoothstep(half_w, reach, d);
            hm.wet[k] = hm.wet[k].max(wet);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::*;

    fn flat_map() -> MapDef {
        MapDef {
            name: "test".into(),
            description: String::new(),
            size: 200.0,
            cell: 2.0,
            base_height: 10.0,
            noise: NoiseDef {
                amplitude: 0.0,
                detail: 0.0,
                ..NoiseDef::default()
            },
            rim: RimDef {
                width: 20.0,
                height: 10.0,
            },
            fence_inset: 0.0,
            features: vec![],
            roads: vec![],
            water: vec![],
            forests: vec![],
            placements: vec![],
            markers: vec![],
            regions: vec![],
            haze: None,
            ash: 0.0,
            sounds: SoundsDef::default(),
        }
    }

    #[test]
    fn flat_map_is_flat_in_the_middle_and_rises_at_the_rim() {
        let hm = Heightmap::build(&flat_map());
        assert!((hm.height([0.0, 0.0]) - 10.0).abs() < 1e-4);
        assert!(hm.height([99.0, 0.0]) > 18.0);
        assert!(hm.slope_deg([0.0, 0.0]) < 0.1);
    }

    #[test]
    fn hills_rise_and_channels_cut() {
        let mut map = flat_map();
        map.features.push(TerrainFeature::Hill {
            at: [0.0, 0.0],
            radius: 30.0,
            height: 12.0,
        });
        map.features.push(TerrainFeature::Channel {
            points: vec![[-80.0, 40.0], [80.0, 40.0]],
            width: 10.0,
            depth: 3.0,
        });
        let hm = Heightmap::build(&map);
        assert!((hm.height([0.0, 0.0]) - 22.0).abs() < 0.1);
        assert!((hm.height([0.0, 40.0]) - 7.0).abs() < 0.1);
    }

    #[test]
    fn roads_flatten_a_bumpy_hillside_and_paint() {
        let mut map = flat_map();
        map.features.push(TerrainFeature::Hill {
            at: [0.0, 0.0],
            radius: 40.0,
            height: 8.0,
        });
        map.roads.push(RoadDef {
            name: "r".into(),
            kind: RoadKind::Gravel,
            points: vec![[-60.0, 0.0], [60.0, 0.0]],
            width: 6.0,
            shoulder: 3.0,
            smoothing: 20.0,
            bridges: vec![],
        });
        let hm = Heightmap::build(&map);
        // Cross-slope across the road is gentle.
        let a = hm.height([0.0, -2.0]);
        let b = hm.height([0.0, 2.0]);
        assert!((a - b).abs() < 0.2, "{a} {b}");
        assert_eq!(hm.surface([10.0, 0.0], false), Surface::Gravel);
        assert_eq!(hm.surface([0.0, 30.0], false), Surface::Grass);
    }

    #[test]
    fn water_fills_a_channel() {
        let mut map = flat_map();
        map.features.push(TerrainFeature::Channel {
            points: vec![[-80.0, 0.0], [80.0, 0.0]],
            width: 12.0,
            depth: 2.0,
        });
        map.water.push(WaterDef {
            name: "creek".into(),
            points: vec![[-80.0, 0.0], [80.0, 0.0]],
            width: 5.0,
            depth: 0.6,
        });
        let hm = Heightmap::build(&map);
        let lvl = hm.water_level([0.0, 0.0]).expect("water in the channel");
        assert!((lvl - 8.6).abs() < 0.2, "{lvl}");
        assert_eq!(hm.surface([0.0, 0.0], false), Surface::Water);
        assert!(hm.water_level([0.0, 30.0]).is_none());
        assert!(hm.wetness([0.0, 4.0]) > 0.5);
    }
}
