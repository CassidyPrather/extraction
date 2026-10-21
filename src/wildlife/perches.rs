//! Where birds can sit: the tops of things. Ridges, flat roofs, the tops of
//! masts, water towers, chimneys and ropeway pylons, boxcar roofs.
//!
//! Found from the buildings' own geometry (`world::buildings::prefab`, the
//! pieces the colliders and meshes are made of), not from physics: a ray
//! straight down onto each building, every [`GRID`] metres, solved against
//! every piece analytically (boxes, prisms as their collider boxes, and
//! cylinders). The first thing a ray meets from above is the top of the
//! building at that spot, so nothing is ever above a perch, and a perch
//! whose bird would poke into a neighbouring piece is dropped. Pure and
//! deterministic: the same map gives the same perches, headless or not.
//!
//! The same pieces are kept as [`Obstacles`], for birds in flight to stay
//! out of.

use bevy::math::Affine3A;
use bevy::prelude::*;

use crate::world::Zone;
use crate::world::buildings::{Shape, placement_transform, prefab};

/// Spacing of the downward rays over a building, metres.
pub const GRID: f32 = 0.5;
/// A perch is at least this far above the ground beside the building.
pub const MIN_HEIGHT: f32 = 2.4;
/// The surface under a perch is no steeper than this (its normal's y).
pub const MIN_UP: f32 = 0.55;
/// A sitting bird's body: this far round and up from its feet must be
/// clear of every other piece.
pub const BIRD_RADIUS: f32 = 0.14;
pub const BIRD_HEIGHT: f32 = 0.3;
/// Birds sit along a building's top: only spots within this of its
/// highest are kept.
pub const TOP_BAND: f32 = 1.4;
/// Perches on one building are at least this far apart.
pub const SPACING: f32 = 0.42;
/// At most this many on one building.
pub const MAX_PER_SITE: usize = 48;

/// One solid piece of a building in world space: a box (half extents in
/// its own frame) or an upright-in-its-frame cylinder.
#[derive(Clone, Copy, Debug)]
pub struct Solid {
    pub to_local: Affine3A,
    pub to_world: Affine3A,
    pub kind: SolidKind,
    /// World bounds, min and max.
    pub lo: Vec3,
    pub hi: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub enum SolidKind {
    Box(Vec3),
    /// Radius, half height (along local Y).
    Cylinder(f32, f32),
}

impl Solid {
    fn new(world: Transform, kind: SolidKind) -> Self {
        let to_world = world.compute_affine();
        let mut s = Self {
            to_local: to_world.inverse(),
            to_world,
            kind,
            lo: Vec3::ZERO,
            hi: Vec3::ZERO,
        };
        (s.lo, s.hi) = s.bounds();
        s
    }

    /// Is `p` inside (grown by `pad` metres)?
    pub fn contains(&self, p: Vec3, pad: f32) -> bool {
        let l = self.to_local.transform_point3(p);
        match self.kind {
            SolidKind::Box(h) => {
                l.x.abs() <= h.x + pad && l.y.abs() <= h.y + pad && l.z.abs() <= h.z + pad
            }
            SolidKind::Cylinder(r, h) => {
                l.y.abs() <= h + pad && Vec2::new(l.x, l.z).length() <= r + pad
            }
        }
    }

    /// Where a ray first enters it: distance along `dir` (in units of
    /// `dir`'s length) and the world normal there.
    pub fn ray(&self, from: Vec3, dir: Vec3) -> Option<(f32, Vec3)> {
        let o = self.to_local.transform_point3(from);
        let d = self.to_local.transform_vector3(dir);
        let (t, n) = match self.kind {
            SolidKind::Box(h) => ray_box(o, d, h)?,
            SolidKind::Cylinder(r, h) => ray_cylinder(o, d, r, h)?,
        };
        let n = self.to_world.transform_vector3(n).normalize_or_zero();
        Some((t, n))
    }

