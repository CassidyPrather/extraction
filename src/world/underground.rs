//! Underground sites, built from their layouts
//! (`extraction_core::underground`, `assets/underground/*.ron`).
//!
//! A site is a building like any other ([`super::buildings::prefab`]
//! returns its [`Geo`]): colliders, doors, loot spots and the merged
//! meshes all come the usual way. This module turns the layout's raster
//! into pieces (walls on every edge between open cells and rock, lintels
//! where ceilings step, floor and ceiling slabs, stairs with their kiosks
//! on the surface), and on the client lights the emergency lamps and
//! takes the daylight away as you go down.
//!
//! The ground is opened over each stairwell (`Heightmap::holes`): the
//! heightfield collider and the terrain mesh leave those cells out, and
//! the kiosk's walls and apron cover the edge.

use bevy::prelude::*;
use extraction_core::underground::{
    self as ug, APRON, CEILING_RUN, Cell, KIOSK, Layout, PropKind, Raster, SLAB, STAIR_HALF,
    STAIR_TOP, Stair, Style, TREAD, WALL,
};

use super::Zone;
use super::buildings::{Builder, Geo, Shape, door};
use crate::render::materials::Mat;

/// The kiosk's walls, floor to eaves.
const KIOSK_HEIGHT: f32 = 2.5;

/// The site ids, for tests and the prefab list.
pub fn ids() -> impl Iterator<Item = &'static str> {
    ug::SITES.iter().map(|(id, _)| *id)
}

/// A site's geometry, building space (= site space), built once.
pub fn geo(id: &str) -> Option<Geo> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static GEO: OnceLock<HashMap<&'static str, Geo>> = OnceLock::new();
    GEO.get_or_init(|| {
        ids()
            .map(|id| (id, build(ug::layout(id).unwrap(), ug::raster(id).unwrap())))
            .collect()
    })
    .get(id)
    .cloned()
}

/// What a style is built of: walls, floors, ceilings, the kiosk.
struct Mats {
    wall: Mat,
    floor: Mat,
    ceiling: Mat,
    kiosk: Mat,
    stair: Mat,
}

fn mats(style: Style) -> Mats {
    match style {
        Style::Bunker => Mats {
            wall: Mat::Plaster,
            floor: Mat::Concrete,
            ceiling: Mat::Concrete,
            kiosk: Mat::Concrete,
            stair: Mat::Concrete,
        },
        Style::Mine => Mats {
            wall: Mat::Boulder,
            floor: Mat::Boulder,
            ceiling: Mat::Boulder,
            kiosk: Mat::Boulder,
            stair: Mat::Concrete,
        },
        Style::Service => Mats {
            wall: Mat::Brick,
            floor: Mat::Concrete,
            ceiling: Mat::Concrete,
            kiosk: Mat::Brick,
            stair: Mat::Concrete,
        },
    }
}

/// An open cell's vertical extent for the walls round it: from below its
/// floor to the top of its roof slab (in a kiosk, up to the kiosk's
/// floor, where its own walls stand).
fn extent(r: &Raster, i: usize, j: usize) -> Option<(f32, f32)> {
    let c = r.centre(i, j);
    match r.get(i, j) {
        Cell::Rock => None,
        Cell::Room(_) | Cell::Hall(_) => {
            Some((r.floor - SLAB, r.floor + r.ceil[j * r.nx + i] + SLAB))
        }
        Cell::Stair(k) => {
            let k = k as usize;
            let s = &r.stairs[k];
            let z = r.portals[k].to_portal(c)[1];
            let (lo, top) = stair_band(s, z);
            Some((lo, top))
        }
    }
}

/// A stair's wall band at portal z: below the lowest tread of its ceiling
/// run, up to its soffit's top (in the kiosk, to the kiosk's floor).
fn stair_band(s: &Stair, z: f32) -> (f32, f32) {
    if z >= s.bottom {
        let c = s.ceiling_at(s.bottom - 0.01);
        return (s.floor_at(s.bottom) - SLAB, c + SLAB);
    }
    if z < STAIR_TOP {
        return (s.top - 0.6, s.top);
    }
    let k = (((z - STAIR_TOP) / TREAD).floor().max(0.0) as usize).min(s.n - 1);
    let run = k / CEILING_RUN;
    let last = ((run + 1) * CEILING_RUN).min(s.n);
    let lo = s.top - last as f32 * s.rise - 0.6;
    let hi = if z < KIOSK[1] - WALL * 0.5 {
        s.top
    } else {
        s.ceiling_at(z) + SLAB
    };
    (lo, hi)
}

/// The underside of the ceiling over an open cell, for lintels.
fn ceiling(r: &Raster, i: usize, j: usize) -> Option<f32> {
    match r.get(i, j) {
        Cell::Room(_) | Cell::Hall(_) => Some(r.floor + r.ceil[j * r.nx + i]),
        Cell::Stair(k) => {
            let k = k as usize;
            let s = &r.stairs[k];
            let z = r.portals[k].to_portal(r.centre(i, j))[1];
            (z >= s.bottom).then(|| s.ceiling_at(s.bottom - 0.01))
        }
        Cell::Rock => None,
    }
}

fn build(l: &'static Layout, r: &Raster) -> Geo {
    let m = mats(l.style);
    let mut b = Builder::default();
    walls(&mut b, r, &m);
    slabs(&mut b, r, &m);
    for k in 0..r.portals.len() {
        stair(&mut b, r, k, &m);
    }
    doors(&mut b, r, &m);
    dressing(&mut b, l, r);
    for room in &l.rooms {
        for p in &room.props {
            let at = Vec3::new(room.at[0] + p.at[0], r.floor, room.at[1] + p.at[1]);
            prop(&mut b, p.kind, at, -p.rot.to_radians(), p.loot.as_deref());
        }
    }
    b.finish()
}

