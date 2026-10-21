//! Roadworks: what a road carries besides its bridges (`bridges`).
//!
//! - Avalanche galleries (`RoadDef::galleries`): over a span of road, a
//!   battered concrete retaining wall on the uphill side, a roof slab from
//!   it over the road, falling toward the valley onto a row of columns
//!   past the downhill edge, a snow-shedding upturn at the lip, an apron
//!   back over the wall for the slope to slide onto, and a thicker portal
//!   at each end. Snow lies on the roof, ice hangs under the lip. The wall
//!   stands on the cut bank past the shoulder and the columns past the
//!   road's edge, so the road keeps its full width and the nav grid is
//!   untouched. Solid (the game's: you shelter under it, climb onto it);
//!   seen in the client.
//! - The funicular: along the road named exactly [`FUNICULAR`] (a footpath
//!   laid straight up a slope), a metre-gauge track on the path's left
//!   half looking uphill (the west half of a north-running line), with a
//!   passing loop at mid-length and a hauling rope on rollers, and a
//!   service stair with a handrail on the other half. Seen only (you walk
//!   the graded path); built by the client, in chunks of merged meshes.
//!
//! The geometry is worked out by plain functions over the map and its
//! heightmap ([`gallery_plans`], [`galleries`], [`funicular`]) so tests
//! can check it without rendering.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::geom;
use extraction_core::map::MapDef;
use extraction_core::terrain::{Heightmap, RoadLine};

use super::bridges::Piece;
use super::{Layer, Zone};
use crate::render::materials::Mat;

/// The name of the road that gets the funicular's track and stair.
pub const FUNICULAR: &str = "Funicular";

/// Logic half: the galleries, solid.
pub struct RoadworksPlugin;

impl Plugin for RoadworksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_galleries);
    }
}

/// Visual half: the galleries seen, and the funicular laid.
pub struct RoadworksVisualsPlugin;

impl Plugin for RoadworksVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, lay_funicular)
            .add_systems(Update, gallery_visuals);
    }
}

/// What a surface looks like: a library material, or one of the two this
/// module makes itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Look {
    Mat(Mat),
    /// Lying snow: matt off-white.
    Snow,
    /// Icicles: pale, glossy.
    Ice,
}

// ---------------------------------------------------------------------------
// A road's centreline as built.

/// A point on a road's graded centreline: where (on the grade), which way
/// (horizontal, unit) and its left (horizontal, unit).
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub s: f32,
    pub at: Vec3,
    pub dir: Vec3,
    pub left: Vec3,
}

/// A road line read by arc length.
pub struct Line<'a> {
    pub rl: &'a RoadLine,
    arc: Vec<f32>,
}

impl<'a> Line<'a> {
    pub fn new(rl: &'a RoadLine) -> Self {
        let mut arc = vec![0.0f32; rl.points.len()];
        for i in 1..rl.points.len() {
            arc[i] = arc[i - 1] + geom::dist(rl.points[i - 1], rl.points[i]);
        }
        Self { rl, arc }
    }

    pub fn len(&self) -> f32 {
        self.arc.last().copied().unwrap_or(0.0)
    }

    /// The graded height at arc length `s` (the samples aren't exactly a
    /// metre apart: interpolate by arc, not by index).
    pub fn graded(&self, s: f32) -> f32 {
        let arc = &self.arc;
        if arc.len() < 2 {
            return self.rl.heights.first().copied().unwrap_or(0.0);
        }
        let k = arc.partition_point(|a| *a <= s).clamp(1, arc.len() - 1);
        let (a0, a1) = (arc[k - 1], arc[k]);
        let t = ((s - a0) / (a1 - a0).max(1e-4)).clamp(0.0, 1.0);
        self.rl.heights[k - 1] + (self.rl.heights[k] - self.rl.heights[k - 1]) * t
    }

    pub fn station(&self, s: f32) -> Station {
        let s = s.clamp(0.0, self.len());
        let (p, d) = geom::polyline_at(&self.rl.points, s);
        let d = Vec2::from_array(d).normalize_or(Vec2::Y);
        let dir = Vec3::new(d.x, 0.0, d.y);
        Station {
            s,
            at: Vec3::new(p[0], self.graded(s), p[1]),
            dir,
            left: Vec3::Y.cross(dir),
        }
    }

    /// Stations from `a` to `b` inclusive, evenly, no more than `step`
    /// apart.
    pub fn stations(&self, a: f32, b: f32, step: f32) -> Vec<Station> {
        let n = ((b - a) / step).ceil().max(1.0) as usize;
        (0..=n)
            .map(|i| self.station(a + (b - a) * i as f32 / n as f32))
            .collect()
    }
}