    /// The world-space bounds (a box round its corners).
    fn bounds(&self) -> (Vec3, Vec3) {
        let h = match self.kind {
            SolidKind::Box(h) => h,
            SolidKind::Cylinder(r, h) => Vec3::new(r, h, r),
        };
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for i in 0..8 {
            let c = Vec3::new(
                if i & 1 == 0 { -h.x } else { h.x },
                if i & 2 == 0 { -h.y } else { h.y },
                if i & 4 == 0 { -h.z } else { h.z },
            );
            let w = self.to_world.transform_point3(c);
            lo = lo.min(w);
            hi = hi.max(w);
        }
        (lo, hi)
    }
}

/// Slab test; the entry face's normal.
fn ray_box(o: Vec3, d: Vec3, h: Vec3) -> Option<(f32, Vec3)> {
    let mut t0 = f32::MIN;
    let mut t1 = f32::MAX;
    let mut n = Vec3::ZERO;
    for a in 0..3 {
        let (oa, da, ha) = (o[a], d[a], h[a]);
        if da.abs() < 1e-9 {
            if oa.abs() > ha {
                return None;
            }
            continue;
        }
        let (mut near, mut far) = ((-ha - oa) / da, (ha - oa) / da);
        let mut sign = -1.0;
        if near > far {
            std::mem::swap(&mut near, &mut far);
            sign = 1.0;
        }
        if near > t0 {
            t0 = near;
            n = Vec3::ZERO;
            n[a] = sign;
        }
        t1 = t1.min(far);
        if t0 > t1 {
            return None;
        }
    }
    (t0 >= 0.0).then_some((t0, n))
}

/// A cylinder about local Y: the side, then the caps.
fn ray_cylinder(o: Vec3, d: Vec3, r: f32, h: f32) -> Option<(f32, Vec3)> {
    let mut best: Option<(f32, Vec3)> = None;
    let mut take = |t: f32, n: Vec3| {
        if t >= 0.0 && best.is_none_or(|(b, _)| t < b) {
            best = Some((t, n));
        }
    };
    // Caps.
    if d.y.abs() > 1e-9 {
        for (y, ny) in [(h, 1.0), (-h, -1.0)] {
            let t = (y - o.y) / d.y;
            let p = o + d * t;
            if p.x * p.x + p.z * p.z <= r * r {
                take(t, Vec3::Y * ny);
            }
        }
    }
    // Side.
    let a = d.x * d.x + d.z * d.z;
    if a > 1e-12 {
        let b = 2.0 * (o.x * d.x + o.z * d.z);
        let c = o.x * o.x + o.z * o.z - r * r;
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let t = (-b - disc.sqrt()) / (2.0 * a);
            let p = o + d * t;
            if p.y.abs() <= h {
                take(t, Vec3::new(p.x, 0.0, p.z).normalize_or_zero());
            }
        }
    }
    best
}

/// Everything built, as solids, with a coarse grid over them for quick
/// "what's near here" (birds in flight, perch clearance).
#[derive(Resource, Default, Clone)]
pub struct Obstacles {
    pub solids: Vec<Solid>,
    /// Bounds of each solid, grown by a little.
    bounds: Vec<(Vec3, Vec3)>,
    cells: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

/// The obstacle grid's cell, metres.
const CELL: f32 = 8.0;

fn cell_of(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL).floor() as i32, (z / CELL).floor() as i32)
}

impl Obstacles {
    pub fn push(&mut self, s: Solid) {
        let (lo, hi) = (s.lo, s.hi);
        let i = self.solids.len() as u32;
        self.solids.push(s);
        self.bounds
            .push((lo - Vec3::splat(0.5), hi + Vec3::splat(0.5)));
        let (a, b) = (cell_of(lo.x, lo.z), cell_of(hi.x, hi.z));
        for cx in a.0..=b.0 {
            for cz in a.1..=b.1 {
                self.cells.entry((cx, cz)).or_default().push(i);
            }
        }
    }