/// Walls on the rock side of every edge between an open cell and rock,
/// merged into runs, and lintels where open cells' ceilings differ.
fn walls(b: &mut Builder, r: &Raster, m: &Mats) {
    let cell = ug::CELL;
    let doors = &r.doors;
    let at_door = |p: Vec2| {
        doors.iter().any(|d| {
            let along = Vec2::from(d.along);
            let off = p - Vec2::from(d.at);
            off.dot(along).abs() < d.width * 0.5 + 0.01 && off.perp_dot(along).abs() < 0.05
        })
    };
    // Horizontal lines (constant z) between rows j-1 and j, then vertical
    // lines (constant x) between columns i-1 and i: the same walk with
    // the axes swapped.
    for vertical in [false, true] {
        let (lines, len) = if vertical { (r.nx, r.nz) } else { (r.nz, r.nx) };
        // (a, b): the cells either side of edge s on line t; a below/left.
        let sides = |t: usize, s: usize| -> (Option<(usize, usize)>, Option<(usize, usize)>) {
            if vertical {
                ((t > 0).then(|| (t - 1, s)), (t < r.nx).then_some((t, s)))
            } else {
                ((t > 0).then(|| (s, t - 1)), (t < r.nz).then_some((s, t)))
            }
        };
        let open = |c: Option<(usize, usize)>| c.is_some_and(|(i, j)| r.get(i, j).open());
        // What an edge is: a wall with the open side below (-1) or above
        // (+1) and its band, open on both sides, or rock on both.
        #[derive(Clone, Copy, PartialEq)]
        enum Edge {
            Rock,
            Open,
            Wall(i8, (i32, i32)),
        }
        let edge = |t: usize, s: usize| -> Edge {
            let (a, c) = sides(t, s);
            match (open(a), open(c)) {
                (false, false) => Edge::Rock,
                (true, true) => Edge::Open,
                (true, false) => {
                    let (i, j) = a.unwrap();
                    let (lo, hi) = extent(r, i, j).unwrap();
                    Edge::Wall(
                        -1,
                        ((lo * 100.0).round() as i32, (hi * 100.0).round() as i32),
                    )
                }
                (false, true) => {
                    let (i, j) = c.unwrap();
                    let (lo, hi) = extent(r, i, j).unwrap();
                    Edge::Wall(
                        1,
                        ((lo * 100.0).round() as i32, (hi * 100.0).round() as i32),
                    )
                }
            }
        };
        for t in 0..=lines {
            let mut s = 0;
            while s < len {
                let e = edge(t, s);
                let Edge::Wall(side, band) = e else {
                    s += 1;
                    continue;
                };
                let s0 = s;
                while s < len && edge(t, s) == e {
                    s += 1;
                }
                // At each end: past a corner, run on over it; at a mouth
                // (open both sides beyond), lines along x stop at it and
                // lines along z stop a wall's thickness short, so the two
                // that meet there never share a face.
                let end = |beyond: Option<usize>| -> f32 {
                    match beyond.map(|q| edge(t, q)) {
                        None | Some(Edge::Rock) => WALL,
                        Some(Edge::Open) if vertical => -WALL,
                        _ => 0.0,
                    }
                };
                let a = s0 as f32 * cell - end(s0.checked_sub(1));
                let c = s as f32 * cell + end((s < len).then_some(s));
                if c - a < 0.01 {
                    continue;
                }
                let (lo, hi) = (band.0 as f32 / 100.0, band.1 as f32 / 100.0);
                let line = t as f32 * cell;
                // The wall's band: on the rock side of the line.
                let off = line - side as f32 * WALL * 0.5;
                let mid_along = (a + c) * 0.5;
                let (size, centre) = if vertical {
                    (
                        Vec3::new(WALL, hi - lo, c - a),
                        Vec3::new(r.origin[0] + off, (lo + hi) * 0.5, r.origin[1] + mid_along),
                    )
                } else {
                    (
                        Vec3::new(c - a, hi - lo, WALL),
                        Vec3::new(r.origin[0] + mid_along, (lo + hi) * 0.5, r.origin[1] + off),
                    )
                };
                let (i, j) = {
                    let (p, q) = sides(t, s0);
                    if side < 0 { p.unwrap() } else { q.unwrap() }
                };
                let mat = if matches!(r.get(i, j), Cell::Stair(_)) {
                    m.stair_wall()
                } else {
                    m.wall
                };
                b.bx(size, centre, mat);
            }
            // Lintels: open both sides, ceilings at different heights.
            let mut s = 0;
            while s < len {
                let lintel = |s: usize| -> Option<(i32, i32)> {
                    let (a, c) = sides(t, s);
                    let (a, c) = (a?, c?);
                    let (ca, cc) = (ceiling(r, a.0, a.1)?, ceiling(r, c.0, c.1)?);
                    if (ca - cc).abs() < 0.02 {
                        return None;
                    }
                    let mid = if vertical {
                        Vec2::new(t as f32 * cell, (s as f32 + 0.5) * cell)
                    } else {
                        Vec2::new((s as f32 + 0.5) * cell, t as f32 * cell)
                    } + Vec2::from(r.origin);
                    if at_door(mid) {
                        return None;
                    }
                    Some((
                        (ca.min(cc) * 100.0).round() as i32,
                        ((ca.max(cc) + SLAB) * 100.0).round() as i32,
                    ))
                };
                let Some(band) = lintel(s) else {
                    s += 1;
                    continue;
                };
                let s0 = s;
                while s < len && lintel(s) == Some(band) {
                    s += 1;
                }
                let (lo, hi) = (band.0 as f32 / 100.0, band.1 as f32 / 100.0);
                let line = t as f32 * cell;
                let (a, c) = (s0 as f32 * cell, s as f32 * cell);
                let (size, centre) = if vertical {
                    (
                        Vec3::new(WALL, hi - lo, c - a),
                        Vec3::new(
                            r.origin[0] + line,
                            (lo + hi) * 0.5,
                            r.origin[1] + (a + c) * 0.5,
                        ),
                    )
                } else {
                    (
                        Vec3::new(c - a, hi - lo, WALL),
                        Vec3::new(
                            r.origin[0] + (a + c) * 0.5,
                            (lo + hi) * 0.5,
                            r.origin[1] + line,
                        ),
                    )
                };
                b.bx(size, centre, m.wall);
            }
        }
    }
}

impl Mats {
    fn stair_wall(&self) -> Mat {
        self.stair
    }
}

/// Floor slabs under every level cell (rooms, corridors, stairs' feet),
/// ceiling slabs over rooms and corridors, merged into rectangles.
fn slabs(b: &mut Builder, r: &Raster, m: &Mats) {
    let level = |i: usize, j: usize| match r.get(i, j) {
        Cell::Room(_) | Cell::Hall(_) => Some(0),
        Cell::Stair(k) => {
            let k = k as usize;
            let z = r.portals[k].to_portal(r.centre(i, j))[1];
            (z >= r.stairs[k].bottom).then_some(0)
        }
        Cell::Rock => None,
    };
    for (lo, hi, _) in rects(r, level) {
        let (size, centre) = rect_box(r, lo, hi, r.floor - SLAB, r.floor);
        b.bx(size, centre, m.floor);
    }
    let roof = |i: usize, j: usize| {
        r.get(i, j)
            .flat()
            .then(|| (r.ceil[j * r.nx + i] * 100.0).round() as i32)
    };
    for (lo, hi, h) in rects(r, roof) {
        let y = r.floor + h as f32 / 100.0;
        let (size, centre) = rect_box(r, lo, hi, y, y + SLAB);
        b.bx(size, centre, m.ceiling);
    }
}