/// A deterministic 0..1 from two integers.
fn hash01(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

// ---------------------------------------------------------------------------
// Avalanche galleries.

/// Roof bays along the road, at most this long (m): the roof and wall
/// follow the road's curve and grade bay by bay.
const BAY: f32 = 3.5;
/// How far out either side of the road the ground is compared to find the
/// uphill side (m).
const SNIFF: f32 = 8.0;
/// Clear headroom under the roof at the road's downhill edge (m).
const HEAD: f32 = 5.5;
/// The roof's fall toward the valley (rise over run).
const FALL: f32 = 0.05;
/// Roof slab thickness (m).
const ROOF: f32 = 0.5;
/// How far the wall's footing goes below the grade (m).
const FOOT: f32 = 0.8;
/// The wall face's batter (run over rise: it leans back into the bank).
const BATTER: f32 = 0.1;
/// The roof's apron back over the wall (m).
const APRON: f32 = 2.0;
/// Column side (m) and spacing along the road (m).
const COL: f32 = 0.45;
const COL_EVERY: f32 = 6.0;
/// The edge beam the roof rests on over the columns: depth (m).
const BEAM: f32 = 0.45;
/// A portal's length along the road (m).
const PORTAL: f32 = 0.8;

/// One gallery's place on its road.
#[derive(Clone, Debug)]
pub struct GalleryPlan {
    /// Index into `MapDef::roads`.
    pub road: usize,
    /// Arc-length span on the road line.
    pub span: (f32, f32),
    pub width: f32,
    /// Which side is uphill: +1 the road's left (looking along its points'
    /// order), -1 its right.
    pub uphill: f32,
    /// The bays' ends along the road.
    pub stations: Vec<Station>,
}

impl GalleryPlan {
    /// Horizontal unit vector from the road toward the uphill side here.
    pub fn out(&self, st: &Station) -> Vec3 {
        st.left * self.uphill
    }
}

/// Every gallery on the map: where, and which side is uphill.
pub fn gallery_plans(map: &MapDef, hm: &Heightmap) -> Vec<GalleryPlan> {
    let mut out = Vec::new();
    for rl in &hm.road_lines {
        let Some(def) = map.roads.get(rl.road) else {
            continue;
        };
        if def.galleries.is_empty() {
            continue;
        }
        let line = Line::new(rl);
        for g in &def.galleries {
            let (_, s, _) = geom::polyline_closest(&rl.points, g.at);
            let a = (s - g.length * 0.5).max(0.5);
            let b = (s + g.length * 0.5).min(line.len() - 0.5);
            if b - a < 2.0 * PORTAL + 1.0 {
                continue;
            }
            let stations = line.stations(a, b, BAY);
            let lean: f32 = stations
                .iter()
                .map(|st| {
                    let o = st.left * SNIFF;
                    hm.height([st.at.x + o.x, st.at.z + o.z])
                        - hm.height([st.at.x - o.x, st.at.z - o.z])
                })
                .sum();
            out.push(GalleryPlan {
                road: rl.road,
                span: (a, b),
                width: rl.width,
                uphill: if lean >= 0.0 { 1.0 } else { -1.0 },
                stations,
            });
        }
    }
    out
}

/// The gallery's cross-section, in (u, v): u from the centreline toward
/// the uphill side, v above the grade.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub hw: f32,
}

impl Section {
    /// The roof's underside.
    pub fn under(&self, u: f32) -> f32 {
        HEAD + FALL * (u + self.hw)
    }
    pub fn top(&self, u: f32) -> f32 {
        self.under(u) + ROOF
    }
    /// The wall's face at its foot (shoulder edge).
    pub fn face(&self) -> f32 {
        self.hw + 0.6
    }
    /// The wall's face at its top.
    pub fn face_top(&self) -> f32 {
        self.face() + BATTER * (self.under(self.face()) + FOOT)
    }
    pub fn back_foot(&self) -> f32 {
        self.face() + 1.1
    }
    pub fn back_top(&self) -> f32 {
        self.face_top() + 0.6
    }
    /// The roof's uphill end (the apron's edge).
    pub fn back(&self) -> f32 {
        self.back_top() + APRON
    }
    /// The roof's outer lip, about a metre past the downhill edge.
    pub fn lip(&self) -> f32 {
        -(self.hw + 1.3)
    }
    /// The columns' centre line: clear of the road's width by 0.3 m.
    pub fn col(&self) -> f32 {
        -(self.hw + 0.3 + COL * 0.5)
    }
}

/// A profile swept along stations: per station a slab-form cross-section
/// (columns of bottom and top points, left to right), so concave tops
/// (snow) still cap cleanly.
#[derive(Clone, Debug)]
pub struct Sweep {
    pub look: Look,
    pub solid: bool,
    /// Per station: (origin on the grade, horizontal unit toward uphill).
    pub frames: Vec<(Vec3, Vec3)>,
    /// Per station: the closed ring, bottoms left to right then tops right
    /// to left.
    pub rings: Vec<Vec<Vec2>>,
}

impl Sweep {
    fn new(
        look: Look,
        solid: bool,
        stations: &[Station],
        uphill: f32,
        profile: impl Fn(&Station) -> Vec<(Vec2, Vec2)>,
    ) -> Self {
        let mut frames = Vec::new();
        let mut rings = Vec::new();
        for st in stations {
            let cols = profile(st);
            let mut ring: Vec<Vec2> = cols.iter().map(|c| c.0).collect();
            ring.extend(cols.iter().rev().map(|c| c.1));
            frames.push((st.at, st.left * uphill));
            rings.push(ring);
        }
        Self {
            look,
            solid,
            frames,
            rings,
        }
    }

    pub fn world(&self, i: usize, k: usize) -> Vec3 {
        let (o, out) = self.frames[i];
        let p = self.rings[i][k];
        o + out * p.x + Vec3::Y * p.y
    }

    /// The world points of bay `i` (between stations `i` and `i + 1`).
    pub fn bay(&self, i: usize) -> Vec<Vec3> {
        let m = self.rings[i].len();
        (0..m)
            .map(|k| self.world(i, k))
            .chain((0..m).map(|k| self.world(i + 1, k)))
            .collect()
    }

    /// An oriented box round bay `i` (for surfaces): half extents, where.
    pub fn bay_box(&self, i: usize) -> (Vec3, Transform) {
        let pts = self.bay(i);
        let m = self.rings[i].len();
        let centre = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
        let c0 = pts[..m].iter().copied().sum::<Vec3>() / m as f32;
        let c1 = pts[m..].iter().copied().sum::<Vec3>() / m as f32;
        let z = (c1 - c0).normalize_or(Vec3::Z);
        let across = self.world(i, m / 2 - 1) - self.world(i, 0);
        let x = (across - z * across.dot(z)).normalize_or(z.cross(Vec3::Y));
        let y = z.cross(x);
        let half = pts.iter().fold(Vec3::ZERO, |h, p| {
            let d = *p - centre;
            h.max(Vec3::new(d.dot(x).abs(), d.dot(y).abs(), d.dot(z).abs()))
        });
        let rot = Quat::from_mat3(&Mat3::from_cols(x, y, z));
        (half, Transform::from_translation(centre).with_rotation(rot))
    }
}

/// One gallery as built: swept concrete and snow, boxes (columns,
/// plaques), and icicles (top, radius, length).
#[derive(Component, Clone, Debug)]
pub struct Gallery {
    pub plan: GalleryPlan,
    pub sweeps: Vec<Sweep>,
    pub pieces: Vec<Piece>,
    pub icicles: Vec<(Vec3, f32, f32)>,
}

/// Every gallery on the map, built.
pub fn galleries(map: &MapDef, hm: &Heightmap) -> Vec<Gallery> {
    gallery_plans(map, hm)
        .into_iter()
        .filter_map(|plan| {
            let rl = hm.road_lines.iter().find(|rl| rl.road == plan.road)?;
            Some(build_gallery(plan, &Line::new(rl), hm))
        })
        .collect()
}

