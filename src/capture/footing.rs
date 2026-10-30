//! Model correctness: every placed prop on what's under it (M3 in
//! `docs/playtests/2026-10-28/TRANSCRIPT.md`). The playtest saw a stool
//! floating by the camp table, a crate in the ground, a table with two
//! legs in the air and the truck sunk in the soil. Each needs a valley's
//! ground to show, so no model check could see them.
//!
//! For every valley and the training yard, every model placed in it
//! (buildings' furniture, fittings, clutter and yards, staged scenes, the
//! camp) is set where the game sets it. Its feet are the clusters of its
//! vertices within [`BAND`] of its lowest point, in its own space (a
//! table's four legs, a crate's bottom). Each foot is held to the nearest
//! surface on the vertical through it: the terrain, a drawn box's top, or
//! another model's triangle. A foot may stand off it by at most [`FLOAT`];
//! a wide foot may sink into it by at most [`SINK_WIDE`] on the ground
//! (the grass hides a little) or [`SINK_FLOOR`] on a floor. A slender one
//! (a leg, a post) may go deeper. A model all of whose feet are more than
//! [`HUNG`] over anything is hung on a wall, not standing.
//!
//! The truck stands on its wheels: each wheel's lowest point is held to
//! the ground the same way.
//!
//! `cargo test --lib capture::footing -- --nocapture` prints every finding.

use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::prelude::Transform;
use std::collections::HashMap;
use std::sync::Arc;

use super::glb;
use crate::world::Zone;
use crate::world::buildings::{self, Shape};

/// A foot's vertices: within this of the model's lowest point (m).
pub const BAND: f32 = 0.006;
/// How far a foot may stand off what's under it (m).
pub const FLOAT: f32 = 0.015;
/// How far a wide foot may sink into the ground (m).
pub const SINK_WIDE: f32 = 0.04;
/// How far a wide foot may sink into a floor or another model (m): the
/// floorboards stand 3 cm proud of the plinth the furniture and the
/// bodies stand on (`Builder::plinth`), so it beds evenly into them.
pub const SINK_FLOOR: f32 = 0.035;
/// How far a slender foot (a leg, a post) may go into the ground (m): a
/// post is set in it.
pub const SINK_SLENDER: f32 = 0.5;
/// A foot narrower than this both ways is slender (m).
pub const SLENDER: f32 = 0.12;
/// A model whose every foot is further than this over anything hangs (m).
pub const HUNG: f32 = 0.25;

/// Models that lie how they fell, are meant to be half buried, or are
/// fittings in a wall (a window's casing and sash, a door's leaf: held by
/// their openings, `capture::attach` checks those), or hung on one.
pub fn exempt(name: &str) -> bool {
    [
        "window_",
        "door_leaf",
        "door_metal",
        "_hook",
        "peg",
        "tally_board",
        "frame",
        "bulkhead_lamp",
        "waders",
        "walking_stick",
        "broom",
        "dog_collar",
        "_board",
        "pigeonholes",
        "timetable",
        "barometer",
        "wall_",
        "antlers",
        "stuffed_pike",
        "rods_rack",
        "signal_flags",
        "photo_church",
        "child_drawing",
        "horseshoe",
        "skis_wall",
        "stove_pipe_rack",
        "drying_line",
        "cloth_pinned",
        "med_cabinet",
        // Held by its gate post.
        "barrier_arm",
        "rubble",
        "ash_heap",
        "beam_fallen",
        "beam_charred",
        "_burnt",
        "_charred",
        "ash_film",
        // Heroes and works over their blockouts: the building code stands
        // them on their own foundations (`prefabs_footprint_...`).
        "mill_",
        "obs_",
        "fun_",
        "ropeway",
        "blast_furnace",
        "headframe",
        "water_tower",
        "steeple",
        "signal_box",
        "kept_bus",
        "wreck_",
    ]
    .iter()
    .any(|e| name.contains(e))
}