/// A box over cells `lo..=hi` between heights `y0` and `y1`.
fn rect_box(r: &Raster, lo: (usize, usize), hi: (usize, usize), y0: f32, y1: f32) -> (Vec3, Vec3) {
    let cell = ug::CELL;
    let a = Vec2::from(r.origin) + Vec2::new(lo.0 as f32, lo.1 as f32) * cell;
    let c = Vec2::from(r.origin) + Vec2::new(hi.0 as f32 + 1.0, hi.1 as f32 + 1.0) * cell;
    (
        Vec3::new(c.x - a.x, y1 - y0, c.y - a.y),
        Vec3::new((a.x + c.x) * 0.5, (y0 + y1) * 0.5, (a.y + c.y) * 0.5),
    )
}

/// Greedy rectangles of cells sharing a class.
fn rects(
    r: &Raster,
    class: impl Fn(usize, usize) -> Option<i32>,
) -> Vec<((usize, usize), (usize, usize), i32)> {
    let mut used = vec![false; r.nx * r.nz];
    let mut out = Vec::new();
    for j in 0..r.nz {
        for i in 0..r.nx {
            if used[j * r.nx + i] {
                continue;
            }
            let Some(c) = class(i, j) else { continue };
            let mut i1 = i;
            while i1 + 1 < r.nx && !used[j * r.nx + i1 + 1] && class(i1 + 1, j) == Some(c) {
                i1 += 1;
            }
            let mut j1 = j;
            'grow: while j1 + 1 < r.nz {
                for ii in i..=i1 {
                    if used[(j1 + 1) * r.nx + ii] || class(ii, j1 + 1) != Some(c) {
                        break 'grow;
                    }
                }
                j1 += 1;
            }
            for jj in j..=j1 {
                for ii in i..=i1 {
                    used[jj * r.nx + ii] = true;
                }
            }
            out.push(((i, j), (i1, j1), c));
        }
    }
    out
}

/// A stair: its kiosk on the surface (walls, roof, door, apron), the
/// steps (drawn) over a ramp (walked), the stepped soffit, and the fill
/// under the kiosk's back wall where the tunnel leaves it.
fn stair(b: &mut Builder, r: &Raster, k: usize, m: &Mats) {
    let p = &r.portals[k];
    let s = r.stairs[k];
    // Built in the portal's own frame, its ground at 0.
    let dy = p.y;
    let mut kb = Builder::default();
    let top = s.top - dy;
    let floor = s.floor_at(s.bottom) - dy;
    let ceiling_at = |z: f32| s.ceiling_at(z) - dy;
    // The kiosk.
    kb.shell(
        KIOSK[0] * 2.0,
        KIOSK[1] * 2.0,
        top,
        KIOSK_HEIGHT,
        WALL,
        m.kiosk,
        [&[door(KIOSK[0])], &[], &[], &[]],
    );
    kb.flat_roof(KIOSK[0] * 2.0, KIOSK[1] * 2.0, top + KIOSK_HEIGHT, m.kiosk);
    let (ox, oz) = (KIOSK[0] + WALL * 0.5, KIOSK[1] + WALL * 0.5);
    // Its floor inside the door, down into the ground.
    let land = STAIR_TOP + oz;
    kb.bx(
        Vec3::new(2.0 * ox, 0.6, land),
        Vec3::new(0.0, top - 0.3, -oz + land * 0.5),
        m.stair,
    );
    // The apron: four strips round the walls, over the hole's edge.
    let t = 0.35;
    for sz in [-1.0f32, 1.0] {
        kb.bx(
            Vec3::new(2.0 * (ox + APRON), t, APRON),
            Vec3::new(0.0, 0.05 - t * 0.5, sz * (oz + APRON * 0.5)),
            Mat::Concrete,
        );
        kb.bx(
            Vec3::new(APRON, t, 2.0 * oz),
            Vec3::new(sz * (ox + APRON * 0.5), 0.05 - t * 0.5, 0.0),
            Mat::Concrete,
        );
    }
    // Steps, drawn: each tread down to below the next, a worn steel
    // nosing on its edge.
    for n in 1..=s.n {
        let tread = top - n as f32 * s.rise;
        let z0 = STAIR_TOP + (n - 1) as f32 * TREAD;
        let depth = s.rise + 0.35;
        kb.piece(
            Shape::Box(Vec3::new(2.0 * STAIR_HALF, depth, TREAD)),
            Transform::from_xyz(0.0, tread - depth * 0.5, z0 + TREAD * 0.5),
            m.stair,
            false,
        );
        kb.piece(
            Shape::Box(Vec3::new(2.0 * STAIR_HALF - 0.1, 0.012, 0.05)),
            Transform::from_xyz(0.0, tread + 0.006, z0 + 0.03),
            Mat::DarkMetal,
            false,
        );
    }
    // The ramp the feet walk on: through the back edge of every tread, so
    // it lies inside the steps (never seen) and you go down smoothly.
    let a = Vec2::new(STAIR_TOP, top);
    let c = Vec2::new(s.bottom, floor);
    let dir = (c - a).normalize();
    let len = (c - a).length() + 0.3;
    let thick = 0.3;
    let pitch = dir.y.atan2(dir.x);
    let normal = Vec2::new(-dir.y, dir.x);
    let centre = (a + c) * 0.5 - normal * thick * 0.5;
    kb.piece(
        Shape::Box(Vec3::new(2.0 * STAIR_HALF, thick, len)),
        Transform::from_xyz(0.0, centre.y, centre.x).with_rotation(Quat::from_rotation_x(-pitch)),
        m.stair,
        true,
    );
    // The stepped soffit past the kiosk, a run of steps at a time; each
    // reaches up past the one before it, so the steps between are closed.
    let back = KIOSK[1] - WALL * 0.5;
    let mut z = back;
    while z < s.foot - 1e-3 {
        let (c, next) = if z >= s.bottom - 1e-3 {
            (ceiling_at(s.bottom - 0.01), s.foot)
        } else {
            let k = ((z + 0.01 - STAIR_TOP) / TREAD).floor() as usize;
            let run_end = STAIR_TOP + ((k / CEILING_RUN + 1) * CEILING_RUN).min(s.n) as f32 * TREAD;
            (ceiling_at(z + 0.01), run_end.min(s.foot))
        };
        let rise = s.rise * CEILING_RUN as f32 + SLAB + 0.05;
        kb.bx(
            Vec3::new(2.0 * ox, rise, next - z),
            Vec3::new(0.0, c + rise * 0.5, (z + next) * 0.5),
            m.stair,
        );
        z = next;
    }
    // Under the kiosk's back wall, down to the tunnel's soffit.
    let soffit = ceiling_at(back + 0.01);
    kb.bx(
        Vec3::new(2.0 * ox, top - soffit, WALL),
        Vec3::new(0.0, (top + soffit) * 0.5, KIOSK[1]),
        m.kiosk,
    );
    append(b, kb.finish(), p.at, dy, p.rot);
}