fn build_gallery(plan: GalleryPlan, line: &Line, hm: &Heightmap) -> Gallery {
    let sec = Section {
        hw: plan.width * 0.5,
    };
    let up = plan.uphill;
    let (a, b) = plan.span;
    let concrete = Look::Mat(Mat::Concrete);
    let mut sweeps = Vec::new();
    let mut pieces = Vec::new();
    let col = |u0: f32, v0: f32, u1: f32, v1: f32| (Vec2::new(u0, v0), Vec2::new(u1, v1));

    // The retaining wall, battered, its top inside the roof.
    let (face, face_top, back_foot, back_top) =
        (sec.face(), sec.face_top(), sec.back_foot(), sec.back_top());
    sweeps.push(Sweep::new(concrete, true, &plan.stations, up, |_| {
        vec![
            col(face, -FOOT, face_top, sec.under(face_top) + 0.02),
            col(back_foot, -FOOT, back_top, sec.under(back_top) + 0.02),
        ]
    }));
    // The roof slab, wall to lip, apron and all.
    let (lip, back) = (sec.lip(), sec.back());
    sweeps.push(Sweep::new(concrete, true, &plan.stations, up, |_| {
        vec![
            col(lip, sec.under(lip), lip, sec.top(lip)),
            col(back, sec.under(back), back, sec.top(back)),
        ]
    }));
    // The upturn at the lip, its outer face a few cm proud of the slab's
    // edge.
    sweeps.push(Sweep::new(concrete, true, &plan.stations, up, |_| {
        let (o, i) = (lip - 0.04, lip + 0.3);
        vec![
            col(o, sec.top(o) - 0.12, o - 0.05, sec.top(lip) + 0.6),
            col(i, sec.top(i) - 0.12, i, sec.top(lip) + 0.55),
        ]
    }));
    // The edge beam over the columns.
    sweeps.push(Sweep::new(concrete, true, &plan.stations, up, |_| {
        let (o, i) = (-(sec.hw + 0.8), -(sec.hw + 0.27));
        vec![
            col(o, sec.under(o) - BEAM, o, sec.under(o) + 0.02),
            col(i, sec.under(i) - BEAM, i, sec.under(i) + 0.02),
        ]
    }));
    // Portals: the roof's ends thickened, the wall's ends widened. Each
    // starts a few cm past the slab's end so their end faces don't meet.
    for (p0, p1) in [(a - 0.05, a + PORTAL), (b - PORTAL, b + 0.05)] {
        let ends = [line.station(p0), line.station(p1)];
        sweeps.push(Sweep::new(concrete, true, &ends, up, |_| {
            let (o, i) = (lip - 0.1, back + 0.08);
            vec![
                col(o, sec.under(o) - 0.4, o, sec.top(o) + 0.12),
                col(i, sec.under(i) - 0.4, i, sec.top(i) + 0.12),
            ]
        }));
        sweeps.push(Sweep::new(concrete, true, &ends, up, |_| {
            vec![
                col(
                    face - 0.15,
                    -FOOT - 0.05,
                    face_top - 0.15,
                    sec.under(face_top) + 0.03,
                ),
                col(
                    back_foot + 0.2,
                    -FOOT - 0.05,
                    back_top + 0.2,
                    sec.under(back_top) + 0.03,
                ),
            ]
        }));
    }
    // A date plaque on each portal's outer face, over the road.
    for (s, sgn) in [(a - 0.05, -1.0f32), (b + 0.05, 1.0)] {
        let st = line.station(s);
        let at = st.at + Vec3::Y * (sec.under(0.0) + 0.11) + st.dir * sgn * 0.02;
        pieces.push(Piece {
            size: Vec3::new(0.9, 0.36, 0.06),
            at: Transform::from_translation(at).with_rotation(yaw(st.dir)),
            mat: Mat::DressedStone,
            solid: false,
            seen: true,
        });
    }
    // Buttresses on the wall's face, between the columns.
    let mut s = a + PORTAL + COL_EVERY * 0.5;
    while s < b - PORTAL - 1.0 {
        let ends = [line.station(s - 0.25), line.station(s + 0.25)];
        sweeps.push(Sweep::new(concrete, true, &ends, up, |_| {
            let (o, ot) = (face - 0.35, face_top - 0.1);
            vec![
                col(o, -FOOT - 0.05, ot, sec.under(ot) + 0.02),
                col(
                    face + 0.3,
                    -FOOT - 0.05,
                    face_top + 0.3,
                    sec.under(face_top) + 0.02,
                ),
            ]
        }));
        s += COL_EVERY;
    }
    // The columns, from below the ground to the beam.
    let usable = b - a - 0.8;
    let n = ((usable / COL_EVERY).round() as usize).max(1);
    for k in 0..=n {
        let st = line.station(a + 0.4 + usable * k as f32 / n as f32);
        let base = st.at + plan.out(&st) * sec.col();
        let ground = hm.height([base.x, base.z]);
        let top = st.at.y + sec.under(sec.col()) - BEAM + 0.02;
        let bottom = ground.min(st.at.y) - 0.3;
        pieces.push(Piece {
            size: Vec3::new(COL, top - bottom, COL),
            at: Transform::from_translation(Vec3::new(base.x, (top + bottom) * 0.5, base.z))
                .with_rotation(yaw(st.dir)),
            mat: Mat::Concrete,
            solid: true,
            seen: true,
        });
    }
    // Snow on the roof: a lumpy slab just clear of the concrete, its ends
    // inside the portals, its outer edge inside the upturn.
    let snow_at = line.stations(a + PORTAL - 0.05, b - PORTAL + 0.05, 1.0);
    sweeps.push(Sweep::new(Look::Snow, false, &snow_at, up, |st| {
        let (u0, u1) = (lip + 0.29, back - 0.15);
        let out = st.left * up;
        let n = 10;
        (0..=n)
            .map(|j| {
                let u = u0 + (u1 - u0) * j as f32 / n as f32;
                let p = st.at + out * u;
                let lump = extraction_core::noise::fbm(p.x * 0.35, p.z * 0.35, 0x5A0, 2, 0.5);
                let deep = if j == 0 || j == n {
                    0.04
                } else {
                    (0.15 + 0.1 * lump).max(0.05)
                };
                let base = sec.top(u) + 0.008;
                col(u, base, u, base + deep)
            })
            .collect()
    }));
    // Ice under the lip's overhang.
    let mut icicles = Vec::new();
    let mut s = a + PORTAL + 0.3;
    let mut k = 0u32;
    while s < b - PORTAL - 0.3 {
        k += 1;
        let h = |n: u32| hash01(k, n.wrapping_add(plan.road as u32 * 7));
        if h(0) > 0.35 {
            let st = line.station(s);
            let u = lip + 0.08 + 0.3 * h(1);
            let top = st.at + plan.out(&st) * u + Vec3::Y * (sec.under(u) + 0.04);
            icicles.push((top, 0.025 + 0.035 * h(2), 0.16 + 0.6 * h(3) * h(3)));
        }
        s += 0.37;
    }
    Gallery {
        plan,
        sweeps,
        pieces,
        icicles,
    }
}

