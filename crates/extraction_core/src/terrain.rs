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

/// A boxcar's bogie centres, metres either side of its middle.
pub const BOGIE: f32 = 4.5;

/// How far a wreck's tyres sit into the plane through the ground under
/// its four wheels (`map::prefab_wheels`; its model's tyres reach 2 cm
/// under its origin). The ground twists under four wheels, which can't
/// all lie in one plane: this leaves the low pair a few cm into it and the
/// high pair no more than a hair over, both inside 5 cm.
pub const WRECK_SEAT: f32 = 0.015;

/// Past a road's end the ground rises or falls no more than this in any
/// half metre: walkable, not a bank (the body steps 0.4 m).
pub const ROAD_END_LEDGE: f32 = 0.35;
/// How far past a road's edge its end blends into the land, metres.
pub const ROAD_END_APRON: f32 = 6.0;

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
    /// Cells left out of the ground (indexed by their low corner's
    /// sample): where a stair goes down through it (`underground`).
    pub holes: Vec<bool>,
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

/// A separable box blur of an `n`-by-`n` grid, radius `r` cells (edges
/// clamped).
fn box_blur(data: &[f32], n: usize, r: usize) -> Vec<f32> {
    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut out = vec![0.0; src.len()];
        for line in 0..n {
            let at = |i: usize| {
                if horizontal {
                    line * n + i
                } else {
                    i * n + line
                }
            };
            let mut sum = 0.0;
            for i in 0..=r.min(n - 1) {
                sum += src[at(i)];
            }
            sum += src[at(0)] * r as f32;
            for i in 0..n {
                out[at(i)] = sum / (2 * r + 1) as f32;
                let add = src[at((i + r + 1).min(n - 1))];
                let sub = src[at(i.saturating_sub(r))];
                sum += add - sub;
            }
        }
        out
    };
    pass(&pass(data, true), false)
}

/// Natural relief in about `-0.5..1`, times the map's amplitude: in
/// patches (a slow mask), knolls and hummocks a few tens of metres across,
/// with a ridged octave so their crests are sharper than their hollows.
fn relief(p: P2, seed: u32) -> f32 {
    let mask = geom::smoothstep(
        -0.15,
        0.35,
        noise::fbm(p[0] / 260.0, p[1] / 260.0, seed ^ 0x51F1, 2, 0.5),
    );
    // Warp the knolls so they don't line up on the noise grid.
    let wx = 12.0 * noise::perlin(p[0] / 70.0, p[1] / 70.0, seed ^ 0x7A11);
    let wz = 12.0 * noise::perlin(p[0] / 70.0 + 5.2, p[1] / 70.0 - 1.3, seed ^ 0x7A12);
    let q = [p[0] + wx, p[1] + wz];
    let knolls = noise::fbm(q[0] / 42.0, q[1] / 42.0, seed ^ 0x6B01, 2, 0.45);
    let ridged = 1.0 - noise::perlin(q[0] / 28.0, q[1] / 28.0, seed ^ 0x6B02).abs();
    mask * (0.75 * knolls + 0.5 * (ridged * ridged - 0.45))
}

impl Heightmap {
    pub fn build(map: &MapDef) -> Self {
        Self::build_with(map, true)
    }

    /// The land as the map shapes it, before `settle` fits the buildings
    /// in: what `map_lint` measures the settling against.
    pub fn build_unsettled(map: &MapDef) -> Self {
        Self::build_with(map, false)
    }