/// Appends geometry built in a portal's own frame.
fn append(b: &mut Builder, g: Geo, at: [f32; 2], y: f32, rot: f32) {
    let xf = Transform::from_xyz(at[0], y, at[1])
        .with_rotation(Quat::from_rotation_y(-rot.to_radians()));
    let base = b.geo.pieces.len();
    for mut p in g.pieces {
        p.at = xf * p.at;
        b.geo.pieces.push(p);
    }
    for mut d in g.doors {
        d.hinge = xf.transform_point(d.hinge);
        d.yaw -= rot.to_radians();
        b.geo.doors.push(d);
    }
    for mut s in g.loot {
        s.at = xf.transform_point(s.at);
        b.geo.loot.push(s);
    }
    for mut d in g.props {
        d.pieces = d.pieces.start + base..d.pieces.end + base;
        for (_, t) in &mut d.models {
            *t = xf * *t;
        }
        b.geo.props.push(d);
    }
}

/// Doors across corridors' mouths and stairs' feet: a wall on the line
/// with a door in it (or the opening, with a lintel over).
fn doors(b: &mut Builder, r: &Raster, m: &Mats) {
    for d in &r.doors {
        let along = Vec2::from(d.along);
        let at = Vec2::from(d.at);
        let a = at - along * d.width * 0.5;
        let c = at + along * d.width * 0.5;
        // Up to the higher roof either side.
        let normal = Vec2::new(-along.y, along.x);
        let roof = [1.0f32, -1.0]
            .iter()
            .filter_map(|s| {
                let q = at + normal * *s * ug::CELL * 0.5;
                r.cell_of(q.to_array()).and_then(|(i, j)| ceiling(r, i, j))
            })
            .fold(d.y + 2.3, f32::max);
        b.wall(
            a,
            c,
            d.y,
            roof - d.y + SLAB,
            WALL,
            m.wall,
            &[door(d.width * 0.5)],
        );
    }
}

/// Junk and fittings that make it a place: timber sets in the mine,
/// pipes along the service tunnel, cable trays in the shelter.
fn dressing(b: &mut Builder, l: &Layout, r: &Raster) {
    match l.style {
        Style::Mine => {
            // Timber sets across corridors every 2 m: posts in the
            // corners, a cap under the ceiling.
            for legs in &r.legs {
                for (lo, hi) in legs {
                    let (lo, hi) = (Vec2::from(*lo), Vec2::from(*hi));
                    let size = hi - lo;
                    let along_x = size.x > size.y;
                    let len = size.x.max(size.y);
                    let n = (len / 2.0).floor() as i32;
                    for k in 1..n {
                        let s = k as f32 * 2.0;
                        let (p, q) = if along_x {
                            (
                                Vec2::new(lo.x + s, lo.y + 0.08),
                                Vec2::new(lo.x + s, hi.y - 0.08),
                            )
                        } else {
                            (
                                Vec2::new(lo.x + 0.08, lo.y + s),
                                Vec2::new(hi.x - 0.08, lo.y + s),
                            )
                        };
                        if !r.at(p.to_array()).flat() || !r.at(q.to_array()).flat() {
                            continue;
                        }
                        let h = r.ceil[r.cell_of(p.to_array()).map_or(0, |(i, j)| j * r.nx + i)];
                        for e in [p, q] {
                            b.piece(
                                Shape::Box(Vec3::new(0.16, h, 0.16)),
                                Transform::from_xyz(e.x, r.floor + h * 0.5, e.y),
                                Mat::Timber,
                                false,
                            );
                        }
                        let mid = (p + q) * 0.5;
                        let span = (q - p).length() + 0.16;
                        let size = if along_x {
                            Vec3::new(0.18, 0.18, span)
                        } else {
                            Vec3::new(span, 0.18, 0.18)
                        };
                        b.piece(
                            Shape::Box(size),
                            Transform::from_xyz(mid.x, r.floor + h - 0.09, mid.y),
                            Mat::Timber,
                            false,
                        );
                    }
                }
            }
        }
        Style::Service | Style::Bunker => {
            // Along every corridor: two pipes (service) or a cable tray
            // (shelter) high on one wall.
            for legs in &r.legs {
                for (lo, hi) in legs {
                    let (lo, hi) = (Vec2::from(*lo), Vec2::from(*hi));
                    let size = hi - lo;
                    let along_x = size.x > size.y;
                    let len = size.x.max(size.y);
                    let h = 2.0;
                    let mid = (lo + hi) * 0.5;
                    if l.style == Style::Service {
                        for (k, rad) in [(0.0f32, 0.09f32), (1.0, 0.06)] {
                            let off = 0.18 + k * 0.22;
                            let (x, z) = if along_x {
                                (mid.x, lo.y + off)
                            } else {
                                (lo.x + off, mid.y)
                            };
                            let rot = if along_x {
                                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)
                            } else {
                                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
                            };
                            b.piece(
                                Shape::Cylinder(rad, len),
                                Transform::from_xyz(x, r.floor + h + 0.05 - k * 0.15, z)
                                    .with_rotation(rot),
                                Mat::RustyMetal,
                                false,
                            );
                        }
                    } else {
                        let (size, x, z) = if along_x {
                            (Vec3::new(len, 0.05, 0.3), mid.x, lo.y + 0.17)
                        } else {
                            (Vec3::new(0.3, 0.05, len), lo.x + 0.17, mid.y)
                        };
                        b.piece(
                            Shape::Box(size),
                            Transform::from_xyz(x, r.floor + h, z),
                            Mat::DarkMetal,
                            false,
                        );
                    }
                }
            }
        }
    }
    // A lamp fitting on the ceiling of every room that has one.
    for room in &l.rooms {
        if room.lamp == ug::Lamp::None {
            continue;
        }
        let y = r.floor + room.height;
        let from = b.geo.pieces.len();
        b.piece(
            Shape::Box(Vec3::new(0.32, 0.12, 0.18)),
            Transform::from_xyz(room.at[0], y - 0.06, room.at[1]),
            Mat::DarkMetal,
            false,
        );
        b.piece(
            Shape::Box(Vec3::new(0.24, 0.05, 0.12)),
            Transform::from_xyz(room.at[0], y - 0.145, room.at[1]),
            Mat::Glass,
            false,
        );
        b.dress(
            from,
            vec![(
                "bulkhead_lamp",
                Transform::from_xyz(room.at[0], y, room.at[1]),
            )],
        );
    }
}