fn yaw(dir: Vec3) -> Quat {
    Quat::from_rotation_y(dir.x.atan2(dir.z))
}

/// The galleries, solid.
pub fn spawn_galleries(mut commands: Commands, zone: Res<Zone>) {
    for g in galleries(&zone.map, &zone.hm) {
        let mut parts: Vec<(Vec3, Quat, Collider)> = Vec::new();
        let mut surfaces = Vec::new();
        for sw in g.sweeps.iter().filter(|sw| sw.solid) {
            for i in 0..sw.frames.len() - 1 {
                if let Some(hull) = Collider::convex_hull(sw.bay(i)) {
                    parts.push((Vec3::ZERO, Quat::IDENTITY, hull));
                }
                let (half, at) = sw.bay_box(i);
                surfaces.push((half, at, Mat::Concrete));
            }
        }
        for p in g.pieces.iter().filter(|p| p.solid) {
            parts.push((
                p.at.translation,
                p.at.rotation,
                Collider::cuboid(p.size.x, p.size.y, p.size.z),
            ));
            surfaces.push((p.size * 0.5, p.at, p.mat));
        }
        commands.spawn((
            Name::new("Avalanche gallery"),
            g,
            super::surfaces::Surfaces(surfaces),
            Transform::IDENTITY,
            Visibility::default(),
            RigidBody::Static,
            Collider::compound(parts),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
}

// ---------------------------------------------------------------------------
// Meshes.

/// Triangles with flat normals and UVs in metres.
#[derive(Default)]
struct Buf {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
}

impl Buf {
    /// A quad, wound to face `want`.
    fn quad(&mut self, q: [Vec3; 4], uv: [Vec2; 4], want: Vec3) {
        let n = (q[1] - q[0]).cross(q[2] - q[0]) + (q[2] - q[0]).cross(q[3] - q[0]);
        let flip = n.dot(want) < 0.0;
        let n = if flip { -n } else { n }.normalize_or(want.normalize_or(Vec3::Y));
        let base = self.pos.len() as u32;
        for k in 0..4 {
            self.pos.push(q[k].to_array());
            self.nrm.push(n.to_array());
            self.uv.push(uv[k].to_array());
        }
        if flip {
            self.idx
                .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            self.idx
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    fn sweep(&mut self, sw: &Sweep) {
        let rings = &sw.rings;
        if rings.len() < 2 {
            return;
        }
        let m = rings[0].len();
        let area: f32 = (0..m)
            .map(|k| rings[0][k].perp_dot(rings[0][(k + 1) % m]))
            .sum();
        let ccw = area > 0.0;
        let mut along = vec![0.0f32; rings.len()];
        for i in 1..rings.len() {
            along[i] = along[i - 1] + sw.frames[i].0.distance(sw.frames[i - 1].0);
        }
        let mut perim = vec![0.0f32; m + 1];
        for k in 0..m {
            perim[k + 1] = perim[k] + rings[0][k].distance(rings[0][(k + 1) % m]);
        }
        for i in 0..rings.len() - 1 {
            let out = sw.frames[i].1;
            for k in 0..m {
                let k1 = (k + 1) % m;
                let e = rings[i][k1] - rings[i][k];
                let n2 = if ccw {
                    Vec2::new(e.y, -e.x)
                } else {
                    Vec2::new(-e.y, e.x)
                };
                let want = out * n2.x + Vec3::Y * n2.y;
                let (v0, v1) = (perim[k], perim[k + 1]);
                self.quad(
                    [
                        sw.world(i, k),
                        sw.world(i, k1),
                        sw.world(i + 1, k1),
                        sw.world(i + 1, k),
                    ],
                    [
                        Vec2::new(along[i], v0),
                        Vec2::new(along[i], v1),
                        Vec2::new(along[i + 1], v1),
                        Vec2::new(along[i + 1], v0),
                    ],
                    want,
                );
            }
        }
        // End caps, a quad between each pair of columns.
        let last = rings.len() - 1;
        let centre = |i: usize| (0..m).map(|k| sw.world(i, k)).sum::<Vec3>() / m as f32;
        for (i, want) in [
            (0, centre(0) - centre(1)),
            (last, centre(last) - centre(last - 1)),
        ] {
            let cols = m / 2;
            for j in 0..cols - 1 {
                let ids = [j, j + 1, m - 2 - j, m - 1 - j];
                self.quad(ids.map(|k| sw.world(i, k)), ids.map(|k| rings[i][k]), want);
            }
        }
    }

    fn mesh(self) -> Option<Mesh> {
        if self.idx.is_empty() {
            return None;
        }
        let mut mesh = Mesh::new(
            bevy::render::render_resource::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_indices(bevy::mesh::Indices::U32(self.idx));
        let _ = mesh.generate_tangents();
        Some(mesh)
    }
}

/// Merges `mesh` into the one kept for `key`.
fn merge_into<K: PartialEq>(into: &mut Vec<(K, Mesh)>, key: K, mesh: Mesh) {
    match into.iter_mut().find(|(k, _)| *k == key) {
        Some((_, m)) => {
            let _ = m.merge(&mesh);
        }
        None => into.push((key, mesh)),
    }
}

/// The two materials this module makes, built once.
#[derive(Resource, Clone)]
struct OwnMaterials {
    snow: Handle<StandardMaterial>,
    ice: Handle<StandardMaterial>,
}

fn material(
    look: Look,
    own: &mut Option<OwnMaterials>,
    library: &mut crate::render::materials::MaterialLibrary,
    server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let own = own.get_or_insert_with(|| OwnMaterials {
        snow: materials.add(StandardMaterial {
            base_color: Color::srgb(0.86, 0.88, 0.9),
            perceptual_roughness: 0.95,
            reflectance: 0.3,
            ..default()
        }),
        ice: materials.add(StandardMaterial {
            base_color: Color::srgb(0.78, 0.86, 0.92),
            perceptual_roughness: 0.15,
            reflectance: 0.6,
            ..default()
        }),
    });
    match look {
        Look::Mat(m) => library.get(m, server, materials),
        Look::Snow => own.snow.clone(),
        Look::Ice => own.ice.clone(),
    }
}

/// The galleries, seen: one mesh per look.
#[allow(clippy::too_many_arguments)]
fn gallery_visuals(
    mut commands: Commands,
    added: Query<(Entity, &Gallery), Added<Gallery>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<crate::render::materials::MaterialLibrary>,
    server: Res<AssetServer>,
    mut own: Local<Option<OwnMaterials>>,
) {
    for (e, g) in &added {
        let mut merged: Vec<(Look, Mesh)> = Vec::new();
        for sw in &g.sweeps {
            let mut buf = Buf::default();
            buf.sweep(sw);
            if let Some(mesh) = buf.mesh() {
                merge_into(&mut merged, sw.look, mesh);
            }
        }
        for p in g.pieces.iter().filter(|p| p.seen) {
            let mesh = crate::render::materials::metric_box(p.size).transformed_by(p.at);
            merge_into(&mut merged, Look::Mat(p.mat), mesh);
        }
        for (look, mesh) in merged {
            let mat = material(look, &mut own, &mut library, &server, &mut materials);
            commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat), ChildOf(e)));
        }
        // Icicles: small, many, pointing down; gone past a few tens of
        // metres, and no shadows.
        let mut ice: Option<Mesh> = None;
        for (top, r, len) in &g.icicles {
            let mut m = Cone::new(*r, *len)
                .mesh()
                .resolution(6)
                .build()
                .rotated_by(Quat::from_rotation_x(std::f32::consts::PI))
                .translated_by(*top - Vec3::Y * (len * 0.5));
            let _ = m.generate_tangents();
            match &mut ice {
                Some(into) => {
                    let _ = into.merge(&m);
                }
                None => ice = Some(m),
            }
        }
        if let Some(mesh) = ice {
            let mat = material(Look::Ice, &mut own, &mut library, &server, &mut materials);
            commands.spawn((
                crate::render::centred(&mut meshes, mesh),
                MeshMaterial3d(mat),
                bevy::light::NotShadowCaster,
                bevy::camera::visibility::VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: 50.0..70.0,
                    use_aabb: true,
                },
                ChildOf(e),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// The funicular.

/// What a piece of the funicular is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitKind {
    Sleeper,
    Rail,
    Rope,
    Roller,
    Tread,
    Handrail,
}

/// One piece: a box (or, `rod`, a cylinder along its local Y of radius
/// `size.x` and length `size.y`), in a chunk of track.
#[derive(Clone, Copy, Debug)]
pub struct Bit {
    pub kind: BitKind,
    pub chunk: usize,
    pub look: Look,
    pub rod: bool,
    pub size: Vec3,
    pub at: Transform,
}

/// The funicular as laid: its road, its length, its pieces.
#[derive(Clone, Debug)]
pub struct Funicular {
    pub road: usize,
    pub length: f32,
    /// Horizontal unit vectors: up the line, and its left looking up it.
    pub up: Vec3,
    pub left: Vec3,
    pub bits: Vec<Bit>,
}

/// Track centre's offset to the left of the path's centreline, looking
/// uphill (m).
const TRACK: f32 = 1.2;
/// Rail gauge (m), sleeper spacing (m) and length across (m).
const GAUGE: f32 = 1.0;
const SLEEPER_EVERY: f32 = 0.9;
const SLEEPER_LEN: f32 = 1.3;
/// Sleepers' tops stand this far above the ground (m).
const SLEEPER_TOP: f32 = 0.07;
/// The passing loop's full-width stretch (m) and its switches' length (m).
const LOOP: f32 = 30.0;
const SWITCH: f32 = 12.0;
/// How far each loop track moves off the single line (outer, inner; m).
const LOOP_OUT: f32 = 0.62;
const LOOP_IN: f32 = 0.78;
/// Rope rollers' spacing (m).
const ROLLER_EVERY: f32 = 8.0;
/// The stair's centre to the right of the centreline (m), its width and
/// tread spacing (m).
const STAIR: f32 = 1.3;
const STAIR_W: f32 = 1.1;
const TREAD: f32 = 0.3;
/// Chunk length for merged meshes (m).
const CHUNK: f32 = 24.0;

/// The funicular on this map, if it has a road named [`FUNICULAR`].
pub fn funicular(map: &MapDef, hm: &Heightmap) -> Option<Funicular> {
    let road = map.roads.iter().position(|r| r.name == FUNICULAR)?;
    let rl = hm.road_lines.iter().find(|rl| rl.road == road)?;
    let line = Line::new(rl);
    let length = line.len();
    if length < 10.0 {
        return None;
    }
    // Uphill is whichever end is higher.
    let forward = rl.heights.last() >= rl.heights.first();
    let st0 = line.station(if forward { 0.0 } else { length });
    let st1 = line.station(if forward { length } else { 0.0 });
    let up = Vec3::new(st1.at.x - st0.at.x, 0.0, st1.at.z - st0.at.z).normalize_or(Vec3::NEG_Z);
    let left = Vec3::Y.cross(up);
    // A point `t` up the line, `off` to its left (right if negative), on
    // the ground.
    let at = |t: f32, off: f32| -> Vec3 {
        let st = line.station(if forward { t } else { length - t });
        let l = if forward { st.left } else { -st.left };
        let p = st.at + l * off;
        Vec3::new(p.x, hm.height([p.x, p.z]), p.z)
    };
    // The loop: 0 on the single line, 1 through the loop.
    let mid = length * 0.5;
    let has_loop = length > LOOP + 2.0 * SWITCH + 10.0;
    let spread = |t: f32| -> f32 {
        if !has_loop {
            return 0.0;
        }
        geom::smoothstep(mid - LOOP * 0.5 - SWITCH, mid - LOOP * 0.5, t)
            * (1.0 - geom::smoothstep(mid + LOOP * 0.5, mid + LOOP * 0.5 + SWITCH, t))
    };
    let outer = |t: f32| TRACK + LOOP_OUT * spread(t);
    let inner = |t: f32| TRACK - LOOP_IN * spread(t);
    let chunk = |t: f32| (t / CHUNK).max(0.0) as usize;
    let mut bits = Vec::new();
    let mut put = |kind: BitKind, t: f32, look: Look, rod: bool, size: Vec3, at: Transform| {
        bits.push(Bit {
            kind,
            chunk: chunk(t),
            look,
            rod,
            size,
            at,
        });
    };
    // A box from `a` to `b` (centres), `w` wide and `h` tall, pitched
    // along, no roll.
    let beam = |a: Vec3, b: Vec3, w: f32, h: f32| -> (Vec3, Transform) {
        let d = b - a;
        let run = Vec2::new(d.x, d.z).length();
        let rot = Quat::from_rotation_y(d.x.atan2(d.z)) * Quat::from_rotation_x(-d.y.atan2(run));
        (
            Vec3::new(w, h, d.length() + 0.01),
            Transform::from_translation((a + b) * 0.5).with_rotation(rot),
        )
    };
    let rod = |a: Vec3, b: Vec3, r: f32| -> (Vec3, Transform) {
        let d = b - a;
        (
            Vec3::new(r, d.length(), r),
            Transform::from_translation((a + b) * 0.5)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y))),
        )
    };
    let up_yaw = Quat::from_rotation_y(up.x.atan2(up.z));
    let pitch = |t: f32| {
        let (a, b) = (at(t - 0.5, 0.0), at(t + 0.5, 0.0));
        Quat::from_rotation_x(-(b.y - a.y).atan2(1.0))
    };
    let (t0, t1) = (1.5, length - 1.5);

    // Sleepers: one under the single line, one under each loop track, or
    // one long one where the switches' tracks are too close for two.
    let mut t = t0;
    while t <= t1 {
        let (o, i) = (outer(t), inner(t));
        let spans: Vec<(f32, f32)> = if o - i < 0.02 {
            vec![(TRACK, SLEEPER_LEN)]
        } else if o - i < SLEEPER_LEN + 0.05 {
            vec![((o + i) * 0.5, o - i + SLEEPER_LEN)]
        } else {
            vec![(o, SLEEPER_LEN), (i, SLEEPER_LEN)]
        };
        let look = Look::Mat(if spread(t) > 0.01 {
            Mat::Concrete
        } else {
            Mat::Timber
        });
        for (c, len) in spans {
            let p = at(t, c);
            put(
                BitKind::Sleeper,
                t,
                look,
                false,
                Vec3::new(len, 0.12, 0.22),
                Transform::from_translation(p + Vec3::Y * (SLEEPER_TOP - 0.06))
                    .with_rotation(up_yaw * pitch(t)),
            );
        }
        t += SLEEPER_EVERY;
    }
    // Rails: the single line's (the outer loop track's) all the way, the
    // inner loop track's where it has left it (a hair lower, so where the
    // two meet at a switch they don't fight).
    let rail_y = SLEEPER_TOP + 0.055;
    let n = ((t1 - t0) / SLEEPER_EVERY).ceil() as usize;
    let ts: Vec<f32> = (0..=n)
        .map(|k| t0 + (t1 - t0) * k as f32 / n as f32)
        .collect();
    for (track, drop) in [(&outer as &dyn Fn(f32) -> f32, 0.0f32), (&inner, 0.008)] {
        for w in ts.windows(2) {
            if drop > 0.0 && spread(w[0]) < 0.01 && spread(w[1]) < 0.01 {
                continue;
            }
            for side in [-0.5f32, 0.5] {
                let p = |t: f32| {
                    let c = at(t, track(t));
                    let r = at(t, track(t) + side * GAUGE);
                    Vec3::new(r.x, c.y + rail_y - drop, r.z)
                };
                let (size, xf) = beam(p(w[0]), p(w[1]), 0.06, 0.11);
                put(
                    BitKind::Rail,
                    w[0],
                    Look::Mat(Mat::RustyMetal),
                    false,
                    size,
                    xf,
                );
            }
        }
    }
    // The rope on rollers down the middle of each track.
    let roller_r = 0.07;
    let rope_r = 0.016;
    let mut rt = Vec::new();
    let mut t = t0 + 1.0;
    while t <= t1 {
        rt.push(t);
        t += ROLLER_EVERY;
    }
    for (track, single) in [(&outer as &dyn Fn(f32) -> f32, true), (&inner, false)] {
        let rope_at = |t: f32| at(t, track(t)) + Vec3::Y * (SLEEPER_TOP + 2.0 * roller_r + rope_r);
        for &t in &rt {
            if !single && spread(t) < 0.01 {
                continue;
            }
            let c = at(t, track(t));
            let l = left;
            let (size, xf) = rod(
                c + Vec3::Y * (SLEEPER_TOP + roller_r) - l * 0.15,
                c + Vec3::Y * (SLEEPER_TOP + roller_r) + l * 0.15,
                roller_r,
            );
            put(
                BitKind::Roller,
                t,
                Look::Mat(Mat::DarkMetal),
                true,
                size,
                xf,
            );
            put(
                BitKind::Roller,
                t,
                Look::Mat(Mat::DarkMetal),
                false,
                Vec3::new(0.4, 0.1, 0.12),
                Transform::from_translation(c + Vec3::Y * 0.03).with_rotation(up_yaw),
            );
        }
        for w in rt.windows(2) {
            if !single && spread(w[0]) < 0.01 && spread(w[1]) < 0.01 {
                continue;
            }
            let (size, xf) = rod(rope_at(w[0]), rope_at(w[1]), rope_r);
            put(
                BitKind::Rope,
                w[0],
                Look::Mat(Mat::DarkMetal),
                true,
                size,
                xf,
            );
        }
    }
    // The service stair: level concrete treads up the slope, each from
    // below the ground at its front to just above it at its back.
    let mut t = 1.0;
    while t + TREAD <= length - 1.0 {
        let corners = [
            at(t, -STAIR + STAIR_W * 0.5),
            at(t, -STAIR - STAIR_W * 0.5),
            at(t + TREAD, -STAIR + STAIR_W * 0.5),
            at(t + TREAD, -STAIR - STAIR_W * 0.5),
        ];
        let top = corners.iter().map(|c| c.y).fold(f32::MIN, f32::max) + 0.03;
        let bottom = corners.iter().map(|c| c.y).fold(f32::MAX, f32::min) - 0.08;
        let c = at(t + TREAD * 0.5, -STAIR);
        put(
            BitKind::Tread,
            t,
            Look::Mat(Mat::Concrete),
            false,
            Vec3::new(STAIR_W, top - bottom, TREAD),
            Transform::from_translation(Vec3::new(c.x, (top + bottom) * 0.5, c.z))
                .with_rotation(up_yaw),
        );
        t += TREAD;
    }
    // Its handrail along the outer edge: posts, a top rail and a mid rail.
    let rail_off = -STAIR - STAIR_W * 0.5 - 0.08;
    let n = ((length - 2.0) / 1.8).ceil() as usize;
    let posts: Vec<f32> = (0..=n)
        .map(|k| 1.0 + (length - 2.0) * k as f32 / n as f32)
        .collect();
    for &t in &posts {
        let g = at(t, rail_off);
        put(
            BitKind::Handrail,
            t,
            Look::Mat(Mat::RustyMetal),
            false,
            Vec3::new(0.05, 1.1, 0.05),
            Transform::from_translation(g + Vec3::Y * 0.45).with_rotation(up_yaw),
        );
    }
    for w in posts.windows(2) {
        for (h, th) in [(1.0, 0.045), (0.5, 0.035)] {
            let (size, xf) = beam(
                at(w[0], rail_off) + Vec3::Y * h,
                at(w[1], rail_off) + Vec3::Y * h,
                th,
                th,
            );
            put(
                BitKind::Handrail,
                w[0],
                Look::Mat(Mat::RustyMetal),
                false,
                size,
                xf,
            );
        }
    }
    Some(Funicular {
        road,
        length,
        up,
        left,
        bits,
    })
}

/// The funicular's track and stair, seen: a mesh per look per chunk.
fn lay_funicular(
    mut commands: Commands,
    zone: Res<Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<crate::render::materials::MaterialLibrary>,
    server: Res<AssetServer>,
) {
    let Some(f) = funicular(&zone.map, &zone.hm) else {
        return;
    };
    let mut merged: Vec<((usize, Look), Mesh)> = Vec::new();
    for b in &f.bits {
        let mut mesh = if b.rod {
            Cylinder::new(b.size.x, b.size.y)
                .mesh()
                .resolution(if b.kind == BitKind::Rope { 6 } else { 10 })
                .build()
        } else {
            crate::render::materials::metric_box(b.size)
        };
        let _ = mesh.generate_tangents();
        merge_into(&mut merged, (b.chunk, b.look), mesh.transformed_by(b.at));
    }
    let mut own = None;
    let root = commands
        .spawn((
            Name::new("Funicular track"),
            Transform::IDENTITY,
            Visibility::default(),
        ))
        .id();
    for ((_, look), mesh) in merged {
        let mat = material(look, &mut own, &mut library, &server, &mut materials);
        // Rails, rope and rollers are thin: no shadow worth its pass.
        let thin = matches!(look, Look::Mat(Mat::RustyMetal | Mat::DarkMetal));
        let mut e = commands.spawn((
            crate::render::centred(&mut meshes, mesh),
            MeshMaterial3d(mat),
            bevy::camera::visibility::VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: if thin { 70.0..90.0 } else { 110.0..130.0 },
                use_aabb: true,
            },
            ChildOf(root),
        ));
        if thin {
            e.insert(bevy::light::NotShadowCaster);
        }
    }
    info!(
        "funicular: {:.0} m, {} pieces in {} chunks",
        f.length,
        f.bits.len(),
        f.bits.iter().map(|b| b.chunk).max().unwrap_or(0) + 1
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slope rising to the east (x), a road along it north-south under a
    /// 40 m gallery, and a straight footpath named for the funicular
    /// running north (-z) up a slope rising to the north.
    fn test_map(rise_east: bool) -> MapDef {
        let dir = if rise_east {
            "(1.0, 0.0)"
        } else {
            "(-1.0, 0.0)"
        };
        MapDef::from_ron(&format!(
            r#"(
                name: "roadworks test", size: 300.0, cell: 1.0, base_height: 20.0,
                noise: (seed: 1, amplitude: 0.0, scale: 100.0, octaves: 1, detail: 0.0, relief: 0.0, erosion: 0.0),
                rim: (width: 10.0, height: 5.0),
                features: [Tilt(dir: {dir}, rise_per_100m: 30.0)],
                roads: [
                    (name: "Pass road", kind: Gravel, width: 6.0,
                     points: [(-20.0, 80.0), (-20.0, 0.0), (-25.0, -80.0)],
                     galleries: [(at: (-19.0, 20.0), length: 40.0)]),
                ],
            )"#
        ))
        .unwrap()
    }

    fn funicular_map() -> MapDef {
        MapDef::from_ron(
            r#"(
                name: "funicular test", size: 300.0, cell: 1.0, base_height: 20.0,
                noise: (seed: 1, amplitude: 0.0, scale: 100.0, octaves: 1, detail: 0.0, relief: 0.0, erosion: 0.0),
                rim: (width: 10.0, height: 5.0),
                features: [Tilt(dir: (0.0, -1.0), rise_per_100m: 20.0)],
                roads: [
                    (name: "Funicular", kind: Path, width: 5.0, smoothing: 30.0,
                     points: [(30.0, 60.0), (30.0, -60.0)]),
                ],
            )"#,
        )
        .unwrap()
    }

    /// The span comes out centred where it was put and as long as asked,
    /// the uphill side is the side the land rises on, and the road keeps
    /// its width under it.
    #[test]
    fn galleries_span_the_road_and_lean_on_the_uphill_side() {
        for rise_east in [true, false] {
            let map = test_map(rise_east);
            let hm = Heightmap::build(&map);
            let plans = gallery_plans(&map, &hm);
            assert_eq!(plans.len(), 1);
            let p = &plans[0];
            assert!(((p.span.1 - p.span.0) - 40.0).abs() < 0.1, "{:?}", p.span);
            let rl = &hm.road_lines[0];
            let mid = geom::polyline_at(&rl.points, (p.span.0 + p.span.1) * 0.5).0;
            assert!(geom::dist(mid, [-20.0, 20.0]) < 1.5, "centred at {mid:?}");
            // Bays no longer than BAY, ends on the span.
            assert!(p.stations.windows(2).all(|w| w[1].s - w[0].s <= BAY + 1e-3));
            assert!((p.stations[0].s - p.span.0).abs() < 1e-3);
            // The road runs south (points go +z to -z): its left is west,
            // so a slope rising east is uphill on its right.
            let st = &p.stations[p.stations.len() / 2];
            let out = p.out(st);
            assert_eq!(out.x > 0.5, rise_east, "uphill {out:?}");
            // Stations sit on the grade.
            let line = Line::new(rl);
            assert!((st.at.y - line.graded(st.s)).abs() < 1e-3);

            // Built: columns stand downhill, clear of the road's width by
            // 0.3 m, and the wall's face uphill clear of its shoulder.
            let g = &galleries(&map, &hm)[0];
            let cols: Vec<&Piece> = g
                .pieces
                .iter()
                .filter(|p| p.solid && p.mat == Mat::Concrete)
                .collect();
            assert!(cols.len() >= 7, "{} columns", cols.len());
            for c in &cols {
                let x = c.at.translation.x;
                let lateral = x - centre_x(rl, c.at.translation.z);
                let inner = lateral.abs() - COL * 0.5;
                assert!(inner >= 3.0 + 0.3 - 0.05, "column at {lateral}");
                assert_eq!(lateral > 0.0, !rise_east, "column on the downhill side");
            }
            let wall = &g.sweeps[0];
            for i in 0..wall.frames.len() {
                let foot = wall.world(i, 0);
                let lateral = foot.x - centre_x(rl, foot.z);
                assert!(lateral.abs() >= 3.5, "wall face at {lateral}");
                assert_eq!(lateral > 0.0, rise_east);
            }
            // Headroom over the whole road.
            let roof = &g.sweeps[1];
            for i in 0..roof.frames.len() {
                let (o, _) = roof.frames[i];
                for k in 0..roof.rings[i].len() {
                    let p = roof.rings[i][k];
                    if p.x.abs() <= 3.0 {
                        assert!(p.y >= HEAD - 1e-3);
                    }
                }
                assert!(o.y > 0.0);
            }
            // Every bay of every solid sweep makes a hull.
            for sw in g.sweeps.iter().filter(|s| s.solid) {
                for i in 0..sw.frames.len() - 1 {
                    assert!(Collider::convex_hull(sw.bay(i)).is_some());
                }
            }
        }
    }

    /// The centreline's x at a given z (the test road runs along z).
    fn centre_x(rl: &RoadLine, z: f32) -> f32 {
        rl.points
            .iter()
            .min_by(|a, b| (a[1] - z).abs().total_cmp(&(b[1] - z).abs()))
            .unwrap()[0]
    }

    /// The track lies on the west half of a north-running line and the
    /// stair on the east, all within the path's width, the loop at its
    /// middle; sleepers every 0.9 m, a few dozen chunks at most.
    #[test]
    fn the_funicular_is_laid_on_its_path() {
        let map = funicular_map();
        let hm = Heightmap::build(&map);
        let f = funicular(&map, &hm).expect("a funicular");
        assert!(f.up.z < -0.99, "uphill is north: {:?}", f.up);
        assert!(f.left.x < -0.99, "left looking uphill is west");
        let half = 2.5;
        let mut loop_z = Vec::new();
        for b in &f.bits {
            let x = b.at.translation.x - 30.0;
            match b.kind {
                BitKind::Sleeper | BitKind::Rail | BitKind::Rope | BitKind::Roller => {
                    assert!(x < 0.25, "{:?} east of the centre at {x}", b.kind);
                    let reach = if b.kind == BitKind::Sleeper {
                        b.size.x * 0.5
                    } else {
                        0.05
                    };
                    assert!(x - reach > -half - 0.01, "{:?} off the path at {x}", b.kind);
                    if b.kind == BitKind::Rail && x > -0.3 {
                        loop_z.push(b.at.translation.z);
                    }
                }
                BitKind::Tread | BitKind::Handrail => {
                    assert!(x > 0.5 && x < half, "{:?} at {x}", b.kind);
                }
            }
        }
        // The inner loop rails come near the centre only mid-line.
        assert!(!loop_z.is_empty());
        assert!(loop_z.iter().all(|z| z.abs() < 30.0), "{loop_z:?}");
        let sleepers = f.bits.iter().filter(|b| b.kind == BitKind::Sleeper).count();
        let expect = (f.length - 3.0) / SLEEPER_EVERY;
        assert!(
            sleepers as f32 >= expect && (sleepers as f32) < expect * 1.4,
            "{sleepers} sleepers"
        );
        let chunks = f.bits.iter().map(|b| b.chunk).max().unwrap() + 1;
        assert!(chunks <= 6, "{chunks} chunks");
        // Treads on the ground: their tops a few cm over it.
        for b in f.bits.iter().filter(|b| b.kind == BitKind::Tread) {
            let top = b.at.translation.y + b.size.y * 0.5;
            let g = hm.height([b.at.translation.x, b.at.translation.z]);
            assert!(top > g && top < g + 0.2, "tread top {top} over {g}");
        }
    }

    /// No funicular without the road, and no galleries on maps without.
    #[test]
    fn nothing_where_nothing_is_asked_for() {
        let map = test_map(true);
        let hm = Heightmap::build(&map);
        assert!(funicular(&map, &hm).is_none());
        let map = funicular_map();
        let hm = Heightmap::build(&map);
        assert!(gallery_plans(&map, &hm).is_empty());
    }
}