    fn build_with(map: &MapDef, settled: bool) -> Self {
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
            holes: vec![false; n * n],
        };
        let relief = hm.shape(map);
        hm.erode(map, &relief);
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
        if settled {
            hm.settle(map);
        }
        hm.underground(map);
        hm
    }

    /// Underground sites: the ground levelled under each exit kiosk to
    /// the height its layout wants (the entrance is a placement, padded as
    /// one), and opened up over every stairwell.
    fn underground(&mut self, map: &MapDef) {
        const BLEND: f32 = 8.0;
        let sites = crate::underground::placed(map);
        // The other buildings' pads stay as they were levelled: how much a
        // pad holds a point (1 on it, easing out over its blend). A kiosk's
        // 8 m blend reached the factory hall's back door at Cinder and
        // left a lip in the doorway.
        let pads: Vec<_> = map
            .placements
            .iter()
            .filter(|p| {
                !p.absolute
                    && crate::map::prefab_wants_pad(&p.prefab)
                    && !crate::underground::is_site(&p.prefab)
            })
            .map(|p| {
                let [w, _, d] = crate::map::prefab_footprint(&p.prefab);
                (p.at, p.rot, w * 0.5 + 0.8, d * 0.5 + 0.8)
            })
            .collect();
        let held = |q: P2| {
            pads.iter().fold(0.0f32, |h, (at, rot, hw, hd)| {
                let l = crate::underground::turn([q[0] - at[0], q[1] - at[1]], -rot);
                let out = ((l[0].abs() - hw).max(0.0)).hypot((l[1].abs() - hd).max(0.0));
                h.max(1.0 - geom::smoothstep(0.0, 5.0, out))
            })
        };
        for site in &sites {
            let footing = self.footing(&site.id, site.at, site.rot);
            for (at, rot, portal) in site.exits() {
                let target = footing + portal.y;
                let [w, _, d] = crate::underground::FOOTPRINT;
                let (half_w, half_d) = (w * 0.5 + 0.8, d * 0.5 + 0.8);
                // Blended out over 8 m: a pad raised or cut 1.5 m into a
                // hillside left a bank too steep to walk up over 5.
                let reach = half_w.max(half_d) + BLEND + 1.0;
                let lo = [at[0] - reach, at[1] - reach];
                let hi = [at[0] + reach, at[1] + reach];
                self.for_cells_in(lo, hi, 0.0, |hm, k, q| {
                    let l = crate::underground::turn([q[0] - at[0], q[1] - at[1]], -rot);
                    let out =
                        ((l[0].abs() - half_w).max(0.0)).hypot((l[1].abs() - half_d).max(0.0));
                    let mut t = 1.0 - geom::smoothstep(0.0, BLEND, out);
                    if out > 0.0 {
                        t *= 1.0 - held(q);
                    }
                    hm.heights[k] += (target - hm.heights[k]) * t;
                });
            }
        }
        if sites.is_empty() {
            return;
        }
        for j in 0..self.n - 1 {
            for i in 0..self.n - 1 {
                let p = self.pos(i, j);
                let centre = [p[0] + self.cell * 0.5, p[1] + self.cell * 0.5];
                if crate::underground::in_stairwell(&sites, centre) {
                    let k = self.idx(i, j);
                    self.holes[k] = true;
                }
            }
        }
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

    /// The land from the noise and features; returns the natural relief's
    /// share of each cell, which `erode` takes back off round buildings.
    fn shape(&mut self, map: &MapDef) -> Vec<f32> {
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
        // A ridge or channel reaches half its width from its line.
        let near: Vec<(geom::NearLine, &TerrainFeature)> = splines
            .iter()
            .map(|(line, f)| {
                let reach = match f {
                    TerrainFeature::Ridge { width, .. } | TerrainFeature::Channel { width, .. } => {
                        *width * 0.5
                    }
                    _ => 0.0,
                };
                (geom::NearLine::new(line, reach), *f)
            })
            .collect();
        // A cell's rise and its share of relief: from where it is alone.
        let cell = |p: P2| -> (f32, f32) {
            let mut h = 0.0;
            let mut relief_h = 0.0;
            if nd.amplitude != 0.0 {
                h += nd.amplitude
                    * noise::fbm(p[0] / nd.scale, p[1] / nd.scale, nd.seed, nd.octaves, 0.5);
            }
            if nd.relief != 0.0 && nd.amplitude != 0.0 {
                relief_h = nd.relief * nd.amplitude * relief(p, nd.seed);
                h += relief_h;
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
            for (line, f) in &near {
                // Beyond the reach, it adds nothing.
                let Some((d, _, _)) = line.closest(p) else {
                    continue;
                };
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
            (h, relief_h)
        };
        // Rows on every core: each cell is its own (and the same sums in the
        // same order as one thread would do them). Most of a valley's load.
        let n = self.n;
        let mut out = vec![(0.0f32, 0.0f32); n * n];
        let threads = std::thread::available_parallelism().map_or(1, |t| t.get());
        let rows = n.div_ceil(threads.max(1));
        let (origin, step) = (self.origin, self.cell);
        std::thread::scope(|scope| {
            for (c, chunk) in out.chunks_mut(rows * n).enumerate() {
                let cell = &cell;
                scope.spawn(move || {
                    for (k, v) in chunk.iter_mut().enumerate() {
                        let at = c * rows * n + k;
                        let (i, j) = (at % n, at / n);
                        *v = cell([origin + i as f32 * step, origin + j as f32 * step]);
                    }
                });
            }
        });
        let mut relief_h = vec![0.0; n * n];
        for (k, (h, r)) in out.into_iter().enumerate() {
            self.heights[k] += h;
            relief_h[k] = r;
        }
        relief_h
    }

    /// Rain on the land: droplets run downhill picking up soil where they
    /// speed up and dropping it where they slow, which cuts gullies down
    /// the slopes and fans silt out at their feet. Deterministic per map.
    fn erode(&mut self, map: &MapDef, relief: &[f32]) {
        let nd = &map.noise;
        let before = self.heights.clone();
        if nd.erosion > 0.0 {
            self.rain(map);
            // Rain cuts crisp: soften what it changed (two box blurs, about
            // a Gaussian of 2 m), so gullies are worn hollows with rounded
            // shoulders rather than ditches with rims.
            let n = self.n;
            let mut delta: Vec<f32> = self
                .heights
                .iter()
                .zip(&before)
                .map(|(h, b)| h - b)
                .collect();
            let r = (2.0 / self.cell).round().max(1.0) as usize;
            for _ in 0..2 {
                delta = box_blur(&delta, n, r);
            }
            for (k, h) in self.heights.iter_mut().enumerate() {
                *h = before[k] + delta[k];
            }
        }
        // Where people built, the ground was settled: the land the map was
        // laid out on (no knolls, no gullies) round the buildings. Not
        // round a car wreck: it was left on the verge as the verge was.
        for pl in map
            .placements
            .iter()
            .filter(|p| !crate::map_lint::WRECKS.contains(&p.prefab.as_str()))
        {
            let r = crate::map::prefab_clearance(&pl.prefab);
            let lo = [pl.at[0] - r - 8.0, pl.at[1] - r - 8.0];
            let hi = [pl.at[0] + r + 8.0, pl.at[1] + r + 8.0];
            self.for_cells_in(lo, hi, 0.0, |hm, k, p| {
                let t = 1.0 - geom::smoothstep(r, r + 8.0, geom::dist(p, pl.at));
                hm.heights[k] += (before[k] - relief[k] - hm.heights[k]) * t;
            });
        }
        // And along the streams, whose levels and fords the map set.
        for w in &map.water {
            let line = geom::catmull_rom(&w.points, 2.0);
            if line.is_empty() {
                continue;
            }
            let (inner, outer) = (w.width * 0.5 + 8.0, w.width * 0.5 + 16.0);
            let (lo, hi) = geom::bounds(&line);
            let near = geom::NearLine::new(&line, outer);
            self.for_cells_in(lo, hi, outer, |hm, k, p| {
                let Some((d, _, _)) = near.closest(p) else {
                    return;
                };
                if d < outer {
                    let t = 1.0 - geom::smoothstep(inner, outer, d);
                    hm.heights[k] += (before[k] - relief[k] - hm.heights[k]) * t;
                }
            });
        }
    }

    fn rain(&mut self, map: &MapDef) {
        let nd = &map.noise;
        let n = self.n;
        let cell = self.cell;
        // Droplets per square metre of map.
        let area = (n as f32 * cell).powi(2);
        let drops = (area * 0.12 * nd.erosion) as usize;
        let mut rng = noise::Rng::new(u64::from(nd.seed) ^ 0x0E20_5104);
        // A small brush so a gully is a few metres wide, not a crack.
        let brush: Vec<(i32, i32, f32)> = {
            let r = 2.2 / cell;
            let ri = r.ceil() as i32;
            let mut b = Vec::new();
            for dj in -ri..=ri {
                for di in -ri..=ri {
                    let d = ((di * di + dj * dj) as f32).sqrt();
                    if d < r {
                        b.push((di, dj, r - d));
                    }
                }
            }
            let sum: f32 = b.iter().map(|w| w.2).sum();
            b.into_iter().map(|(i, j, w)| (i, j, w / sum)).collect()
        };
        const INERTIA: f32 = 0.08;
        const CAPACITY: f32 = 3.0;
        const DEPOSIT: f32 = 0.25;
        const ERODE: f32 = 0.2;
        const EVAPORATE: f32 = 0.025;
        const GRAVITY: f32 = 9.0;
        const MIN_SLOPE: f32 = 0.004;
        // At most this much off any one point of a drop's path, metres.
        const BITE: f32 = 0.08;
        let lim = (n - 2) as f32;
        let sample = |h: &[f32], x: f32, y: f32| {
            let (i, j) = (x as usize, y as usize);
            let (fx, fy) = (x - i as f32, y - j as f32);
            let k = j * n + i;
            let (a, b, c, d) = (h[k], h[k + 1], h[k + n], h[k + n + 1]);
            let gx = (b - a) * (1.0 - fy) + (d - c) * fy;
            let gy = (c - a) * (1.0 - fx) + (d - b) * fx;
            let height = a * (1.0 - fx) * (1.0 - fy)
                + b * fx * (1.0 - fy)
                + c * (1.0 - fx) * fy
                + d * fx * fy;
            (height, gx / cell, gy / cell)
        };
        for _ in 0..drops {
            let (mut x, mut y) = (rng.range(2.0, lim - 1.0), rng.range(2.0, lim - 1.0));
            let (mut dx, mut dy) = (0.0f32, 0.0f32);
            let (mut speed, mut water, mut sediment) = (1.0f32, 1.0f32, 0.0f32);
            for _ in 0..60 {
                let (i, j) = (x as usize, y as usize);
                let (fx, fy) = (x - i as f32, y - j as f32);
                let (h, gx, gy) = sample(&self.heights, x, y);
                dx = dx * INERTIA - gx * (1.0 - INERTIA);
                dy = dy * INERTIA - gy * (1.0 - INERTIA);
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1e-6 {
                    break;
                }
                dx /= len;
                dy /= len;
                x += dx;
                y += dy;
                if x < 2.0 || y < 2.0 || x >= lim - 1.0 || y >= lim - 1.0 {
                    break;
                }
                let dh = sample(&self.heights, x, y).0 - h;
                let capacity = (-dh / cell).max(MIN_SLOPE) * speed * water * CAPACITY;
                if sediment > capacity || dh > 0.0 {
                    // Drop some: fill the pit it ran into, or shed the excess.
                    let drop = if dh > 0.0 {
                        dh.min(sediment)
                    } else {
                        (sediment - capacity) * DEPOSIT
                    };
                    sediment -= drop;
                    let k = j * n + i;
                    self.heights[k] += drop * (1.0 - fx) * (1.0 - fy);
                    self.heights[k + 1] += drop * fx * (1.0 - fy);
                    self.heights[k + n] += drop * (1.0 - fx) * fy;
                    self.heights[k + n + 1] += drop * fx * fy;
                } else {
                    let take = ((capacity - sediment) * ERODE).min(-dh).min(BITE);
                    for (di, dj, w) in &brush {
                        let (bi, bj) = (i as i32 + di, j as i32 + dj);
                        if bi < 0 || bj < 0 || bi >= n as i32 || bj >= n as i32 {
                            continue;
                        }
                        let k = bj as usize * n + bi as usize;
                        self.heights[k] -= take * w;
                    }
                    sediment += take;
                }
                speed = (speed * speed - dh * GRAVITY).max(0.0).sqrt();
                water *= 1.0 - EVAPORATE;
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
        if let Some((a, b)) = self.bogies(prefab, at, rot_deg) {
            return (a + b) * 0.5;
        }
        if let Some(h) = self.wheels(prefab, at, rot_deg) {
            return h.iter().sum::<f32>() * 0.25 + 0.02 - WRECK_SEAT;
        }
        // The funicular's car is built on the grade through its middle.
        if prefab == "funicular_car" {
            return self.height(at);
        }
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

    /// Rolling stock stands on its bogies, on the rail's grade: the ground
    /// under the rear and front bogie centres (along the car's local Z).
    fn bogies(&self, prefab: &str, at: P2, rot_deg: f32) -> Option<(f32, f32)> {
        if prefab != "boxcar" {
            return None;
        }
        let (s, c) = rot_deg.to_radians().sin_cos();
        let h = |lz: f32| self.height([at[0] - lz * s, at[1] + lz * c]);
        Some((h(-BOGIE), h(BOGIE)))
    }

    /// The ground under a wreck's four wheels (`map::prefab_wheels`'
    /// order), where the game's Quat::from_rotation_y(-rot) puts them.
    fn wheels(&self, prefab: &str, at: P2, rot_deg: f32) -> Option<[f32; 4]> {
        let w = crate::map::prefab_wheels(prefab)?;
        let (s, c) = rot_deg.to_radians().sin_cos();
        Some(w.map(|[lx, lz]| self.height([at[0] + lx * c - lz * s, at[1] + lx * s + lz * c])))
    }

    /// How far a placement pitches nose-up about its local X, radians:
    /// rolling stock follows the grade, a wreck the ground under its
    /// axles; buildings stand level.
    pub fn pitch(&self, prefab: &str, at: P2, rot_deg: f32) -> f32 {
        if let (Some(h), Some(w)) = (
            self.wheels(prefab, at, rot_deg),
            crate::map::prefab_wheels(prefab),
        ) {
            return ((h[2] + h[3] - h[0] - h[1]) * 0.5).atan2(w[2][1] - w[0][1]);
        }
        self.bogies(prefab, at, rot_deg)
            .map_or(0.0, |(a, b)| (b - a).atan2(2.0 * BOGIE))
    }

    /// How far a placement rolls about its local Z, radians (its +X side
    /// up): a wreck takes the ground's fall across its track; everything
    /// else stands level.
    pub fn roll(&self, prefab: &str, at: P2, rot_deg: f32) -> f32 {
        let (Some(h), Some(w)) = (
            self.wheels(prefab, at, rot_deg),
            crate::map::prefab_wheels(prefab),
        ) else {
            return 0.0;
        };
        let track = (w[1][0] - w[0][0] + w[3][0] - w[2][0]) * 0.5;
        ((h[1] + h[3] - h[0] - h[2]) * 0.5).atan2(track)
    }

    /// Whether `p` is on the declared ground `poly`, its edge too.
    pub fn on_terrace(poly: &[P2], p: P2) -> bool {
        geom::point_in_polygon(poly, p) || geom::polygon_edge_distance(poly, p) < 1e-3
    }

    /// The ground a map declares: each plateau it sets to a height (a
    /// terrace, a ledge, a dam's crest), as polygon and level.
    pub fn terraces(map: &MapDef) -> impl Iterator<Item = (&[P2], f32)> {
        map.features.iter().filter_map(|f| match f {
            TerrainFeature::Plateau {
                polygon,
                height: Some(h),
                ..
            } => Some((polygon.as_slice(), *h)),
            _ => None,
        })
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
        let on_terrace = Self::on_terrace;
        let terraces: Vec<(&[P2], f32, (P2, P2))> = Self::terraces(map)
            .map(|(poly, level)| (poly, level, geom::bounds(poly)))
            .collect();
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
            // The terraces the map declares round it that it doesn't stand
            // on: past its walls it digs none below its level (a house below
            // a terraced street dug the steps above it away); what a road
            // banked on top it may still trim.
            let others: Vec<(&[P2], f32)> = terraces
                .iter()
                .filter(|(poly, _, (a, b))| {
                    a[0] <= hi[0]
                        && b[0] >= lo[0]
                        && a[1] <= hi[1]
                        && b[1] >= lo[1]
                        && !geom::point_in_polygon(poly, p.at)
                })
                .map(|(poly, level, _)| (*poly, *level))
                .collect();
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
                    let floor = if inside {
                        f32::NEG_INFINITY
                    } else {
                        others
                            .iter()
                            .filter(|(poly, _)| on_terrace(poly, q))
                            .map(|(_, level)| *level)
                            .fold(f32::NEG_INFINITY, f32::max)
                    };
                    // A smooth minimum past the flat, so the cut meets the
                    // hillside in a rounded shoulder, not a straight crease
                    // (it only ever lowers, and never under the walls).
                    let (a, b) = (hm.heights[k], y + out * RAMP);
                    let soft = 2.0 * geom::smoothstep(0.5, 3.0, out);
                    let lower = if soft > 1e-3 {
                        let t = ((soft - (a - b).abs()) / soft).max(0.0);
                        a.min(b) - t * t * soft * 0.25
                    } else {
                        a.min(b)
                    };
                    hm.heights[k] = lower.max(floor.min(a));
                });
                if footing(self) >= y - 1e-3 {
                    break;
                }
            }
        }
        // A building stepped down its slope (the ore mill) stands where the
        // map put it, on terraces the map cut for it, and isn't padded; but
        // each terrace's blend ran out over the bay below it and up through
        // that bay's floor, a metre high at the footing (bots pinned against
        // it, short of the loot along the back wall). Under each bay the
        // ground goes no higher than its floor; a cell either side of the
        // footing between two bays, no higher than the lower one's, so the
        // slope between samples doesn't rise through it either.
        for p in &map.placements {
            if crate::map::stepped_floor_level(&p.prefab, [0.0, 0.0]).is_none() {
                continue;
            }
            let base = if p.absolute {
                p.y
            } else {
                self.height(p.at) + p.y
            };
            let [w, _, d] = crate::map::prefab_footprint(&p.prefab);
            let (s, c) = (-p.rot.to_radians()).sin_cos();
            let reach = (w * 0.5).hypot(d * 0.5) + self.cell;
            let lo = [p.at[0] - reach, p.at[1] - reach];
            let hi = [p.at[0] + reach, p.at[1] + reach];
            let cell = self.cell;
            self.for_cells_in(lo, hi, 0.0, |hm, k, q| {
                let (dx, dz) = (q[0] - p.at[0], q[1] - p.at[1]);
                let l = [dx * c - dz * s, dx * s + dz * c];
                let level = [-cell, 0.0, cell]
                    .iter()
                    .flat_map(|ox| [-cell, 0.0, cell].map(|oz| [l[0] + ox, l[1] + oz]))
                    .filter_map(|q| crate::map::stepped_floor_level(&p.prefab, q))
                    .fold(f32::INFINITY, f32::min);
                if level.is_finite() {
                    hm.heights[k] = hm.heights[k].min(base + level);
                }
            });
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
                // Where the window is cut short (near an end), a plain
                // average leans to the side it has: on a slope the road
                // would end in a cut or on a fill. A straight line fitted
                // to the same samples, read off here, doesn't lean.
                // (A rail bed keeps its own grade to the buffers.)
                let short = road.kind != RoadKind::Rail
                    && ((i - win) < 0 || (i + win) as usize > raw.len() - 1);
                let (mut n, mut sx, mut sy, mut sxx, mut sxy) = (0.0f32, 0.0, 0.0, 0.0, 0.0);
                for k in a..=b {
                    if !on_bridge(arc[k]) {
                        let x = arc[k] - arc[i as usize];
                        n += 1.0;
                        sx += x;
                        sy += raw[k];
                        sxx += x * x;
                        sxy += x * raw[k];
                    }
                }
                if n == 0.0 {
                    return raw[i as usize];
                }
                let det = n * sxx - sx * sx;
                if short && det.abs() > 1e-3 {
                    // The fitted line's height at x = 0.
                    (sy * sxx - sx * sxy) / det
                } else {
                    sy / n
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
        // Where the road stops you walk on: its cut or fill blends back
        // into the land over an apron, not over the shoulder, which on a
        // slope leaves a bank (or a drop) a body can't step past.
        let last = line.len() - 1;
        let apron = |s: usize| (s == 0 || s == last) && !on_bridge(arc[s]);
        let end_reach = half_w + road.shoulder.max(ROAD_END_APRON);
        // Nearest centreline sample per touched cell.
        let mut best: std::collections::HashMap<usize, (f32, usize)> = Default::default();
        for (s, p) in line.iter().enumerate() {
            let reach = if apron(s) { end_reach } else { reach };
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
            let outer = if apron(s) { end_reach } else { reach };
            let t = 1.0 - geom::smoothstep(half_w, outer, d);
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
        let near = geom::NearLine::new(&line, reach);
        self.for_cells_in(lo, hi, reach, |hm, k, p| {
            let Some((d, _, seg)) = near.closest(p) else {
                return;
            };
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

    /// A car-space point where the game places it: Ry(-rot) after
    /// Rx(-pitch) after Rz(roll), from the footing (`placement_transform`).
    fn posed(hm: &Heightmap, p: &Placement, local: [f32; 3]) -> [f32; 3] {
        let (pitch, roll) = (
            hm.pitch(&p.prefab, p.at, p.rot),
            hm.roll(&p.prefab, p.at, p.rot),
        );
        let [x, y, z] = local;
        let (sr, cr) = roll.sin_cos();
        let (x, y) = (x * cr - y * sr, x * sr + y * cr);
        let (sp, cp) = (-pitch).sin_cos();
        let (y, z) = (y * cp - z * sp, y * sp + z * cp);
        let (sy, cy) = (-p.rot.to_radians()).sin_cos();
        let (x, z) = (x * cy + z * sy, -x * sy + z * cy);
        let y0 = hm.footing(&p.prefab, p.at, p.rot) + p.y;
        [p.at[0] + x, y0 + y, p.at[1] + z]
    }

    /// Every wreck in every valley stands on its four wheels: each tyre's
    /// bottom within 5 cm of the ground under it, none in the air.
    #[test]
    fn wrecks_stand_on_their_wheels() {
        let mut bad = Vec::new();
        let mut n = 0;
        let yard = [("training_yard", TRAINING_YARD)];
        for (name, text) in VALLEYS.into_iter().chain(yard) {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            for p in &map.placements {
                let Some(w) = prefab_wheels(&p.prefab) else {
                    continue;
                };
                n += 1;
                for [x, z] in w {
                    let q = posed(&hm, p, [x, -0.02, z]);
                    let off = q[1] - hm.height([q[0], q[2]]);
                    if off.abs() > 0.05 {
                        bad.push(format!(
                            "{name} {} at ({:.1}, {:.1}): wheel ({x}, {z}) {off:+.3} m",
                            p.prefab, p.at[0], p.at[1]
                        ));
                    }
                }
            }
        }
        println!("{n} wrecks");
        assert!(bad.is_empty(), "wrecks off their wheels: {bad:#?}");
    }

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
                relief: 0.0,
                erosion: 0.0,
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
            cold: 0.0,
            scree: 0.0,
            snow_line: None,
            snow: 0.0,
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

    /// A house below a terrace the map sets keeps its own footing, and
    /// the ramp settling it in stops at the terrace's level: it used to
    /// dig the step above it away, metres deep. (It stands past its pad's
    /// blend from the terrace: the ramp reaches further.)
    #[test]
    fn settling_a_house_leaves_the_terrace_above_it_be() {
        let mut map = flat_map();
        map.features.push(TerrainFeature::Plateau {
            polygon: vec![[0.0, -20.0], [30.0, -20.0], [30.0, 20.0], [0.0, 20.0]],
            height: Some(16.0),
            blend: 2.0,
        });
        map.placements.push(Placement {
            prefab: "house_brick".into(),
            at: [-11.5, 0.0],
            rot: 0.0,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        });
        let hm = Heightmap::build(&map);
        assert!(hm.footing("house_brick", [-11.5, 0.0], 0.0) < 11.0);
        for x in [0.0, 0.5, 2.0, 3.0] {
            let h = hm.height([x, 0.0]);
            assert!(h > 15.5, "the terrace at x {x} is cut to {h}");
        }
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
            galleries: vec![],
        });
        let hm = Heightmap::build(&map);
        // Cross-slope across the road is gentle.
        let a = hm.height([0.0, -2.0]);
        let b = hm.height([0.0, 2.0]);
        assert!((a - b).abs() < 0.2, "{a} {b}");
        assert_eq!(hm.surface([10.0, 0.0], false), Surface::Gravel);
        assert_eq!(hm.surface([0.0, 30.0], false), Surface::Grass);
    }

    /// Where a road stops partway up (or down) a hillside you walk on, and
    /// the ground past its end must not stand up as the bank its grading
    /// cut (or drop off its fill): nothing steeper than a step in any half
    /// metre (the body steps 0.4 m; this keeps a margin).
    #[test]
    fn roads_end_where_you_can_walk_on() {
        let mut map = flat_map();
        map.features.push(TerrainFeature::Hill {
            at: [0.0, 0.0],
            radius: 40.0,
            height: 10.0,
        });
        // Up the hill from the west and the north, down it to the east.
        for (points, kind, width, shoulder, smoothing) in [
            (
                vec![[-80.0, 0.0], [-15.0, 0.0]],
                RoadKind::Gravel,
                4.0,
                3.0,
                20.0,
            ),
            (
                vec![[0.0, -80.0], [0.0, -18.0]],
                RoadKind::Path,
                1.6,
                1.0,
                4.0,
            ),
            (
                vec![[8.0, 30.0], [30.0, 30.0]],
                RoadKind::Dirt,
                3.5,
                3.0,
                10.0,
            ),
        ] {
            map.roads.push(RoadDef {
                name: format!("{kind:?}"),
                kind,
                points,
                width,
                shoulder,
                smoothing,
                bridges: vec![],
                galleries: vec![],
            });
        }
        let hm = Heightmap::build(&map);
        for rl in &hm.road_lines {
            for (end, inward) in [(0, 1), (rl.points.len() - 1, rl.points.len() - 2)] {
                let p = rl.points[end];
                let d = geom::sub(p, rl.points[inward]);
                let d = geom::scale(d, 1.0 / geom::len(d));
                let at = |m: f32| geom::add(p, geom::scale(d, m));
                let ledge = (1..=16)
                    .map(|k| {
                        let (a, b) = (at((k - 1) as f32 * 0.5), at(k as f32 * 0.5));
                        (hm.height(b) - hm.height(a)).abs()
                    })
                    .fold(0.0, f32::max);
                assert!(
                    ledge <= ROAD_END_LEDGE,
                    "{:?} road ends at ({:.0}, {:.0}) below a {ledge:.2} m ledge",
                    rl.kind,
                    p[0],
                    p[1]
                );
            }
        }
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

    #[test]
    fn rain_wears_the_land_but_not_round_buildings() {
        let mut map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let worn = Heightmap::build(&map);
        map.noise.erosion = 0.0;
        map.noise.relief = 0.0;
        let plain = Heightmap::build(&map);
        let d: Vec<f32> = worn
            .heights
            .iter()
            .zip(&plain.heights)
            .map(|(a, b)| a - b)
            .collect();
        assert!(d.iter().all(|x| x.is_finite()));
        let rms = (d.iter().map(|x| x * x).sum::<f32>() / d.len() as f32).sqrt();
        let most = d.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(rms > 0.15 && most < 6.0, "rms {rms}, most {most}");
        // Round buildings the ground is the map's own. (Not quite to the
        // centimetre: a plateau or a road's grade averages the land round
        // it, worn or not, and a road's end follows the slope it ends on;
        // the logging road ends in Marrow Creek's sawmill, a few cm.)
        let mut moved: Vec<(f32, &str)> = map
            .placements
            .iter()
            .map(|p| {
                (
                    (worn.height(p.at) - plain.height(p.at)).abs(),
                    p.prefab.as_str(),
                )
            })
            .collect();
        moved.sort_by(|a, b| a.0.total_cmp(&b.0));
        let median = moved[moved.len() / 2].0;
        assert!(
            median < 0.04 && moved.iter().all(|m| m.0 < 0.5),
            "moved: {moved:?}"
        );
    }

    /// Writes each valley's heights (little-endian f32, row by row) to
    /// `$DUMP/<name>_<n>.f32`, for a hillshade: `DUMP=dir cargo test
    /// --release dump_heights -- --ignored`.
    #[test]
    #[ignore]
    fn dump_heights() {
        let out = std::env::var("DUMP").unwrap();
        for (name, src) in [
            ("marrow", crate::map::MARROW_CREEK),
            ("kettle", crate::map::KETTLE_PASS),
            ("cinder", crate::map::CINDER_WORKS),
        ] {
            let map = MapDef::from_ron(src).unwrap();
            let t = std::time::Instant::now();
            let hm = Heightmap::build(&map);
            println!("{name}: {:?}", t.elapsed());
            let bytes: Vec<u8> = hm.heights.iter().flat_map(|h| h.to_le_bytes()).collect();
            std::fs::write(format!("{out}/{name}_{}.f32", hm.n), bytes).unwrap();
        }
    }
}