/// One of the layout's props at `at` (on the floor), turned `yaw`.
fn prop(b: &mut Builder, kind: PropKind, at: Vec3, yaw: f32, loot: Option<&'static str>) {
    let rot = Quat::from_rotation_y(yaw);
    match kind {
        PropKind::Shelf => {
            b.shelf(at, yaw, loot);
        }
        PropKind::Locker => {
            b.locker(at, yaw);
        }
        PropKind::Crate => {
            b.crate_box(at, 0.9, loot);
        }
        PropKind::SmallCrate => {
            b.crate_box(at, 0.6, loot);
        }
        PropKind::Table => {
            b.table(at, yaw, loot);
        }
        PropKind::Workbench => {
            b.workbench(at, yaw, loot);
        }
        PropKind::Bunk => {
            b.bed(at, yaw);
            // The upper berth on four posts.
            for (x, z) in [(-0.42, -0.95), (0.42, -0.95), (-0.42, 0.95), (0.42, 0.95)] {
                b.piece(
                    Shape::Box(Vec3::new(0.06, 1.6, 0.06)),
                    Transform::from_translation(at + rot * Vec3::new(x, 0.8, z)),
                    Mat::DarkMetal,
                    false,
                );
            }
            b.piece(
                Shape::Box(Vec3::new(0.9, 0.06, 1.95)),
                Transform::from_translation(at + Vec3::Y * 1.3).with_rotation(rot),
                Mat::DarkMetal,
                true,
            );
            b.piece(
                Shape::Box(Vec3::new(0.85, 0.1, 1.85)),
                Transform::from_translation(at + Vec3::Y * 1.38).with_rotation(rot),
                Mat::Canvas,
                false,
            );
            if let Some(t) = loot {
                b.loot(at + Vec3::Y * 0.6, t);
            }
        }
        PropKind::Barrel => {
            b.barrel(at);
        }
        PropKind::Pallet => {
            b.pallet(at, yaw);
        }
        PropKind::Counter => {
            b.counter(at, yaw, 1.8, loot);
        }
        PropKind::Chair => {
            b.chair(at, yaw);
        }
        PropKind::Rubble => {
            // Slabs and lumps fallen from the ceiling: one low collider,
            // the pieces drawn tumbled over it.
            b.piece(
                Shape::Box(Vec3::new(1.2, 0.45, 1.0)),
                Transform::from_translation(at + Vec3::Y * 0.225).with_rotation(rot),
                Mat::Concrete,
                true,
            );
            for k in 0..5 {
                let a = k as f32 * 1.7 + yaw;
                let r = 0.25 + (k % 3) as f32 * 0.15;
                let size = Vec3::new(0.5 + (k % 2) as f32 * 0.25, 0.22, 0.4);
                b.piece(
                    Shape::Box(size),
                    Transform::from_translation(
                        at + Vec3::new(a.cos() * r, 0.38 + (k % 2) as f32 * 0.12, a.sin() * r),
                    )
                    .with_rotation(Quat::from_euler(
                        EulerRot::YXZ,
                        a,
                        0.25 * a.sin(),
                        0.3,
                    )),
                    Mat::Concrete,
                    false,
                );
            }
        }
        PropKind::Cart => {
            let from = b.geo.pieces.len();
            b.piece(
                Shape::Box(Vec3::new(0.9, 0.7, 1.4)),
                Transform::from_translation(at + Vec3::Y * 0.6).with_rotation(rot),
                Mat::RustyMetal,
                true,
            );
            for (x, z) in [(-0.45, -0.45), (0.45, -0.45), (-0.45, 0.45), (0.45, 0.45)] {
                b.piece(
                    Shape::Cylinder(0.16, 0.08),
                    Transform::from_translation(at + rot * Vec3::new(x, 0.16, z))
                        .with_rotation(rot * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    Mat::DarkMetal,
                    false,
                );
            }
            b.dress(
                from,
                vec![(
                    "mine_tub",
                    Transform::from_translation(at).with_rotation(rot),
                )],
            );
            if let Some(t) = loot {
                b.loot(at + Vec3::Y * 1.0, t);
            }
        }
        PropKind::Machine => {
            let from = b.geo.pieces.len();
            b.piece(
                Shape::Box(Vec3::new(1.4, 1.6, 1.0)),
                Transform::from_translation(at + Vec3::Y * 0.8).with_rotation(rot),
                Mat::PaintedMetal,
                true,
            );
            b.piece(
                Shape::Cylinder(0.25, 0.9),
                Transform::from_translation(at + rot * Vec3::new(0.0, 2.05, 0.0)),
                Mat::RustyMetal,
                false,
            );
            b.piece(
                Shape::Box(Vec3::new(0.4, 0.3, 0.06)),
                Transform::from_translation(at + rot * Vec3::new(0.3, 1.1, -0.53))
                    .with_rotation(rot),
                Mat::DarkMetal,
                false,
            );
            b.dress(
                from,
                vec![(
                    "filter_unit",
                    Transform::from_translation(at).with_rotation(rot),
                )],
            );
            if let Some(t) = loot {
                b.loot(at + Vec3::Y * 1.65, t);
            }
        }
        PropKind::Cabinet => {
            let from = b.geo.pieces.len();
            b.piece(
                Shape::Box(Vec3::new(0.8, 1.4, 0.4)),
                Transform::from_translation(at + Vec3::Y * 0.7).with_rotation(rot),
                Mat::PaintedMetal,
                true,
            );
            b.dress(
                from,
                vec![(
                    "switch_cabinet",
                    Transform::from_translation(at).with_rotation(rot),
                )],
            );
            if let Some(t) = loot {
                b.loot(at + Vec3::Y * 1.45, t);
            }
        }
    }
}

/// Logic: the anomalies that live in the sites (their layouts'
/// `anomalies`), laid out again with every cycle's.
pub fn spawn_anomalies(world: &mut World) {
    let zone = world.resource::<Zone>().clone();
    for p in zone
        .map
        .placements
        .iter()
        .filter(|p| ug::is_site(&p.prefab))
    {
        let Some(l) = ug::layout(&p.prefab) else {
            continue;
        };
        let xf = super::buildings::placement_transform(&zone, p);
        for h in &l.anomalies {
            let (Some(kind), Some(room)) = (h.kind(), l.rooms.iter().find(|r| r.id == h.room))
            else {
                continue;
            };
            let at = xf.transform_point(Vec3::new(
                room.at[0] + h.at[0],
                l.floor,
                room.at[1] + h.at[1],
            ));
            crate::anomaly::spawn_anomaly(world, kind, at, h.radius, None);
        }
    }
}

/// Client: the sites' emergency lamps, and the daylight taken away below
/// ground.
pub struct UndergroundVisualsPlugin;

impl Plugin for UndergroundVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (light_lamps, flicker, torch))
            .add_systems(PostUpdate, (go_dark, no_rain_below));
    }
}