    /// Solids whose bounds take in `p`.
    pub fn near(&self, p: Vec3) -> impl Iterator<Item = &Solid> + '_ {
        self.cells
            .get(&cell_of(p.x, p.z))
            .into_iter()
            .flatten()
            .filter(move |&&i| {
                let (lo, hi) = self.bounds[i as usize];
                p.cmpge(lo).all() && p.cmple(hi).all()
            })
            .map(|&i| &self.solids[i as usize])
    }

    /// The top of anything built within `r` (horizontally) of `p`, if any.
    pub fn top_near(&self, p: Vec3, r: f32) -> Option<f32> {
        let (a, b) = (cell_of(p.x - r, p.z - r), cell_of(p.x + r, p.z + r));
        let mut top: Option<f32> = None;
        for cx in a.0..=b.0 {
            for cz in a.1..=b.1 {
                for &i in self.cells.get(&(cx, cz)).into_iter().flatten() {
                    let s = &self.solids[i as usize];
                    if p.x >= s.lo.x - r
                        && p.x <= s.hi.x + r
                        && p.z >= s.lo.z - r
                        && p.z <= s.hi.z + r
                    {
                        top = Some(top.map_or(s.hi.y, |t: f32| t.max(s.hi.y)));
                    }
                }
            }
        }
        top
    }

    /// Is `p` inside anything built (grown by `pad`)?
    pub fn inside(&self, p: Vec3, pad: f32) -> bool {
        self.near(p).any(|s| s.contains(p, pad))
    }
}

/// A building's perches.
#[derive(Clone, Debug)]
pub struct PerchSite {
    /// The placement's prefab and where it stands.
    pub prefab: String,
    pub centre: Vec3,
    /// Where a bird's feet go, and the way along the top it faces (a
    /// ridge's line, so birds sit in a row along it).
    pub spots: Vec<(Vec3, f32)>,
    /// Shallow water, for a heron to stand in ([`wading`]), not the top of
    /// anything.
    pub wading: bool,
}

impl PerchSite {
    pub fn top(&self) -> f32 {
        self.spots.iter().map(|s| s.0.y).fold(f32::MIN, f32::max)
    }
}

/// Every building's solids, and the perches on top of them.
#[derive(Resource, Default, Clone)]
pub struct Perches {
    pub sites: Vec<PerchSite>,
}

/// One placement's solids, in world space.
fn solids_of(zone: &Zone, p: &extraction_core::map::Placement) -> Vec<Solid> {
    let Some(geo) = prefab(&p.prefab) else {
        return Vec::new();
    };
    let root = placement_transform(zone, p);
    geo.pieces
        .iter()
        .map(|piece| {
            let world = root * piece.at;
            let kind = match piece.shape {
                Shape::Box(s) => SolidKind::Box(s * 0.5),
                Shape::Prism(s) => SolidKind::Box(Vec3::new(s.x, s.y * 0.5, s.z) * 0.5),
                Shape::Cylinder(r, h) => SolidKind::Cylinder(r, h * 0.5),
            };
            Solid::new(world, kind)
        })
        .collect()
}

/// The first solid a ray straight down from `top` meets: where, and the
/// surface's normal.
fn down(solids: &[Solid], top: Vec3) -> Option<(Vec3, Vec3)> {
    solids
        .iter()
        .filter(|s| top.x >= s.lo.x && top.x <= s.hi.x && top.z >= s.lo.z && top.z <= s.hi.z)
        .filter_map(|s| s.ray(top, Vec3::NEG_Y))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(t, n)| (top - Vec3::Y * t, n))
}