/// Things set into the ground (a raised bed's frame, a stump, a pump's
/// pipe): a wide foot may go 15 cm in.
pub fn dug_in(name: &str) -> bool {
    ["garden_bed", "chopping_block", "hand_pump"]
        .iter()
        .any(|e| name.contains(e))
}

/// A model's feet and triangles, in its own space.
pub struct Shape3 {
    pub tris: Vec<[Vec3; 3]>,
    pub lo: Vec3,
    pub hi: Vec3,
    /// Each foot: up to eight of its outermost points, and whether it's
    /// wide.
    pub feet: Vec<(Vec<Vec3>, bool)>,
}

pub fn shape(path: &std::path::Path) -> Option<Shape3> {
    let tris: Vec<[Vec3; 3]> = glb::triangles(path)
        .ok()?
        .into_iter()
        .filter(|t| !t.node.contains("_lod"))
        .map(|t| t.p)
        .collect();
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for t in &tris {
        for p in t {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
    }
    if tris.is_empty() {
        return None;
    }
    // The lowest band's vertices, and points filling its faces (a box's
    // underside is four corners: they'd be four tiny feet, not one).
    let mut low: Vec<Vec3> = tris
        .iter()
        .flatten()
        .filter(|p| p.y < lo.y + BAND)
        .copied()
        .collect();
    for t in tris.iter().filter(|t| t.iter().all(|p| p.y < lo.y + BAND)) {
        let n = ((t[1] - t[0]).length().max((t[2] - t[0]).length()) / 0.03).ceil() as usize;
        for i in 0..=n.min(40) {
            for j in 0..=(n.min(40) - i) {
                let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
                low.push(t[0] + (t[1] - t[0]) * u + (t[2] - t[0]) * v);
            }
        }
    }
    Some(Shape3 {
        feet: clusters(&low, 0.03),
        tris,
        lo,
        hi,
    })
}

/// Points grouped by touching `cell`-sized squares of the ground plane.
fn clusters(pts: &[Vec3], cell: f32) -> Vec<(Vec<Vec3>, bool)> {
    let key = |p: &Vec3| ((p.x / cell).floor() as i32, (p.z / cell).floor() as i32);
    let mut cells: HashMap<(i32, i32), usize> = HashMap::new();
    for p in pts {
        let n = cells.len();
        cells.entry(key(p)).or_insert(n);
    }
    let mut parent: Vec<usize> = (0..cells.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let list: Vec<((i32, i32), usize)> = cells.iter().map(|(k, v)| (*k, *v)).collect();
    for ((x, z), i) in &list {
        for (dx, dz) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
            if let Some(&j) = cells.get(&(x + dx, z + dz)) {
                let (a, b) = (find(&mut parent, *i), find(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    let mut groups: HashMap<usize, Vec<Vec3>> = HashMap::new();
    for p in pts {
        let c = cells[&key(p)];
        let r = find(&mut parent, c);
        groups.entry(r).or_default().push(*p);
    }
    let mut out: Vec<(Vec<Vec3>, bool)> = groups
        .into_values()
        .map(|g| {
            // The outermost points: along x, z and both diagonals.
            let dirs = [
                Vec2::X,
                Vec2::NEG_X,
                Vec2::Y,
                Vec2::NEG_Y,
                Vec2::ONE,
                -Vec2::ONE,
                Vec2::new(1.0, -1.0),
                Vec2::new(-1.0, 1.0),
            ];
            let mut keep: Vec<Vec3> = dirs
                .iter()
                .map(|d| {
                    *g.iter()
                        .max_by(|a, b| d.dot(a.xz()).total_cmp(&d.dot(b.xz())))
                        .unwrap()
                })
                .collect();
            // And a grid across it: a counter top's underside is one quad
            // whose corners overhang its cabinets; its middle stands on them.
            let (lo3, hi3) = g.iter().fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(l, h), p| (l.min(*p), h.max(*p)),
            );
            for i in 1..4 {
                for j in 1..4 {
                    let (u, v) = (i as f32 / 4.0, j as f32 / 4.0);
                    keep.push(Vec3::new(
                        lo3.x + (hi3.x - lo3.x) * u,
                        lo3.y,
                        lo3.z + (hi3.z - lo3.z) * v,
                    ));
                }
            }
            keep.dedup_by(|a, b| a.distance(*b) < 1e-4);
            let (lo, hi) = g.iter().fold(
                (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
                |(l, h), p| (l.min(p.xz()), h.max(p.xz())),
            );
            let size = hi - lo;
            (keep, size.x > SLENDER || size.y > SLENDER)
        })
        .collect();
    out.sort_by(|a, b| a.0[0].x.total_cmp(&b.0[0].x));
    out
}

/// One model placed in the world.
pub struct Placed {
    pub name: String,
    pub at: Transform,
    pub shape: Arc<Shape3>,
    /// World triangles and bounds.
    tris: Vec<[Vec3; 3]>,
    lo: Vec3,
    hi: Vec3,
}

impl Placed {
    fn new(name: &str, at: Transform, shape: Arc<Shape3>) -> Self {
        let tris: Vec<[Vec3; 3]> = shape
            .tris
            .iter()
            .map(|t| t.map(|p| at.transform_point(p)))
            .collect();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for t in &tris {
            for p in t {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        Self {
            name: name.to_string(),
            at,
            shape,
            tris,
            lo,
            hi,
        }
    }
}

/// Everything at one site (a building, a scene, the camp): its drawn
/// boxes in the world, and its models.
pub struct Site {
    pub name: String,
    pub boxes: Vec<(Transform, Vec3)>,
    pub models: Vec<Placed>,
    /// The terrain counts (not down an underground site).
    pub terrain: bool,
}

/// The height of a triangle over (x, z), if it covers it.
fn tri_height(t: &[Vec3; 3], x: f32, z: f32) -> Option<f32> {
    let (a, b, c) = (t[0], t[1], t[2]);
    let d = (b.x - a.x) * (c.z - a.z) - (c.x - a.x) * (b.z - a.z);
    if d.abs() < 1e-10 {
        return None;
    }
    let u = ((x - a.x) * (c.z - a.z) - (c.x - a.x) * (z - a.z)) / d;
    let v = ((b.x - a.x) * (z - a.z) - (x - a.x) * (b.z - a.z)) / d;
    (u >= -1e-5 && v >= -1e-5 && u + v <= 1.0 + 1e-5)
        .then(|| a.y + u * (b.y - a.y) + v * (c.y - a.y))
}

/// Where a vertical line from `top` down to `bottom` enters a box (its
/// top surface there), if it passes through it.
fn box_top(at: &Transform, half: Vec3, x: f32, z: f32, top: f32, bottom: f32) -> Option<f32> {
    let inv = at.compute_affine().inverse();
    let a = inv.transform_point3(Vec3::new(x, top, z));
    let b = inv.transform_point3(Vec3::new(x, bottom, z));
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..3 {
        if d[k].abs() < 1e-9 {
            if a[k].abs() > half[k] {
                return None;
            }
            continue;
        }
        let (mut e, mut f) = ((-half[k] - a[k]) / d[k], (half[k] - a[k]) / d[k]);
        if e > f {
            std::mem::swap(&mut e, &mut f);
        }
        t0 = t0.max(e);
        t1 = t1.min(f);
        if t0 > t1 {
            return None;
        }
    }
    Some(top + (bottom - top) * t0)
}

impl Site {
    /// Whether another model's upward face comes up to a wide foot
    /// anywhere under it (its points' bounds), leaving model `skip` out.
    fn under_area(&self, skip: usize, pts: &[(f32, Vec3, String)]) -> bool {
        let (lo, hi) = pts.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(l, h), p| (l.min(p.1), h.max(p.1)),
        );
        let y = lo.y;
        self.models.iter().enumerate().any(|(i, m)| {
            i != skip
                && m.lo.x <= hi.x
                && m.hi.x >= lo.x
                && m.lo.z <= hi.z
                && m.hi.z >= lo.z
                && m.tris.iter().any(|t| {
                    let n = (t[1] - t[0]).cross(t[2] - t[0]);
                    n.y > 0.0
                        && t.iter().any(|p| {
                            p.x >= lo.x
                                && p.x <= hi.x
                                && p.z >= lo.z
                                && p.z <= hi.z
                                && p.y >= y - FLOAT
                                && p.y <= y + SINK_FLOOR
                        })
                })
        })
    }

    /// What's on the vertical through `p`, leaving model `skip` out: the
    /// surfaces under it that could hold it (height, what), and how deep
    /// it is in a solid (the terrain over it, a box round it), if it is.
    fn under(
        &self,
        zone: &Zone,
        skip: usize,
        p: Vec3,
    ) -> (Vec<(f32, String)>, Option<(f32, String)>) {
        let (top, bottom) = (p.y + 0.5, p.y - 1.0);
        let mut below = Vec::new();
        let mut inside: Option<(f32, String)> = None;
        let mut deeper = |d: f32, what: &str| {
            if inside.as_ref().is_none_or(|(e, _)| d > *e) {
                inside = Some((d, what.to_string()));
            }
        };
        if self.terrain {
            let g = zone.ground(p.x, p.z);
            if g > p.y && g < top {
                deeper(g - p.y, "ground");
            } else if g >= bottom && g <= p.y {
                below.push((g, "ground".to_string()));
            }
        }
        for (at, half) in &self.boxes {
            if let Some(h) = box_top(at, *half, p.x, p.z, top, bottom) {
                let l = at.compute_affine().inverse().transform_point3(p);
                if (l.abs() - *half).max_element() < 0.0 {
                    deeper(h - p.y, "floor");
                } else if h <= p.y + 1e-4 {
                    below.push((h, "floor".to_string()));
                }
            }
        }
        for (i, m) in self.models.iter().enumerate() {
            if i == skip || p.x < m.lo.x || p.x > m.hi.x || p.z < m.lo.z || p.z > m.hi.z {
                continue;
            }
            if m.hi.y < bottom || m.lo.y > top {
                continue;
            }
            for t in &m.tris {
                // Only faces looking up hold anything.
                let n = (t[1] - t[0]).cross(t[2] - t[0]);
                if n.y <= 0.0 {
                    continue;
                }
                if let Some(h) = tri_height(t, p.x, p.z) {
                    // A thing may bed in a little (a counter top into its
                    // cabinets).
                    if h >= bottom && h <= p.y + SINK_FLOOR {
                        below.push((h, m.name.clone()));
                    }
                }
            }
        }
        (below, inside)
    }
}

/// One fault: where, what, and by how much.
#[derive(Clone, Debug)]
pub struct Finding {
    pub map: String,
    pub site: String,
    pub model: String,
    pub at: Vec3,
    /// + floats this far, - sinks this far (m).
    pub gap: f32,
    pub on: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let how = if self.gap > 0.0 { "floats" } else { "sinks" };
        write!(
            f,
            "{} {}: {} {how} {:.0} mm {} {} at ({:.2}, {:.2}, {:.2})",
            self.map,
            self.site,
            self.model,
            self.gap.abs() * 1000.0,
            if self.gap > 0.0 { "over" } else { "into" },
            self.on,
            self.at.x,
            self.at.y,
            self.at.z
        )
    }
}

/// A site's feet against what's under them.
pub fn check_site(zone: &Zone, map: &str, site: &Site) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, m) in site.models.iter().enumerate() {
        if exempt(&m.name) {
            continue;
        }
        // Each foot: its points' gaps to what's under them, and how deep
        // the deepest is in something.
        let mut feet = Vec::new();
        for (pts, wide) in &m.shape.feet {
            let mut gaps = Vec::new();
            let mut sunk: Option<(f32, Vec3, String)> = None;
            for p in pts {
                let w = m.at.transform_point(*p);
                if site.terrain && zone.hm.water_level([w.x, w.z]).is_some() {
                    continue;
                }
                let (below, inside) = site.under(zone, i, w);
                if let Some((d, on)) = inside {
                    if sunk.as_ref().is_none_or(|s| d > s.0) {
                        sunk = Some((d, w, on));
                    }
                }
                if let Some((h, on)) = below.into_iter().max_by(|a, b| a.0.total_cmp(&b.0)) {
                    gaps.push((w.y - h, w, on));
                }
            }
            feet.push((gaps, sunk, *wide));
        }
        // Hung on something, or over nothing: not standing.
        let lowest = feet
            .iter()
            .filter_map(|(g, s, _)| {
                if s.is_some() {
                    Some(0.0)
                } else {
                    g.iter().map(|x| x.0).reduce(f32::min)
                }
            })
            .fold(f32::MAX, f32::min);
        if lowest > HUNG {
            continue;
        }
        let mut worst: Option<Finding> = None;
        for (gaps, sunk, wide) in &feet {
            let fault = if let Some((d, w, on)) = sunk {
                let limit = if !wide || site.name.contains("_drowned") {
                    // (A drowned house's things stand in its silt.)
                    SINK_SLENDER
                } else if on == "ground" && dug_in(&m.name) {
                    0.15
                } else if on == "ground" {
                    SINK_WIDE
                } else {
                    SINK_FLOOR
                };
                (*d > limit).then(|| (-*d, *w, on.clone()))
            } else {
                // A foot touches if any of its points does, or (a wide
                // one) if anything under it comes up to it: a counter
                // top rests on its cabinets' thin sides.
                gaps.iter()
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .filter(|g| g.0 > FLOAT)
                    .filter(|_| !(*wide && site.under_area(i, gaps)))
                    .cloned()
            };
            if let Some((gap, at, on)) = fault {
                if worst.as_ref().is_none_or(|f| gap.abs() > f.gap.abs()) {
                    worst = Some(Finding {
                        map: map.to_string(),
                        site: site.name.clone(),
                        model: m.name.clone(),
                        at,
                        gap,
                        on,
                    });
                }
            }
        }
        out.extend(worst);
    }
    out
}

/// Model shapes by name, loaded once.
#[derive(Default)]
pub struct Shapes(HashMap<String, Option<Arc<Shape3>>>);

impl Shapes {
    pub fn get(&mut self, path: &str) -> Option<Arc<Shape3>> {
        self.0
            .entry(path.to_string())
            .or_insert_with(|| shape(&crate::asset_root_path().join(path)).map(Arc::new))
            .clone()
    }

    fn prop(&mut self, name: &str) -> Option<Arc<Shape3>> {
        self.get(&crate::render::world_props::path(name))
    }
}

/// A prefab (building or scene) as the game puts it up at `t`, with what
/// its people left and its yard.
pub fn building_site(
    zone: &Zone,
    shapes: &mut Shapes,
    id: &str,
    tag: &str,
    t: Transform,
) -> Option<Site> {
    let mut geo = buildings::prefab(id)?;
    crate::world::scenes::settle(&mut geo, zone, id, &t);
    let models = crate::world::clutter::dress(&zone.map, id, tag, &t, false);
    if !models.is_empty() {
        let n = geo.pieces.len();
        geo.props.push(buildings::PropDef {
            models,
            pieces: n..n,
            inside: true,
            shadow: false,
            lods: None,
        });
    }
    let hm = &zone.hm;
    let ground = |p: Vec2| {
        let p = [p.x, p.y];
        hm.water_level(p).is_none().then(|| hm.height(p))
    };
    let models = crate::world::clutter::yard(&zone.map, id, tag, &t, &ground);
    if !models.is_empty() {
        let n = geo.pieces.len();
        geo.props.push(buildings::PropDef {
            models,
            pieces: n..n,
            inside: false,
            shadow: true,
            lods: None,
        });
    }
    let mut replaced = vec![false; geo.pieces.len()];
    for d in &geo.props {
        for i in d.pieces.clone() {
            replaced[i] = true;
        }
    }
    let boxes = geo
        .pieces
        .iter()
        .enumerate()
        .filter(|(i, _)| !replaced[*i])
        .filter_map(|(_, p)| match p.shape {
            Shape::Box(s) => Some((t * p.at, s * 0.5)),
            Shape::Cylinder(r, h) => Some((t * p.at, Vec3::new(r, h * 0.5, r))),
            Shape::Prism(_) => None,
        })
        .collect();
    let mut placed = Vec::new();
    for d in &geo.props {
        for (name, at) in &d.models {
            // A wreck's or prefab's one node (`wreck_lada#body`).
            if let Some(s) = shapes.prop(name) {
                placed.push(Placed::new(name, t * *at, s));
            }
        }
    }
    let site = crate::world::underground::ids().any(|u| u == id);
    Some(Site {
        name: if tag.is_empty() {
            id.to_string()
        } else {
            format!("{id} ({tag})")
        },
        boxes,
        models: placed,
        terrain: !site,
    })
}

/// Every site a valley puts up, as the game places it.
pub fn sites(map: &str, shapes: &mut Shapes) -> (Zone, Vec<Site>) {
    let zone = Zone::load(map);
    let mut out = Vec::new();
    for p in &zone.map.placements {
        let t = buildings::placement_transform(&zone, p);
        if let Some(s) = building_site(&zone, shapes, &p.prefab, &p.tag, t) {
            out.push(s);
        }
    }
    for (id, scene) in crate::world::scenes::of(map) {
        if !crate::world::scenes::stands(&zone, scene) {
            continue;
        }
        let t = crate::world::scenes::pose(&zone, scene);
        if let Some(mut s) = building_site(&zone, shapes, &id, "", t) {
            s.name = format!("scene {}", scene.name);
            out.push(s);
        }
    }
    // The camp, in a valley (`camp::spawn_camp`, `render::props::dress_camp`).
    if map != crate::camp::TRAINING_YARD {
        let pose = crate::camp::camp_pose(&zone);
        let mut models = Vec::new();
        for (name, at) in crate::camp::models(Some(&zone), &pose) {
            if let Some(s) = shapes.prop(name) {
                models.push(Placed::new(name, pose * at, s));
            }
        }
        out.push(Site {
            name: "camp".into(),
            boxes: Vec::new(),
            models,
            terrain: true,
        });
    }
    (zone, out)
}

/// The truck's wheels on the ground, wherever it parks: (where, gap).
pub fn truck_wheels(zone: &Zone) -> Vec<(String, Vec3, f32)> {
    let radius = truck_wheel_radius();
    let mut poses = vec![("base".to_string(), crate::raid::truck::base_pose(zone))];
    let rvs = zone
        .map
        .markers_of(&extraction_core::map::MarkerKind::Rendezvous)
        .count();
    for rv in 0..rvs {
        poses.push((
            format!("rendezvous {rv}"),
            crate::raid::truck::rv_pose(zone, rv),
        ));
    }
    let mut out = Vec::new();
    for (name, pose) in poses {
        for (i, w) in crate::raid::truck::WHEELS.iter().enumerate() {
            // The tyre's lowest point, round its contact patch, the wheel
            // hanging on its springs (`render::props::droop_wheels`).
            let droop = crate::raid::truck::wheel_droop(zone, &pose, i);
            let mut best = f32::MAX;
            let mut at = Vec3::ZERO;
            for k in -2..=2 {
                let c = *w + Vec3::new(0.0, -radius - droop, k as f32 * 0.05);
                let p = pose.transform_point(c);
                let gap = p.y - zone.ground(p.x, p.z);
                if gap < best {
                    best = gap;
                    at = p;
                }
            }
            out.push((format!("{name} wheel {i}"), at, best));
        }
    }
    out
}

/// The truck model's road wheel radius (its `wheel` node), or the
/// gameplay one.
pub fn truck_wheel_radius() -> f32 {
    let path = crate::asset_root_path().join("models/vehicles/truck.glb");
    glb::read(&path)
        .ok()
        .and_then(|i| {
            i.nodes
                .iter()
                .find(|n| n.name == "wheel")
                .map(|n| (n.hi.y - n.lo.y) * 0.5)
        })
        .unwrap_or(0.55)
}

/// Every map the game ships.
pub fn maps() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(crate::asset_root_path().join("maps"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "ron").then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    out.sort();
    out
}

/// Every fault in one valley: props, then the truck's wheels.
pub fn findings(map: &str, shapes: &mut Shapes) -> Vec<String> {
    let (zone, sites) = sites(map, shapes);
    let mut out: Vec<String> = sites
        .iter()
        .flat_map(|s| check_site(&zone, map, s))
        .map(|f| f.to_string())
        .collect();
    for (what, at, gap) in truck_wheels(&zone) {
        if gap > FLOAT || gap < -SINK_WIDE {
            let how = if gap > 0.0 { "floats" } else { "sinks" };
            out.push(format!(
                "{map} truck: {what} {how} {:.0} mm at ({:.1}, {:.1}, {:.1})",
                gap.abs() * 1000.0,
                at.x,
                at.y,
                at.z
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_foot_is_held_to_the_surface_under_it() {
        let t = [
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];
        assert_eq!(tri_height(&t, 0.2, 0.2), Some(1.0));
        assert_eq!(tri_height(&t, 0.8, 0.8), None);
        // A floor slab 0.1 thick, its top at y = 0.
        let slab = (
            Transform::from_xyz(0.0, -0.05, 0.0),
            Vec3::new(1.0, 0.05, 1.0),
        );
        let h = box_top(&slab.0, slab.1, 0.3, 0.3, 0.5, -1.0).unwrap();
        assert!(h.abs() < 1e-5, "{h}");
        assert!(box_top(&slab.0, slab.1, 3.0, 0.3, 0.5, -1.0).is_none());
    }

    #[test]
    fn feet_are_the_lowest_clusters() {
        // Four legs at the corners of a 0.6 m square, and a top.
        let mut pts = Vec::new();
        for (x, z) in [(0.0, 0.0), (0.6, 0.0), (0.0, 0.6), (0.6, 0.6)] {
            pts.push(Vec3::new(x, 0.0, z));
            pts.push(Vec3::new(x + 0.03, 0.0, z + 0.03));
        }
        let f = clusters(&pts, 0.03);
        assert_eq!(f.len(), 4);
        assert!(f.iter().all(|(_, wide)| !wide));
    }

    /// Every placed model in every valley and the yard stands on what's
    /// under it: nothing floating, nothing sunk. Prints each fault.
    #[test]
    fn placed_props_stand_on_what_is_under_them() {
        let mut shapes = Shapes::default();
        let mut bad = Vec::new();
        let mut seen = Vec::new();
        for map in maps() {
            let all = findings(&map, &mut shapes);
            println!("{map}: {} faults", all.len());
            for f in all {
                println!("    {f}");
                if !KNOWN.iter().any(|k| f.contains(k)) {
                    bad.push(f.clone());
                }
                seen.push(f);
            }
        }
        // Lines in KNOWN nothing matches any more: take them out.
        for k in KNOWN {
            if !seen.iter().any(|l| l.contains(k)) {
                println!("fixed, out of KNOWN: {k}");
            }
        }
        assert!(
            bad.is_empty(),
            "props off what's under them:\n{}",
            bad.join("\n")
        );
    }

    /// Faults waiting on their owners, by text in their line.
    const KNOWN: &[&str] = &[
        // Rime's quarantine gate stands on a slope; its posts' footings go
        // 0.4 m into the high side (Map design: level its spot).
        "quarantine_gate: gate_post",
        // What the first rounds left (Models, next round): a ledger with
        // no table under it in the pump stations, lying remains in three
        // scenes, a bed and a tub a little high, a barrel on the ropeway
        // station's own floor.
        "pump_station (gauge_house): ledger_open",
        "pump_station: ledger_open",
        "scene The shelter door: remains_lying",
        "scene Last fire in the dead wood: remains_lying",
        "scene Farm barricade: remains_lying",
        "cabin_log: bed_name_mirek",
        "ore_mill (ore_mill): mine_tub",
        "ropeway_station: barrel",
    ];
}

#[cfg(test)]
mod poses {
    /// Prints where the camp and the truck stand in each valley, for
    /// shots (`-- --ignored --nocapture`).
    #[test]
    #[ignore = "prints poses"]
    fn print_camp_and_truck_poses() {
        for map in super::maps() {
            let zone = crate::world::Zone::load(&map);
            let truck = crate::raid::truck::base_pose(&zone);
            let at = zone
                .marker(&extraction_core::map::MarkerKind::TruckStop)
                .map_or([0.0, 0.0], |m| m.at);
            let level = crate::raid::truck::level_frame(&truck);
            let fall = |z: f32| {
                let p = level.transform_point(bevy::math::Vec3::new(0.0, 0.0, z));
                zone.ground(p.x, p.z)
            };
            print!(
                "{map}: truck {:.2?} (stop {at:?}), ground fall cab to tail {:.2} m",
                truck.translation,
                fall(-3.9) - fall(0.8)
            );
            if map != crate::camp::TRAINING_YARD {
                let camp = crate::camp::camp_pose(&zone);
                let (yaw, ..) = camp.rotation.to_euler(bevy::math::EulerRot::YXZ);
                print!(
                    ", camp {:.2?} yaw {:.0}",
                    camp.translation,
                    yaw.to_degrees()
                );
            }
            println!();
        }
    }
}

#[cfg(test)]
mod ladder {
    use bevy::math::Vec3;

    /// Wherever the truck stands on its wheels, the bed is in reach from
    /// the ground behind it: the lowest tread at most a step over it, or
    /// a ramp a body walks up from the tread down to it.
    #[test]
    fn the_trucks_steps_reach_the_ground_at_every_stop() {
        let mut bad = Vec::new();
        for map in super::maps() {
            let zone = crate::world::Zone::load(&map);
            let rvs = zone
                .map
                .markers_of(&extraction_core::map::MarkerKind::Rendezvous)
                .count();
            let poses = std::iter::once(("base".to_string(), crate::raid::truck::base_pose(&zone)))
                .chain((0..rvs).map(|i| {
                    (
                        format!("rendezvous {i}"),
                        crate::raid::truck::rv_pose(&zone, i),
                    )
                }));
            for (name, pose) in poses {
                // Steps and ramp stand level (`raid::truck::level_frame`).
                let level = crate::raid::truck::level_frame(&pose);
                let ground = |p: Vec3| {
                    let w = level.transform_point(p);
                    zone.ground(w.x, w.z) - level.translation.y
                };
                let edge = level
                    .compute_affine()
                    .inverse()
                    .transform_point3(pose.transform_point(Vec3::new(0.0, 0.37, 2.4)));
                match crate::raid::truck::ladder_steps(&zone, &pose).first() {
                    None => {
                        let rise = edge.y - ground(edge + Vec3::Z * 0.15);
                        if rise > 0.38 {
                            bad.push(format!("{map} {name}: the lowest tread {rise:.2} m up"));
                        }
                    }
                    Some((size, centre, tilt)) => {
                        let r = bevy::math::Quat::from_rotation_x(*tilt);
                        let foot = *centre + r * Vec3::new(0.0, size.y * 0.5, size.z * 0.5);
                        let gap = foot.y - ground(foot);
                        if tilt.tan() > 0.65 || gap.abs() > 0.12 {
                            bad.push(format!(
                                "{map} {name}: a ramp at {:.0} degrees, its foot {gap:.2} m off the ground",
                                tilt.to_degrees()
                            ));
                        }
                    }
                }
            }
        }
        assert!(bad.is_empty(), "the bed out of reach:\n{}", bad.join("\n"));
    }
}