/// Lamps lit at once in any site: unshadowed point lights, a handful.
pub const LAMP_BUDGET: usize = 6;
/// Lamps further than this from the eye are put out (nobody sees them
/// through the rock).
const LAMP_RANGE: f32 = 45.0;

/// An emergency lamp; `stutter` if it's failing. Its lens glows with it.
#[derive(Component)]
pub struct EmergencyLamp {
    pub stutter: bool,
    seed: f32,
    base: f32,
    lens: Handle<StandardMaterial>,
}

/// The lens's glow at full brightness.
const GLOW: LinearRgba = LinearRgba::rgb(9.0, 5.5, 2.6);

fn light_lamps(
    mut commands: Commands,
    buildings: Query<(Entity, &super::buildings::Building), Added<super::buildings::Building>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, b) in &buildings {
        let (Some(l), Some(r)) = (ug::layout(&b.prefab), ug::raster(&b.prefab)) else {
            continue;
        };
        let lit = l
            .rooms
            .iter()
            .filter(|room| matches!(room.lamp, ug::Lamp::Steady | ug::Lamp::Flicker))
            .take(LAMP_BUDGET);
        for (k, room) in lit.enumerate() {
            let at = Vec3::new(room.at[0], r.floor + room.height - 0.2, room.at[1]);
            let lens = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.85, 0.6),
                emissive: GLOW,
                ..default()
            });
            let size = room.size[0].max(room.size[1]);
            let base = 14_000.0 + 1_500.0 * size;
            commands
                .spawn((
                    Name::new("Emergency lamp"),
                    EmergencyLamp {
                        stutter: room.lamp == ug::Lamp::Flicker,
                        seed: k as f32 * 3.7 + room.at[0] * 0.13,
                        base,
                        lens: lens.clone(),
                    },
                    PointLight {
                        color: Color::srgb(1.0, 0.86, 0.68),
                        intensity: base,
                        range: size + 4.0,
                        radius: 0.05,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(at),
                    ChildOf(e),
                ))
                .with_child((
                    Mesh3d(meshes.add(Cuboid::new(0.22, 0.03, 0.1))),
                    MeshMaterial3d(lens),
                    bevy::light::NotShadowCaster,
                    Transform::from_xyz(0.0, 0.03, 0.0),
                ));
        }
    }
}