/// Perches on one placement's solids.
fn site_perches(zone: &Zone, prefab_id: &str, solids: &[Solid]) -> Option<PerchSite> {
    if solids.is_empty() {
        return None;
    }
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for s in solids {
        lo = lo.min(s.lo);
        hi = hi.max(s.hi);
    }
    // Every spot a bird could stand: top-most, flat enough, high enough...
    let mut found: Vec<(Vec3, Vec3)> = Vec::new();
    let nx = ((hi.x - lo.x) / GRID).ceil() as i32;
    let nz = ((hi.z - lo.z) / GRID).ceil() as i32;
    for i in 0..=nx {
        for j in 0..=nz {
            let x = lo.x + i as f32 * GRID;
            let z = lo.z + j as f32 * GRID;
            let Some((p, n)) = down(solids, Vec3::new(x, hi.y + 1.0, z)) else {
                continue;
            };
            if n.y >= MIN_UP && p.y >= zone.ground(x, z) + MIN_HEIGHT {
                found.push((p, n));
            }
        }
    }
    // ...along the top...
    let top = found.iter().map(|f| f.0.y).fold(f32::MIN, f32::max);
    found.retain(|f| f.0.y >= top - TOP_BAND);
    // ...with room for the bird: its body clear of every piece, and the
    // spot not overhanging an edge (a ray beside it hits about as high: a
    // mast's rim, not the air beside it).
    let near: Vec<&Solid> = solids
        .iter()
        .filter(|s| s.hi.y >= top - TOP_BAND - 0.5)
        .collect();
    let sides = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z];
    found.retain(|&(p, _)| {
        let body = p + Vec3::Y * (BIRD_HEIGHT * 0.5 + 0.03);
        let clear = std::iter::once(Vec3::ZERO)
            .chain(sides.iter().map(|&o| o * BIRD_RADIUS))
            .all(|o| !near.iter().any(|s| s.contains(body + o, 0.0)));
        clear
            && sides.iter().all(|&o| {
                let q = Vec3::new(p.x, hi.y + 1.0, p.z) + o * (BIRD_RADIUS * 0.6);
                down(solids, q).is_some_and(|(h, _)| (h.y - p.y).abs() < 0.25)
            })
    });
    // Highest first; then far enough from those already taken.
    found.sort_by(|a, b| {
        b.0.y
            .total_cmp(&a.0.y)
            .then(a.0.x.total_cmp(&b.0.x))
            .then(a.0.z.total_cmp(&b.0.z))
    });
    let mut spots: Vec<(Vec3, Vec3)> = Vec::new();
    for f in found {
        if spots.len() >= MAX_PER_SITE {
            break;
        }
        if spots.iter().all(|s| s.0.distance(f.0) >= SPACING) {
            spots.push(f);
        }
    }
    if spots.len() < 4 {
        return None;
    }
    let centre = spots.iter().map(|s| s.0).sum::<Vec3>() / spots.len() as f32;
    // Facing: across the slope (along a ridge), or the building's own
    // long axis on a flat top.
    let spots = spots
        .into_iter()
        .map(|(p, n)| {
            let along = if Vec2::new(n.x, n.z).length() > 0.1 {
                Vec2::new(-n.z, n.x)
            } else {
                let d = (hi - lo).xz();
                if d.x > d.y { Vec2::X } else { Vec2::Y }
            };
            (p, along.y.atan2(along.x))
        })
        .collect();
    Some(PerchSite {
        prefab: prefab_id.to_string(),
        centre,
        spots,
        wading: false,
    })
}

/// The map's obstacles and perches (one site per building that has any).
pub fn survey(zone: &Zone) -> (Obstacles, Perches) {
    let mut obstacles = Obstacles::default();
    let mut perches = Perches::default();
    for p in &zone.map.placements {
        // Every generated prefab (buildings, the dam, scenes'), except
        // underground sites: no place for birds, and the biggest of all.
        if crate::world::underground::ids().any(|id| id == p.prefab) {
            continue;
        }
        let solids = solids_of(zone, p);
        if let Some(site) = site_perches(zone, &p.prefab, &solids) {
            perches.sites.push(site);
        }
        for s in solids {
            obstacles.push(s);
        }
    }
    (obstacles, perches)
}

/// Water this deep is for wading in (m).
pub const WADE: (f32, f32) = (0.08, 0.32);
/// Wading spots are gathered in cells this big, each a site...
pub const WADE_CELL: f32 = 24.0;
/// ...if it has at least this many spots, this far apart.
pub const WADE_SPOTS: usize = 3;
pub const WADE_SPACING: f32 = 1.5;