/// Failing lamps stutter (their lenses with them); every lamp goes out
/// past [`LAMP_RANGE`] from the eye.
fn flicker(
    time: Res<Time>,
    eyes: Query<(&GlobalTransform, &Camera), With<Camera3d>>,
    mut lamps: Query<(
        &EmergencyLamp,
        &GlobalTransform,
        &mut PointLight,
        &mut Visibility,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let eye = eyes
        .iter()
        .filter(|(_, c)| c.is_active)
        .map(|(g, _)| g.translation())
        .next();
    for (lamp, at, mut light, mut vis) in &mut lamps {
        let near = eye.is_none_or(|e| e.distance(at.translation()) < LAMP_RANGE);
        let want = if near {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if !lamp.stutter || !near {
            continue;
        }
        let k = stutter(t, lamp.seed);
        light.intensity = lamp.base * k;
        if let Some(mut m) = materials.get_mut(&lamp.lens) {
            m.emissive = GLOW * k;
        }
    }
}

/// Screenshots of the dark (`EXTRACTION_TORCH=1`): the free camera
/// carries a torch like the flashlight gadget's beam.
fn torch(mut commands: Commands, cams: Query<Entity, Added<crate::render::FreeCamera>>) {
    if std::env::var_os("EXTRACTION_TORCH").is_none() {
        return;
    }
    for e in &cams {
        commands.entity(e).with_child((
            SpotLight {
                color: Color::srgb(1.0, 0.95, 0.85),
                intensity: 250_000.0,
                range: 45.0,
                radius: 0.01,
                inner_angle: 0.12,
                outer_angle: 0.38,
                shadow_maps_enabled: true,
                ..default()
            },
            // Held low and to the right, as a hand would.
            Transform::from_xyz(0.25, -0.35, -0.2),
        ));
    }
}

/// A failing lamp's brightness at time `t`: mostly on, with a burst of
/// dropouts every several seconds.
pub fn stutter(t: f32, seed: f32) -> f32 {
    let cycle = 7.0 + 3.0 * (seed * 1.3).sin().abs();
    let phase = (t + seed * 11.0) % cycle;
    if phase < cycle - 1.2 {
        // A slight hum.
        return 0.93 + 0.07 * (t * 37.0 + seed).sin();
    }
    let s = ((phase * 23.0 + seed).sin() * 43_758.547).fract().abs();
    if s > 0.55 { 0.9 } else { 0.05 + 0.2 * s }
}

/// The cameras' exposure in daylight (`render::spawn_camera`'s), and how
/// many stops the eyes open up underground.
const SURFACE_EV: f32 = 10.6;
const ADAPT_STOPS: f32 = 2.5;

/// The surface's light as the sky set it, before going dark.
#[derive(Default)]
struct Daylight {
    written: Option<(f32, f32)>,
    base: (f32, f32),
    fog: Option<Color>,
}

/// How far below ground the eye is, 0 (on the surface) to 1 (a few
/// metres down a stair or further): ambient light, the sun and the fog's
/// colour go with it, so the rooms are as dark as their lamps leave them.
fn go_dark(
    zone: Res<Zone>,
    eyes: Query<(&GlobalTransform, &Camera), With<Camera3d>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut suns: Query<&mut DirectionalLight, With<super::sky::Sun>>,
    mut fogs: Query<&mut DistanceFog>,
    mut exposures: Query<&mut bevy::camera::Exposure>,
    mut day: Local<Daylight>,
    mut depth: Local<f32>,
) {
    let Some(eye) = eyes
        .iter()
        .filter(|(_, c)| c.is_active)
        .map(|(g, _)| g.translation())
        .next()
    else {
        return;
    };
    let want = match floor_under(&zone, eye) {
        Some(_) => {
            let below = zone.ground(eye.x, eye.z) - eye.y;
            extraction_core::geom::smoothstep(0.3, 2.5, below)
        }
        None => 0.0,
    };
    *depth += (want - *depth) * 0.15;
    let k = *depth;
    let sun = suns.iter().next().map_or(0.0, |s| s.illuminance);
    // What the sky systems last set (anything other than what this wrote).
    if day.written != Some((ambient.brightness, sun)) {
        day.base = (ambient.brightness, sun);
    }
    if k < 1e-3 && day.written.is_none() {
        return;
    }
    let dark = 1.0 - k * 0.985;
    ambient.brightness = day.base.0 * dark;
    for mut s in &mut suns {
        s.illuminance = day.base.1 * (1.0 - k);
    }
    day.written = Some((ambient.brightness, day.base.1 * (1.0 - k)));
    // The eyes adapt: a few stops brighter, all the way down.
    for mut e in &mut exposures {
        let want = SURFACE_EV - ADAPT_STOPS * k;
        if (e.ev100 - want).abs() > 1e-3 {
            e.ev100 = want;
        }
    }
    for mut fog in &mut fogs {
        let surface = *day.fog.get_or_insert(fog.color);
        if k < 1e-3 {
            fog.color = surface;
            day.fog = None;
        } else {
            let a = surface.to_linear().to_vec3();
            let c = a.lerp(Vec3::new(0.004, 0.004, 0.005), k);
            fog.color = Color::linear_rgb(c.x, c.y, c.z);
        }
    }
    if k < 1e-3 {
        day.written = None;
    }
}

/// No rain or ash falls past your eyes underground (both are drawn round
/// the camera): hidden below, back as they were on the surface.
fn no_rain_below(
    zone: Res<Zone>,
    eyes: Query<(&GlobalTransform, &Camera), With<Camera3d>>,
    mut fall: Query<(Entity, &mut Visibility), With<MeshMaterial3d<super::rain::RainMaterial>>>,
    mut hidden: Local<Vec<Entity>>,
) {
    let Some(eye) = eyes
        .iter()
        .filter(|(_, c)| c.is_active)
        .map(|(g, _)| g.translation())
        .next()
    else {
        return;
    };
    let below = floor_under(&zone, eye).is_some() && eye.y < zone.ground(eye.x, eye.z) - 0.5;
    if below {
        for (e, mut vis) in &mut fall {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
                hidden.push(e);
            }
        }
    } else {
        for e in hidden.drain(..) {
            if let Ok((_, mut vis)) = fall.get_mut(e) {
                *vis = Visibility::Inherited;
            }
        }
    }
}

/// Where the floor is under a world point, if it's inside an underground
/// site (on a stair or in a room, below the ground): the world y.
pub fn floor_under(zone: &Zone, p: Vec3) -> Option<f32> {
    for site in ug::placed(&zone.map) {
        let r = ug::raster(&site.id)?;
        let footing = zone.hm.footing(&site.id, site.at, site.rot);
        let local = site.to_site([p.x, p.z]);
        let Some(f) = r.floor_at(local) else { continue };
        let floor = footing + f;
        if p.y > floor - 0.5 && p.y < floor + 4.5 && floor < zone.ground(p.x, p.z) - 0.5 {
            return Some(floor);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::LaunchOptions;
    use avian3d::prelude::*;
    use bevy::ecs::system::RunSystemOnce;

    /// Every site's pieces are sound (positive sizes, finite poses) and it
    /// has doors and loot.
    #[test]
    fn every_site_builds() {
        for id in ids() {
            let g = geo(id).unwrap();
            assert!(g.pieces.len() > 100, "{id}: {} pieces", g.pieces.len());
            assert!(g.doors.len() >= 6, "{id}: {} doors", g.doors.len());
            assert!(g.loot.len() >= 8, "{id}: {} loot spots", g.loot.len());
            for (i, p) in g.pieces.iter().enumerate() {
                let ok = match p.shape {
                    Shape::Box(s) | Shape::Prism(s) => s.is_finite() && s.min_element() > 0.0,
                    Shape::Cylinder(r, h) => r > 0.0 && h > 0.0,
                } && p.at.translation.is_finite()
                    && p.at.rotation.is_finite();
                assert!(ok, "{id} piece {i}: {:?} at {:?}", p.shape, p.at);
            }
        }
    }

    fn app(map: &str) -> App {
        let opts = LaunchOptions {
            robots: false,
            map: map.to_string(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        app
    }

    /// The sites each valley places (one each).
    fn sites_on(app: &App) -> Vec<(ug::Placed, Transform)> {
        let zone = app.world().resource::<Zone>().clone();
        zone.map
            .placements
            .iter()
            .filter(|p| ug::is_site(&p.prefab))
            .map(|p| {
                (
                    ug::placed(&zone.map)
                        .into_iter()
                        .find(|s| s.at == p.at)
                        .unwrap(),
                    super::super::buildings::placement_transform(&zone, p),
                )
            })
            .collect()
    }

    /// No daylight gets into the rooms: from every room and corridor, rays
    /// every way (doors shut) meet the site's own walls, ceilings and
    /// doors before they could reach the sky; and the floor is under the
    /// ground everywhere past the kiosks.
    #[test]
    fn no_daylight_in_the_rooms() {
        for map in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let mut app = app(map);
            let sites = sites_on(&app);
            assert_eq!(sites.len(), 1, "{map}: one site");
            let mut dirs = Vec::new();
            for a in 0..12 {
                for e in [-0.3f32, 0.0, 0.4, 0.8, 1.2] {
                    let a = a as f32 * std::f32::consts::TAU / 12.0;
                    dirs.push(Vec3::new(a.cos() * e.cos(), e.sin(), a.sin() * e.cos()));
                }
            }
            dirs.push(Vec3::Y);
            for (site, xf) in sites {
                let r = ug::raster(&site.id).unwrap();
                let mut from = Vec::new();
                for j in (0..r.nz).step_by(4) {
                    for i in (0..r.nx).step_by(4) {
                        if r.get(i, j).flat() {
                            let c = r.centre(i, j);
                            from.push(xf.transform_point(Vec3::new(c[0], r.floor + 1.5, c[1])));
                        }
                    }
                }
                assert!(from.len() > 50);
                let dirs = dirs.clone();
                let leaks: Vec<(Vec3, Vec3)> = app
                    .world_mut()
                    .run_system_once(move |spatial: SpatialQuery| {
                        let mut out = Vec::new();
                        for p in &from {
                            for d in &dirs {
                                let hit = spatial.cast_ray(
                                    *p,
                                    Dir3::new(*d).unwrap(),
                                    300.0,
                                    true,
                                    &SpatialQueryFilter::default(),
                                );
                                // Within the site's own bounds: its walls.
                                if hit.is_none_or(|h| h.distance > 90.0) {
                                    out.push((*p, *d));
                                }
                            }
                        }
                        out
                    })
                    .unwrap();
                assert!(
                    leaks.is_empty(),
                    "{map} {}: {} rays out to the sky, e.g. {:?}",
                    site.id,
                    leaks.len(),
                    &leaks[..leaks.len().min(5)]
                );
            }
        }
    }

    /// Down every stairwell, inside its kiosk, a ray from above meets the
    /// steps, not the ground (the heightfield's cells are open there).
    #[test]
    fn the_ground_is_open_over_the_stairs() {
        for map in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let mut app = app(map);
            for (site, xf) in sites_on(&app) {
                let r = ug::raster(&site.id).unwrap();
                let probes: Vec<(Vec3, f32)> = r
                    .portals
                    .iter()
                    .zip(&r.stairs)
                    .flat_map(|(p, s)| {
                        [0.5f32, 1.5, 2.5].map(|z| {
                            let q = p.to_site([0.3, z]);
                            (
                                xf.transform_point(Vec3::new(q[0], p.top() + 1.5, q[1])),
                                xf.transform_point(Vec3::new(0.0, s.floor_at(z), 0.0)).y,
                            )
                        })
                    })
                    .collect();
                let hits: Vec<(Vec3, f32, Option<f32>)> = app
                    .world_mut()
                    .run_system_once(move |spatial: SpatialQuery| {
                        probes
                            .iter()
                            .map(|(p, want)| {
                                let h = spatial.cast_ray(
                                    *p,
                                    Dir3::NEG_Y,
                                    20.0,
                                    true,
                                    &SpatialQueryFilter::default(),
                                );
                                (*p, *want, h.map(|h| p.y - h.distance))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap();
                for (p, want, got) in hits {
                    let got = got.expect("something under the stair");
                    assert!(
                        (got - want).abs() < 0.3,
                        "{map}: over the stair at {p:?} the ray stops at {got:.2}, the tread is at {want:.2}"
                    );
                }
            }
        }
    }

    /// A bot goes in at the entrance, down the stairs, to the deepest loot
    /// in the site, and out by the other way (the exit kiosk).
    #[test]
    fn a_bot_goes_down_and_out_the_other_way() {
        use crate::bot::Bot;
        use crate::rig::LocalPlayer;
        use crate::sim::indoor::{BuildingPlan, plan_buildings};
        for map in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let mut app = app(map);
            let plans: Vec<BuildingPlan> = app
                .world_mut()
                .run_system_once(
                    |b: Query<(Entity, &super::super::buildings::Building, &Transform)>,
                     d: Query<(Entity, &super::super::buildings::DoorLeaf, &ChildOf)>| {
                        plan_buildings(&b, &d)
                    },
                )
                .unwrap();
            let plan = plans
                .iter()
                .find(|p| ug::is_site(&p.name))
                .expect("a site plan")
                .clone();
            assert!(
                plan.entries.len() >= 2,
                "{map}: {} ways in",
                plan.entries.len()
            );
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            // Stand outside the entrance (the nearest stand to the origin).
            let entry = *plan
                .entries
                .iter()
                .min_by(|a, b| a.length().total_cmp(&b.length()))
                .unwrap();
            let exit = *plan
                .entries
                .iter()
                .max_by(|a, b| a.length().total_cmp(&b.length()))
                .unwrap();
            let start = plan.to_world(entry);
            let ground = app.world().resource::<Zone>().ground(start.x, start.z);
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = Vec3::new(start.x, ground + 0.1, start.z);
            app.update();
            // The loot furthest from the entrance.
            let spots: Vec<Vec3> = app
                .world()
                .resource::<super::super::buildings::BuildingLoot>()
                .0
                .iter()
                .map(|s| s.at)
                .filter(|s| plan.contains(*s, 0.0))
                .collect();
            assert!(spots.len() >= 8, "{map}: {} spots in the site", spots.len());
            let goal = spots
                .iter()
                .map(|s| plan.to_local(*s).xz())
                .max_by(|a, b| a.distance(entry).total_cmp(&b.distance(entry)))
                .unwrap();
            if std::env::var_os("SITE_DUMP").is_some() {
                crate::sim::indoor::dump_plans(
                    std::slice::from_ref(&plan),
                    &spots,
                    &crate::asset_root_path()
                        .join("../target/site_plans")
                        .join(map),
                );
            }
            let walk = |app: &mut App, to: Vec2, within: f32, what: &str| {
                let pos = app.world().get::<Transform>(player).unwrap().translation;
                let tasks = crate::sim::raider::indoor_route(&plan, pos, to, within)
                    .unwrap_or_else(|| {
                        let a = plan.to_local(pos).xz();
                        panic!(
                            "{map}: no route {what} from {a:?} (walkable {}) to {to:?}: reach {:?}",
                            plan.grid.walkable(a.to_array()),
                            crate::sim::raider::reach_cell(&plan, a, to)
                        )
                    });
                app.world_mut().entity_mut(player).insert(Bot::new(tasks));
                for _ in 0..90 * 120 {
                    app.update();
                    if app.world().get::<Bot>(player).unwrap().done() {
                        break;
                    }
                }
                app.world().get::<Transform>(player).unwrap().translation
            };
            let at = walk(&mut app, goal, 0.3, "to the loot");
            let local = plan.to_local(at);
            assert!(
                local.xz().distance(goal) < crate::bot::REACH + 0.3,
                "{map}: stopped at {local:?} short of the loot at {goal:?}"
            );
            assert!(
                local.y < plan.floor + 1.0,
                "{map}: not down at the floor: {local:?}"
            );
            let at = walk(&mut app, exit, 0.6, "out");
            let local = plan.to_local(at);
            assert!(
                local.xz().distance(exit) < 1.2,
                "{map}: stopped at {local:?}, not out at {exit:?}"
            );
            let ground = app.world().resource::<Zone>().ground(at.x, at.z);
            assert!(at.y > ground - 0.3, "{map}: out but under the ground");
            // Out on the ground, it's out: a raider there doesn't try to
            // leave the site again.
            assert!(
                !plan.contains(at, -0.1),
                "{map}: out, but still in the site"
            );
            assert!(
                plan.in_grid(at),
                "{map}: the ground by the kiosk is on the plan"
            );
        }
    }
}