/// Wading sites: shallow water at the margins of the valley's water, the
/// bed under a heron's feet. Sampled every metre and a half over the map.
pub fn wading(zone: &Zone) -> Vec<PerchSite> {
    let half = zone.map.size * 0.5;
    let mut cells: std::collections::BTreeMap<(i32, i32), Vec<(Vec3, f32)>> = Default::default();
    let n = (zone.map.size / WADE_SPACING) as i32;
    for i in 0..n {
        for j in 0..n {
            let x = -half + i as f32 * WADE_SPACING;
            let z = -half + j as f32 * WADE_SPACING;
            let Some(top) = zone.hm.water_level([x, z]) else {
                continue;
            };
            let bed = zone.ground(x, z);
            let d = top - bed;
            if !(WADE.0..=WADE.1).contains(&d) {
                continue;
            }
            let cell = (
                (x / WADE_CELL).floor() as i32,
                (z / WADE_CELL).floor() as i32,
            );
            // Facing along the shore, more or less: across the slope.
            let yaw = ((i * 7 + j * 13) % 16) as f32 / 16.0 * std::f32::consts::TAU;
            cells
                .entry(cell)
                .or_default()
                .push((Vec3::new(x, bed, z), yaw));
        }
    }
    cells
        .into_values()
        .filter(|spots| spots.len() >= WADE_SPOTS)
        .map(|spots| {
            let centre = spots.iter().map(|s| s.0).sum::<Vec3>() / spots.len() as f32;
            PerchSite {
                prefab: "shallows".into(),
                centre,
                spots,
                wading: true,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rays_meet_boxes_and_cylinders_from_above() {
        let b = Solid::new(
            Transform::from_xyz(0.0, 2.0, 0.0),
            SolidKind::Box(Vec3::new(1.0, 0.5, 1.0)),
        );
        let (t, n) = b.ray(Vec3::new(0.2, 10.0, 0.3), Vec3::NEG_Y).unwrap();
        assert!((t - 7.5).abs() < 1e-4 && n.y > 0.99, "{t} {n}");
        assert!(b.ray(Vec3::new(1.2, 10.0, 0.0), Vec3::NEG_Y).is_none());
        // A slab tilted 30 degrees: hit lower down its slope, normal tilted.
        let tilted = Solid::new(
            Transform::from_xyz(0.0, 2.0, 0.0).with_rotation(Quat::from_rotation_z(0.5)),
            SolidKind::Box(Vec3::new(2.0, 0.1, 1.0)),
        );
        let (_, n) = tilted.ray(Vec3::new(1.0, 10.0, 0.0), Vec3::NEG_Y).unwrap();
        assert!((n.y - 0.5f32.cos()).abs() < 1e-3, "{n}");
        let c = Solid::new(
            Transform::from_xyz(5.0, 4.0, 0.0),
            SolidKind::Cylinder(0.5, 4.0),
        );
        let (t, n) = c.ray(Vec3::new(5.1, 20.0, 0.1), Vec3::NEG_Y).unwrap();
        assert!((t - 12.0).abs() < 1e-4 && n.y > 0.99);
        // Sideways into the side.
        let (t, n) = c.ray(Vec3::new(0.0, 4.0, 0.0), Vec3::X).unwrap();
        assert!((t - 4.5).abs() < 1e-4 && n.x < -0.99, "{t} {n}");
        assert!(c.contains(Vec3::new(5.0, 7.9, 0.0), 0.0));
        assert!(!c.contains(Vec3::new(5.0, 8.1, 0.0), 0.0));
    }

    /// Every valley has somewhere for the crows: dozens of buildings with
    /// perches, the tall things among them, and no perch inside or under
    /// anything built, or below the ground.
    #[test]
    fn every_valley_has_perches_on_top_of_things() {
        for map in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let zone = Zone::load(map);
            let (obstacles, perches) = survey(&zone);
            assert!(
                perches.sites.len() >= 8,
                "{map}: {} perch sites",
                perches.sites.len()
            );
            let spots: usize = perches.sites.iter().map(|s| s.spots.len()).sum();
            assert!(spots >= 150, "{map}: {spots} perches");
            for site in &perches.sites {
                for &(p, _) in &site.spots {
                    let body = p + Vec3::Y * (BIRD_HEIGHT * 0.5 + 0.03);
                    assert!(
                        !obstacles.inside(body, 0.0),
                        "{map}: a perch on the {} is inside something at {p}",
                        site.prefab
                    );
                    // Nothing overhead: straight up is clear.
                    let up = obstacles
                        .solids
                        .iter()
                        .filter_map(|s| s.ray(p + Vec3::Y * 0.05, Vec3::Y))
                        .count();
                    assert_eq!(up, 0, "{map}: a perch under something at {p}");
                    assert!(p.y > zone.ground(p.x, p.z) + MIN_HEIGHT - 0.01);
                }
            }
        }
    }
}
