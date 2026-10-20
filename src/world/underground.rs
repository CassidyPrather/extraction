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

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::underground::{
    self as ug, APRON, CEILING_RUN, Cell, KIOSK, Layout, PropKind, Raster, SLAB, STAIR_HALF,
    STAIR_TOP, Stair, Style, TREAD, WALL,
};
use std::collections::{HashMap, HashSet};

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
            let c = j * r.nx + i;
            Some((r.floors[c] - SLAB, r.floors[c] + r.ceil[c] + SLAB))
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
        Cell::Room(_) | Cell::Hall(_) => {
            let c = j * r.nx + i;
            Some(r.floors[c] + r.ceil[c])
        }
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
    dressing(&mut b, l, r);
    for room in &l.rooms {
        for p in &room.props {
            let at = Vec3::new(room.at[0] + p.at[0], l.floor_of(room), room.at[1] + p.at[1]);
            prop(&mut b, p.kind, at, -p.rot.to_radians(), p.loot.as_deref());
        }
    }
    // Breaker boards on their walls (a generator is the room's machine).
    for p in l.power.iter().filter(|p| p.kind == ug::PowerKind::Breaker) {
        if let Some(room) = l.rooms.iter().find(|room| room.id == p.room) {
            let at = Vec3::new(room.at[0] + p.at[0], l.floor_of(room), room.at[1] + p.at[1]);
            prop(&mut b, PropKind::Cabinet, at, -p.rot.to_radians(), None);
        }
    }
    // Last, so each door can swing clear of what's there.
    doors(&mut b, l, r, &m);
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
                // Where it stopped short at a mouth, the wall across the
                // mouth fills the corner, but only as high as its own
                // band: fill on up to this wall's top (a lower corridor
                // into a tall room left a slit of sky there).
                for (beyond, from) in [
                    (s0.checked_sub(1), s0 as f32 * cell),
                    ((s < len).then_some(s), s as f32 * cell - WALL),
                ] {
                    let Some(q) = beyond else { continue };
                    if !(vertical && edge(t, q) == Edge::Open) {
                        continue;
                    }
                    let (p, o) = sides(t, q);
                    let Some((pi, pj)) = (if side < 0 { o } else { p }) else {
                        continue;
                    };
                    let Some((_, top)) = extent(r, pi, pj) else {
                        continue;
                    };
                    if hi <= top + 0.01 {
                        continue;
                    }
                    b.bx(
                        Vec3::new(WALL, hi - top, WALL),
                        Vec3::new(
                            r.origin[0] + off,
                            (top + hi) * 0.5,
                            r.origin[1] + from + WALL * 0.5,
                        ),
                        mat,
                    );
                }
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
    // By the floor's height (cm): each level, each step of a stair
    // corridor, each stair's landing.
    let level = |i: usize, j: usize| match r.get(i, j) {
        Cell::Room(_) | Cell::Hall(_) => Some((r.floors[j * r.nx + i] * 100.0).round() as i32),
        Cell::Stair(k) => {
            let k = k as usize;
            let s = &r.stairs[k];
            let z = r.portals[k].to_portal(r.centre(i, j))[1];
            (z >= s.bottom).then(|| (s.floor_at(s.bottom) * 100.0).round() as i32)
        }
        Cell::Rock => None,
    };
    for (lo, hi, f) in rects(r, level) {
        let y = f as f32 / 100.0;
        let (size, centre) = rect_box(r, lo, hi, y - SLAB, y);
        b.bx(size, centre, m.floor);
    }
    // By the ceiling's height over each cell.
    let roof = |i: usize, j: usize| {
        let c = j * r.nx + i;
        r.get(i, j)
            .flat()
            .then(|| ((r.floors[c] + r.ceil[c]) * 100.0).round() as i32)
    };
    for (lo, hi, h) in rects(r, roof) {
        let y = h as f32 / 100.0;
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
fn doors(b: &mut Builder, l: &Layout, r: &Raster, m: &Mats) {
    for d in &r.doors {
        let at = Vec2::from(d.at);
        // A leaf hinges at the wall's start and opens toward its right,
        // (along.z, -along.x): the wall one way or the other. Its whole
        // swing clear of the furniture and walls, and into its room
        // rather than back into the corridor it closes, if it can.
        let into = d.room.map(|k| Vec2::from(l.rooms[k as usize].at) - at);
        let along = [Vec2::from(d.along), -Vec2::from(d.along)]
            .into_iter()
            .max_by_key(|dir| {
                let clear = swing_clear(b, r, at, *dir, d.width, d.y);
                let inward = into.is_some_and(|v| Vec2::new(dir.y, -dir.x).dot(v) > 0.0);
                (clear, inward)
            })
            .unwrap();
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

/// Whether a door in a wall from `at - dir * width / 2` along `dir` (as
/// `Builder::wall` hangs it: hinged at the opening's start, opening to the
/// wall's right) swings clear of every solid piece built so far and of
/// the rock.
fn swing_clear(b: &Builder, r: &Raster, at: Vec2, dir: Vec2, width: f32, y: f32) -> bool {
    let (open, leaf) = (0.95, 0.91);
    let start = at - dir * width * 0.5 + dir * ((width - open) * 0.5 + 0.02);
    let hinge = Vec3::new(start.x, y + 1.0, start.y);
    let yaw = -dir.y.atan2(dir.x);
    [0.3f32, 0.6, 0.9, 1.2, 1.5, 1.75].iter().all(|a| {
        [0.35f32, 0.65, 0.95].iter().all(|f| {
            let p = hinge + Quat::from_rotation_y(yaw + a) * Vec3::X * leaf * f;
            r.at([p.x, p.z]).flat()
                && !b.geo.pieces.iter().any(|piece| {
                    let Shape::Box(size) = piece.shape else {
                        return false;
                    };
                    let l = piece.at.compute_affine().inverse().transform_point3(p);
                    piece.collide && l.abs().cmplt(size * 0.5 - 0.02).all()
                })
        })
    })
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
                        // In the corridor itself, not where it runs on
                        // through a room to the room's centre.
                        let hall = |p: Vec2| matches!(r.at(p.to_array()), Cell::Hall(_));
                        if !hall(p) || !hall(q) {
                            continue;
                        }
                        let c = r.cell_of(p.to_array()).map_or(0, |(i, j)| j * r.nx + i);
                        let (h, f) = (r.ceil[c], r.floors[c]);
                        for e in [p, q] {
                            b.piece(
                                Shape::Box(Vec3::new(0.16, h, 0.16)),
                                Transform::from_xyz(e.x, f + h * 0.5, e.y),
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
                            Transform::from_xyz(mid.x, f + h - 0.09, mid.y),
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
                    // Level legs only (a corridor between levels steps).
                    let floor_at = |p: Vec2| r.floor_at(p.to_array());
                    let ends = [lo + (hi - lo) * 0.05, mid, hi - (hi - lo) * 0.05];
                    let fs: Vec<f32> = ends.iter().filter_map(|p| floor_at(*p)).collect();
                    let Some(&f) = fs.first() else { continue };
                    if fs.iter().any(|x| (x - f).abs() > 0.01) {
                        continue;
                    }
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
                                Transform::from_xyz(x, f + h + 0.05 - k * 0.15, z)
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
                            Transform::from_xyz(x, f + h, z),
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
        let y = l.floor_of(room) + room.height;
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
            // the pieces drawn tumbled over it (the `rubble` model once in).
            let from = b.geo.pieces.len();
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
            b.dress(
                from,
                vec![("rubble", Transform::from_translation(at).with_rotation(rot))],
            );
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
        PropKind::Bin => {
            // A floor and four sides: what's in it lies in it.
            let from = b.geo.pieces.len();
            let side = |b: &mut Builder, size: Vec3, off: Vec3| {
                b.piece(
                    Shape::Box(size),
                    Transform::from_translation(at + rot * off).with_rotation(rot),
                    Mat::DarkMetal,
                    true,
                );
            };
            side(b, Vec3::new(1.2, 0.06, 0.8), Vec3::new(0.0, 0.03, 0.0));
            side(b, Vec3::new(1.2, 0.9, 0.04), Vec3::new(0.0, 0.45, -0.38));
            side(b, Vec3::new(1.2, 0.9, 0.04), Vec3::new(0.0, 0.45, 0.38));
            side(b, Vec3::new(0.04, 0.9, 0.8), Vec3::new(-0.58, 0.45, 0.0));
            side(b, Vec3::new(0.04, 0.9, 0.8), Vec3::new(0.58, 0.45, 0.0));
            b.dress(
                from,
                vec![(
                    "depot_bin",
                    Transform::from_translation(at).with_rotation(rot),
                )],
            );
            if let Some(t) = loot {
                b.loot(at + Vec3::Y * 0.3, t);
            }
        }
        // A kept crate is its own entity (`KeptCrate`: it opens); here only
        // its collider, for the walkers and the bots' plans.
        PropKind::KeptCrate | PropKind::KeptCrateSmall => {
            let size = kept_size(kind == PropKind::KeptCrate);
            let from = b.geo.pieces.len();
            b.piece(
                Shape::Box(size),
                Transform::from_translation(at + Vec3::Y * size.y * 0.5).with_rotation(rot),
                Mat::PaintedMetal,
                true,
            );
            b.dress(from, vec![]);
        }
        PropKind::Conveyor | PropKind::Tagger | PropKind::Cradle => {
            let (model, size) = match kind {
                PropKind::Conveyor => ("depot_conveyor", Vec3::new(6.0, 1.0, 0.9)),
                PropKind::Tagger => ("depot_tagger", Vec3::new(0.7, 1.3, 0.6)),
                _ => ("depot_cradle", Vec3::new(2.2, 0.3, 2.0)),
            };
            let from = b.geo.pieces.len();
            b.piece(
                Shape::Box(size),
                Transform::from_translation(at + Vec3::Y * size.y * 0.5).with_rotation(rot),
                Mat::PaintedMetal,
                true,
            );
            // The cradle's uprights, at the wall.
            if kind == PropKind::Cradle {
                for x in [-1.0, 1.0] {
                    b.piece(
                        Shape::Box(Vec3::new(0.2, 2.6, 0.3)),
                        Transform::from_translation(at + rot * Vec3::new(x, 1.3, 0.8))
                            .with_rotation(rot),
                        Mat::PaintedMetal,
                        true,
                    );
                }
            }
            b.dress(
                from,
                vec![(model, Transform::from_translation(at).with_rotation(rot))],
            );
            if let Some(t) = loot {
                b.loot(at + Vec3::Y * (size.y + 0.15), t);
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
                l.floor_of(room),
                room.at[1] + h.at[1],
            ));
            crate::anomaly::spawn_anomaly(world, kind, at, h.radius, None);
        }
    }
}

/// Logic: power, residents and the robots' map of the dark.
pub struct UndergroundPlugin;

impl Plugin for UndergroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                register_residents,
                wire_power,
                throw_switches.after(wire_power),
                generator_hum,
                splashing,
                depot_visits,
                fill_lost_bins,
                spawn_kept_crates,
            ),
        )
        .add_message::<DepotEntered>()
        .add_message::<CarriedOutOfDepot>();
    }
}

/// A site's power ([`ug::PowerDef`]), stage by stage: each off until its
/// switch is thrown, and a switch does nothing until the stage before is
/// on.
#[derive(Component, Debug)]
pub struct SitePower {
    pub stages: Vec<Stage>,
}

#[derive(Debug)]
pub struct Stage {
    pub on: bool,
    pub kind: ug::PowerKind,
    pub switch: Entity,
}

impl SitePower {
    pub fn on(&self, stage: usize) -> bool {
        self.stages.get(stage).is_some_and(|s| s.on)
    }

    /// The stage its next switch turns on, if any are still off.
    pub fn next(&self) -> Option<usize> {
        self.stages.iter().position(|s| !s.on)
    }
}

/// The lever (a breaker's) or pull-start (a generator's) that turns a
/// stage of a site's power on.
#[derive(Component, Debug)]
pub struct PowerSwitch {
    pub site: Entity,
    pub stage: usize,
}

/// A door held shut until its stage of its site's power is on.
#[derive(Component, Debug)]
pub struct PowerLocked {
    pub site: Entity,
    pub stage: usize,
}

/// Where a stage's power switch is, building space, and which way it
/// faces.
pub fn switch_pose(l: &Layout, stage: usize) -> Option<Transform> {
    let p = l.power.get(stage)?;
    let room = l.rooms.iter().find(|room| room.id == p.room)?;
    let rot = Quat::from_rotation_y(-p.rot.to_radians());
    let at = Vec3::new(room.at[0] + p.at[0], l.floor_of(room), room.at[1] + p.at[1]);
    // A breaker's lever on its board's face; a generator's pull-start low
    // on its end.
    let (up, out) = match p.kind {
        ug::PowerKind::Breaker => (1.2, 0.26),
        ug::PowerKind::Generator => (0.9, 0.05),
    };
    Some(
        Transform::from_translation(at + Vec3::Y * up + rot * Vec3::new(0.0, 0.0, -out))
            .with_rotation(rot),
    )
}

/// The door leaves (building space hinges) a stage of a site's power
/// holds shut.
pub fn powered_hinges(id: &str, stage: usize) -> Vec<Vec3> {
    let (Some(l), Some(r), Some(g)) = (ug::layout(id), ug::raster(id), geo(id)) else {
        return Vec::new();
    };
    let Some(p) = l.power.get(stage) else {
        return Vec::new();
    };
    // The geometry's doors: each kiosk's, then the raster's in order.
    let first = r.portals.len();
    let kiosks = p.portals().filter_map(|k| g.doors.get(k).map(|d| d.hinge));
    r.doors
        .iter()
        .enumerate()
        .filter(|(_, d)| p.holds(l, d))
        .filter_map(|(k, _)| g.doors.get(first + k).map(|d| d.hinge))
        .chain(kiosks)
        .collect()
}

/// Logic, once: each powered site gets its switches, its state, and its
/// held doors.
fn wire_power(
    mut commands: Commands,
    buildings: Query<(Entity, &super::buildings::Building)>,
    doors: Query<(Entity, &super::buildings::DoorLeaf, &ChildOf)>,
    mut done: Local<bool>,
) {
    if *done || buildings.is_empty() {
        return;
    }
    *done = true;
    for (site, b) in &buildings {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        let mut stages = Vec::new();
        for (stage, p) in l.power.iter().enumerate() {
            let Some(pose) = switch_pose(l, stage) else {
                continue;
            };
            let switch = commands
                .spawn((
                    Name::new(format!("Power switch {}", stage + 1)),
                    PowerSwitch { site, stage },
                    crate::raid::truck::Pressable::BenchRun,
                    crate::interact::Grabbable {
                        points: vec![crate::interact::GrabPoint::free()],
                    },
                    Collider::cuboid(0.3, 0.3, 0.15),
                    CollisionLayers::new(super::Layer::Item, LayerMask::NONE),
                    pose,
                    Visibility::default(),
                    ChildOf(site),
                ))
                .id();
            stages.push(Stage {
                on: false,
                kind: p.kind,
                switch,
            });
            for hinge in powered_hinges(&b.prefab, stage) {
                for (d, leaf, parent) in &doors {
                    if parent.parent() == site && leaf.0.hinge.distance(hinge) < 1e-3 {
                        commands.entity(d).insert(PowerLocked { site, stage });
                    }
                }
            }
        }
        if !stages.is_empty() {
            commands.entity(site).insert(SitePower { stages });
        }
    }
}

/// Throwing a switch: its stage comes on (for good), the doors it held
/// are free, its lamps light (and the robots know it), and it clanks or
/// roars loud enough to bring the crawlers. Out of turn, it's dead.
#[allow(clippy::too_many_arguments)]
fn throw_switches(
    mut commands: Commands,
    mut events: MessageReader<crate::interact::InteractEvent>,
    switches: Query<(&PowerSwitch, &GlobalTransform)>,
    mut sites: Query<(&mut SitePower, &super::buildings::Building, &Transform)>,
    locked: Query<(Entity, &PowerLocked)>,
    lighting: Option<ResMut<crate::robots::senses::Lighting>>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    let mut lighting = lighting;
    for ev in events.read() {
        let crate::interact::InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok((sw, at)) = switches.get(*item) else {
            continue;
        };
        let Ok((mut power, b, xf)) = sites.get_mut(sw.site) else {
            continue;
        };
        if power.on(sw.stage) {
            continue;
        }
        if power.next() != Some(sw.stage) {
            raid.say(match power.stages[sw.stage].kind {
                ug::PowerKind::Generator => "The starter turns over on nothing. No feed.",
                ug::PowerKind::Breaker => "The breaker drops home, dead. Nothing upstream.",
            });
            continue;
        }
        power.stages[sw.stage].on = true;
        for (d, l) in &locked {
            if l.site == sw.site && l.stage == sw.stage {
                commands.entity(d).remove::<PowerLocked>();
            }
        }
        let (radius, line) = match power.stages[sw.stage].kind {
            ug::PowerKind::Generator => (45.0, "The generator coughs, catches and roars. Lights."),
            ug::PowerKind::Breaker => (30.0, "The breaker slams home. Somewhere, relays clack."),
        };
        noise.write(crate::robots::Noise {
            at: at.translation(),
            radius,
            source: None,
        });
        raid.say(line);
        if let (Some(lighting), Some(l)) = (lighting.as_mut(), ug::layout(&b.prefab)) {
            for room in l
                .rooms
                .iter()
                .filter(|room| room.lamp == ug::Lamp::Powered && room.stage == sw.stage)
            {
                let p = Vec3::new(room.at[0], l.floor_of(room) + room.height - 0.2, room.at[1]);
                lighting
                    .lamps
                    .push((xf.transform_point(p), room.size[0].max(room.size[1]) + 4.0));
            }
        }
    }
}

/// Wading underground is loud: the water slops off the walls. (Footsteps
/// are heard as ever; this is the splashing on top, every half second
/// you move in it.)
fn splashing(
    zone: Res<Zone>,
    bodies: Query<(Entity, &crate::player::Body, &GlobalTransform)>,
    time: Res<Time>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut wait: Local<f32>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = 0.5;
    for (e, body, gt) in &bodies {
        let moving = Vec2::new(body.velocity.x, body.velocity.z).length() > 0.4;
        if body.wading < 0.2 || !moving || floor_under(&zone, gt.translation()).is_none() {
            continue;
        }
        noise.write(crate::robots::Noise {
            at: gt.translation(),
            radius: if body.swimming { 20.0 } else { 16.0 },
            source: Some(e),
        });
    }
}

/// A running generator is heard all round it, every few seconds.
fn generator_hum(
    sites: Query<&SitePower>,
    switches: Query<&GlobalTransform, With<PowerSwitch>>,
    time: Res<Time>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut wait: Local<f32>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = 4.0;
    for p in sites
        .iter()
        .flat_map(|p| &p.stages)
        .filter(|p| p.on && p.kind == ug::PowerKind::Generator)
    {
        if let Ok(at) = switches.get(p.switch) {
            noise.write(crate::robots::Noise {
                at: at.translation(),
                radius: 22.0,
                source: None,
            });
        }
    }
}

/// Logic, once the sites are built: tells the robots where it's dark and
/// where the lamps are (`senses::Lighting`), and registers each site's
/// resident crawlers with their round (`robots::indoor::IndoorSites`).
#[allow(clippy::type_complexity)]
pub fn register_residents(
    buildings: Query<(Entity, &super::buildings::Building, &Transform)>,
    doors: Query<(Entity, &super::buildings::DoorLeaf, &ChildOf)>,
    lighting: Option<ResMut<crate::robots::senses::Lighting>>,
    residents: Option<ResMut<crate::robots::indoor::IndoorSites>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    *done = true;
    let (Some(mut lighting), Some(mut residents)) = (lighting, residents) else {
        return;
    };
    let plans = crate::sim::indoor::plan_buildings(&buildings, &doors);
    for plan in plans.into_iter().filter(|p| ug::is_site(&p.name)) {
        let (Some(l), Some(r)) = (ug::layout(&plan.name), ug::raster(&plan.name)) else {
            continue;
        };
        let xf = plan.xf;
        // Dark: the site's rooms and stairs, from under the floor to just
        // under the ground (the kiosks are lit like any building).
        let lo = Vec2::from(r.origin);
        let hi = lo + Vec2::new(r.nx as f32, r.nz as f32) * ug::CELL;
        let deepest = l
            .rooms
            .iter()
            .map(|room| l.floor_of(room))
            .fold(r.floor, f32::min);
        let (y0, y1) = (deepest - 0.5, -0.3);
        let centre = Vec3::new((lo.x + hi.x) * 0.5, (y0 + y1) * 0.5, (lo.y + hi.y) * 0.5);
        lighting.dark.push((
            xf * Transform::from_translation(centre),
            Vec3::new((hi.x - lo.x) * 0.5, (y1 - y0) * 0.5, (hi.y - lo.y) * 0.5),
        ));
        for room in l
            .rooms
            .iter()
            .filter(|room| matches!(room.lamp, ug::Lamp::Steady | ug::Lamp::Flicker))
            .take(LAMP_BUDGET)
        {
            let at = Vec3::new(room.at[0], l.floor_of(room) + room.height - 0.2, room.at[1]);
            lighting
                .lamps
                .push((xf.transform_point(at), room.size[0].max(room.size[1]) + 4.0));
        }
        let rounds = l.residents();
        if rounds.is_empty() {
            continue;
        }
        // Their grid is the site's, without the ground round the kiosks:
        // residents stay below.
        let mut grid = plan.grid.clone();
        let mut floors = plan.levels.clone().unwrap_or_default();
        for (k, out) in plan.outside.iter().enumerate() {
            if *out {
                grid.cost[k] = extraction_core::nav::BLOCKED;
                if let Some(f) = floors.get_mut(k) {
                    *f = f32::NAN;
                }
            }
        }
        // Nor into deep water, as on the surface.
        for j in 0..grid.n {
            for i in 0..grid.n {
                let c = grid.centre(i, j);
                if r.water_at(c) >= ug::DEEP {
                    grid.cost[j * grid.n + i] = extraction_core::nav::BLOCKED;
                }
            }
        }
        let nav = crate::robots::indoor::LocalNav {
            grid: std::sync::Arc::new(grid),
            xf,
            floors: Some(std::sync::Arc::new(floors)),
        };
        for (rooms, crawlers) in rounds {
            let points: Vec<(&ug::Room, [f32; 2])> = match rooms {
                // One room: round its quarters.
                [id] => l
                    .rooms
                    .iter()
                    .filter(|room| &room.id == id)
                    .flat_map(|room| {
                        let q = [room.size[0] * 0.25, room.size[1] * 0.25];
                        [[-q[0], -q[1]], [q[0], -q[1]], [q[0], q[1]], [-q[0], q[1]]]
                            .map(|d| (room, [room.at[0] + d[0], room.at[1] + d[1]]))
                    })
                    .collect(),
                _ => rooms
                    .iter()
                    .filter_map(|id| l.rooms.iter().find(|room| &room.id == id))
                    .map(|room| (room, room.at))
                    .collect(),
            };
            let patrol: Vec<Vec3> = points
                .into_iter()
                .filter_map(|(room, at)| nav.grid.nearest_walkable(at).map(|p| (room, p)))
                .map(|(room, p)| xf.transform_point(Vec3::new(p[0], l.floor_of(room), p[1])))
                .collect();
            let Some(&spawn) = patrol.first() else {
                continue;
            };
            residents.sites.push(crate::robots::indoor::IndoorSite {
                nav: nav.clone(),
                patrol,
                spawn,
                crawlers,
                wardens: 0,
                dormant: false,
            });
        }
    }
}

/// Client: the sites' emergency lamps, and the daylight taken away below
/// ground.
pub struct UndergroundVisualsPlugin;

impl Plugin for UndergroundVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                light_lamps,
                flicker,
                torch,
                near_doors_only,
                show_below,
                water_surfaces,
                kept_crate_looks,
            ),
        )
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
    /// On a stage of its site's power (dark until it's on).
    pub site: Option<(Entity, usize)>,
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
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        let lit = l
            .rooms
            .iter()
            .filter(|room| {
                matches!(
                    room.lamp,
                    ug::Lamp::Steady | ug::Lamp::Flicker | ug::Lamp::Powered
                )
            })
            .take(LAMP_BUDGET);
        for (k, room) in lit.enumerate() {
            let at = Vec3::new(room.at[0], l.floor_of(room) + room.height - 0.2, room.at[1]);
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
                        site: (room.lamp == ug::Lamp::Powered).then_some((e, room.stage)),
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
                    bevy::camera::visibility::VisibilityRange {
                        start_margin: 0.0..0.0,
                        end_margin: (DOOR_REACH * 0.8)..DOOR_REACH,
                        use_aabb: false,
                    },
                    Transform::from_xyz(0.0, 0.03, 0.0),
                ));
        }
    }
}

/// Client: the flooded rooms' and corridors' water, a dark still surface
/// (it moves only when you do: the splashing is sound).
fn water_surfaces(
    mut commands: Commands,
    buildings: Query<(Entity, &super::buildings::Building), Added<super::buildings::Building>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, b) in &buildings {
        let Some(r) = ug::raster(&b.prefab) else {
            continue;
        };
        if r.water.iter().all(|w| *w <= 0.0) {
            continue;
        }
        let water = materials.add(StandardMaterial {
            base_color: Color::srgba(0.05, 0.075, 0.07, 0.9),
            perceptual_roughness: 0.06,
            reflectance: 0.7,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        // By the water's surface (cm): each level's floods apart.
        let class = |i: usize, j: usize| {
            let c = j * r.nx + i;
            let w = r.water[c];
            (w > 0.0).then(|| ((r.floors[c] + w) * 100.0).round() as i32)
        };
        for (lo, hi, level) in rects(r, class) {
            let y = level as f32 / 100.0;
            let (size, centre) = rect_box(r, lo, hi, y - 0.01, y);
            commands.spawn((
                Name::new("Flood"),
                Mesh3d(meshes.add(Plane3d::default().mesh().size(size.x, size.z))),
                MeshMaterial3d(water.clone()),
                bevy::light::NotShadowCaster,
                Below,
                Transform::from_xyz(centre.x, y, centre.z),
                ChildOf(e),
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
    power: Query<&SitePower>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let eye = eyes
        .iter()
        .filter(|(_, c)| c.is_active)
        .map(|(g, _)| g.translation())
        .next();
    for (lamp, at, mut light, mut vis) in &mut lamps {
        let powered = lamp
            .site
            .is_none_or(|(s, k)| power.get(s).is_ok_and(|p| p.on(k)));
        let near = powered && eye.is_none_or(|e| e.distance(at.translation()) < LAMP_RANGE);
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

/// A site's geometry deeper than this (building space y) is drawn apart
/// from its kiosks ([`Below`]).
pub const BELOW_Y: f32 = -1.0;
/// From this near a kiosk you might see down its stair.
const KIOSK_SIGHT: f32 = 18.0;

/// A site's rooms and stairs' meshes (merged, below [`BELOW_Y`]): drawn
/// only while the eye is underground or near one of its kiosks. Nothing
/// culls by the rock over them, and a site under a village counted in
/// every view of it.
#[derive(Component)]
pub struct Below;

/// Below-ground meshes shown or hidden with where the eye is.
fn show_below(
    zone: Res<Zone>,
    eyes: Query<(&GlobalTransform, &Camera), With<Camera3d>>,
    sites: Query<(Entity, &super::buildings::Building, &Transform, &Children)>,
    mut below: Query<&mut Visibility, With<Below>>,
) {
    let Some(eye) = eyes
        .iter()
        .filter(|(_, c)| c.is_active)
        .map(|(g, _)| g.translation())
        .next()
    else {
        return;
    };
    let underground = floor_under(&zone, eye).is_some();
    for (_, b, xf, children) in &sites {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        let near = l.portals.iter().any(|p| {
            let at = xf.transform_point(Vec3::new(p.at[0], p.y, p.at[1]));
            at.distance(eye) < KIOSK_SIGHT
        });
        let want = if underground || near {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        for c in children.iter() {
            if let Ok(mut v) = below.get_mut(c) {
                if *v != want {
                    *v = want;
                }
            }
        }
    }
}

/// How near the eye a door down in a site is drawn: from the surface
/// nothing shows them (the rock's in the way, but nothing culls by it),
/// and below, the rooms are small.
const DOOR_REACH: f32 = 25.0;
/// How near a kiosk's door leaf is drawn (the kiosk itself, always).
const KIOSK_DOOR_REACH: f32 = 50.0;

/// A site's doors are drawn only near the eye: below ground, its
/// dozen-odd leaves (each a handful of meshes) counted in every view of
/// the village over the shelter; a kiosk's, past 50 m, is a pixel or two.
#[allow(clippy::type_complexity)]
fn near_doors_only(
    mut commands: Commands,
    added: Query<
        Entity,
        (
            Added<Mesh3d>,
            Without<bevy::camera::visibility::VisibilityRange>,
        ),
    >,
    parents: Query<&ChildOf>,
    leaves: Query<&super::buildings::DoorLeaf>,
    sites: Query<&super::buildings::Building>,
) {
    for e in &added {
        // The mesh, its door (a parent or two up), the door's building.
        let mut at = e;
        for _ in 0..3 {
            let Ok(p) = parents.get(at) else { break };
            at = p.parent();
            let Ok(leaf) = leaves.get(at) else { continue };
            let site = parents
                .get(at)
                .ok()
                .and_then(|b| sites.get(b.parent()).ok())
                .is_some_and(|b| ug::is_site(&b.prefab));
            if site {
                // Down below, near only; a kiosk's door, as far as a door
                // that small shows.
                let reach = if leaf.0.hinge.y < BELOW_Y {
                    DOOR_REACH
                } else {
                    KIOSK_DOOR_REACH
                };
                commands
                    .entity(e)
                    .try_insert(bevy::camera::visibility::VisibilityRange {
                        start_margin: 0.0..0.0,
                        end_margin: (reach * 0.8)..reach,
                        use_aabb: false,
                    });
            }
            break;
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
pub(crate) struct Daylight {
    written: Option<(f32, f32)>,
    base: (f32, f32),
    fog: Option<Color>,
}

/// How far below ground the eye is, 0 (on the surface) to 1 (a few
/// metres down a stair or further): ambient light, the sun and the fog's
/// colour go with it, so the rooms are as dark as their lamps leave them.
pub(crate) fn go_dark(
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

/// Inside a site (below the ground, on its floor): `Some`, with the water
/// over the floor there, if any (world y of its surface). Outside every
/// site: `None`, and the ground's own water applies.
pub fn site_water(zone: &Zone, feet: Vec3) -> Option<Option<f32>> {
    for site in ug::placed(&zone.map) {
        let Some(r) = ug::raster(&site.id) else {
            continue;
        };
        let local = site.to_site([feet.x, feet.z]);
        let Some(f) = r.floor_at(local) else { continue };
        let footing = zone.hm.footing(&site.id, site.at, site.rot);
        let floor = footing + f;
        if feet.y > floor - 0.5 && feet.y < floor + 4.5 && floor < zone.ground(feet.x, feet.z) - 0.5
        {
            let w = r.water_at(local);
            return Some((w > 0.0).then_some(floor + w));
        }
    }
    None
}

/// Where the floor is under a world point, if it's inside an underground
/// site (on a stair or in a room, below the ground): the world y.
pub fn floor_under(zone: &Zone, p: Vec3) -> Option<f32> {
    site_under(zone, p).map(|s| s.1)
}

/// The site a world point is down in, and its floor there.
pub fn site_under(zone: &Zone, p: Vec3) -> Option<(String, f32)> {
    for site in ug::placed(&zone.map) {
        let r = ug::raster(&site.id)?;
        let footing = zone.hm.footing(&site.id, site.at, site.rot);
        let local = site.to_site([p.x, p.z]);
        let Some(f) = r.floor_at(local) else { continue };
        let floor = footing + f;
        if p.y > floor - 0.5 && p.y < floor + 4.5 && floor < zone.ground(p.x, p.z) - 0.5 {
            return Some((site.id, floor));
        }
    }
    None
}

/// The Depot's site id.
pub const DEPOT: &str = "depot_marrow";

/// A player has gone down into the Depot (each time they do; the first
/// of a raid is the reader's to pick out).
#[derive(Message, Clone, Copy, Debug)]
pub struct DepotEntered {
    pub player: Entity,
}

/// Something a player picked up in the Depot has left it with them (in
/// a hand, on the body or in the pack), as they came out.
#[derive(Message, Clone, Copy, Debug)]
pub struct CarriedOutOfDepot {
    pub player: Entity,
    pub item: Entity,
}

/// A kept crate's size: long (a person's) or small.
fn kept_size(long: bool) -> Vec3 {
    if long {
        Vec3::new(2.1, 0.65, 0.7)
    } else {
        Vec3::new(0.8, 0.5, 0.6)
    }
}

/// A crate the machines keep things in "pending review": shut until the
/// release (Story's epilogue sets `open`). Its tag says what's in it.
#[derive(Component, Clone, Debug)]
pub struct KeptCrate {
    pub tag: String,
    pub open: bool,
    /// A long one (a person's), or small.
    pub long: bool,
}

/// Where the small crate's collar disc hangs, in the crate's own space
/// (`pt_disc` of `depot_crate_small`).
pub const KEPT_DISC: Vec3 = Vec3::new(0.24, 0.174, -0.304);

/// Logic, once a site's built: its kept crates, each its own entity in
/// the site's space (the building draws only their colliders).
fn spawn_kept_crates(
    mut commands: Commands,
    buildings: Query<(Entity, &super::buildings::Building), Added<super::buildings::Building>>,
) {
    for (site, b) in &buildings {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        for room in &l.rooms {
            for p in room
                .props
                .iter()
                .filter(|p| matches!(p.kind, PropKind::KeptCrate | PropKind::KeptCrateSmall))
            {
                let at = Vec3::new(room.at[0] + p.at[0], l.floor_of(room), room.at[1] + p.at[1]);
                commands.spawn((
                    Name::new(format!("Kept crate {}", p.tag.as_deref().unwrap_or(""))),
                    KeptCrate {
                        tag: p.tag.clone().unwrap_or_default(),
                        open: false,
                        long: p.kind == PropKind::KeptCrate,
                    },
                    Transform::from_translation(at)
                        .with_rotation(Quat::from_rotation_y(-p.rot.to_radians())),
                    Visibility::default(),
                    ChildOf(site),
                ));
            }
        }
    }
}

/// Client: a kept crate drawn shut or open (its model, Kit's `_open` one
/// once there is one; till then, boxes with the lid leant off the back).
fn kept_crate_looks(
    mut commands: Commands,
    crates: Query<(Entity, &KeptCrate, Option<&Children>), Changed<KeptCrate>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mat: Local<Option<Handle<StandardMaterial>>>,
) {
    use crate::render::world_props::{Blockout, SwapIn};
    for (e, c, children) in &crates {
        for k in children.into_iter().flatten() {
            commands.entity(*k).despawn();
        }
        let mat = mat
            .get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.82, 0.83, 0.82),
                    perceptual_roughness: 0.6,
                    ..default()
                })
            })
            .clone();
        let size = kept_size(c.long);
        let lid = 0.08;
        let body = Vec3::new(size.x, size.y - lid, size.z);
        commands.spawn((
            Blockout,
            Mesh3d(meshes.add(Cuboid::from_size(body))),
            MeshMaterial3d(mat.clone()),
            Transform::from_xyz(0.0, body.y * 0.5, 0.0),
            ChildOf(e),
        ));
        // The lid: on top, or hinged up off the back edge and leant back.
        let hinge = Vec3::new(0.0, body.y, size.z * 0.5);
        let turn = if c.open { 100f32.to_radians() } else { 0.0 };
        commands.spawn((
            Blockout,
            Mesh3d(meshes.add(Cuboid::new(size.x, lid, size.z))),
            MeshMaterial3d(mat),
            Transform::from_translation(hinge).with_rotation(Quat::from_rotation_x(turn))
                * Transform::from_xyz(0.0, lid * 0.5, -size.z * 0.5),
            ChildOf(e),
        ));
        let model = match (c.long, c.open) {
            (true, false) => "depot_crate_long",
            (false, false) => "depot_crate_small",
            (true, true) => "depot_crate_long_open",
            (false, true) => "depot_crate_small_open",
        };
        commands.entity(e).insert(SwapIn::one(model));
    }
}

/// Something the convoys carried off, lying in the Depot's sorting hall.
#[derive(Component, Debug)]
pub struct LostKit;

/// How many of the lost things lie in the hall (the latest).
pub const LOST_SHOWN: usize = 16;

/// Logic, once: what the memorial convoys took (`Memorial::lost`, the
/// latest first) lies in the sorting hall's bins, on the crates and
/// pallets, for the taking back.
fn fill_lost_bins(
    mut commands: Commands,
    buildings: Query<(&super::buildings::Building, &Transform)>,
    memorial: Option<Res<crate::persist::Memorial>>,
    catalog: Res<crate::items::Catalog>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some((_, xf)) = buildings.iter().find(|(b, _)| b.prefab == DEPOT) else {
        return;
    };
    *done = true;
    let (Some(memorial), Some(l)) = (memorial, ug::layout(DEPOT)) else {
        return;
    };
    let Some(hall) = l.rooms.iter().find(|r| r.id == "sorting_hall") else {
        return;
    };
    let bins: Vec<Vec2> = hall
        .props
        .iter()
        .filter(|p| matches!(p.kind, PropKind::Bin | PropKind::Crate | PropKind::Pallet))
        .map(|p| Vec2::new(hall.at[0] + p.at[0], hall.at[1] + p.at[1]))
        .collect();
    if bins.is_empty() {
        return;
    }
    let kinds = memorial.0.lost.iter().rev().filter_map(|name| {
        catalog
            .iter()
            .find(|(_, d)| d.name == *name)
            .map(|(k, _)| k)
    });
    for (n, kind) in kinds.take(LOST_SHOWN).enumerate() {
        // Round the bins, a few to each, a little apart.
        let bin = bins[n % bins.len()];
        let k = (n / bins.len()) as f32;
        let at = Vec3::new(
            bin.x - 0.25 + 0.25 * (k % 3.0),
            l.floor_of(hall) + 1.3 + 0.2 * k,
            bin.y - 0.15 + 0.15 * (k % 2.0),
        );
        let e = crate::items::spawn_item(
            &mut commands,
            &catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::from_translation(xf.transform_point(at)),
                condition: 0.8,
            },
        );
        commands.entity(e).insert(LostKit);
    }
}

/// A site's own walking grid (its residents': the rooms, corridors and
/// stairs below the kiosk doors, not deep water), found by the site's
/// transform. For anyone else who walks it (Lev, in the epilogue).
pub fn site_nav<'a>(
    sites: &'a crate::robots::indoor::IndoorSites,
    site: &Transform,
) -> Option<&'a crate::robots::indoor::LocalNav> {
    sites
        .sites
        .iter()
        .map(|s| &s.nav)
        .find(|n| n.xf.translation.distance(site.translation) < 0.01)
}

/// Logic: who's in the Depot, what they had on them going in, and what
/// they came out with that they hadn't.
fn depot_visits(
    zone: Res<Zone>,
    players: Query<(Entity, &GlobalTransform), With<crate::player::Player>>,
    carried: Query<(Entity, &crate::anomaly::artifacts::CarriedBy)>,
    mut inside: Local<HashMap<Entity, HashSet<Entity>>>,
    mut entered: MessageWriter<DepotEntered>,
    mut out: MessageWriter<CarriedOutOfDepot>,
) {
    for (player, gt) in &players {
        let here = site_under(&zone, gt.translation()).is_some_and(|s| s.0 == DEPOT);
        let on = |p: Entity| {
            carried
                .iter()
                .filter(move |(_, c)| c.0 == p)
                .map(|(e, _)| e)
        };
        match (here, inside.contains_key(&player)) {
            (true, false) => {
                inside.insert(player, on(player).collect());
                entered.write(DepotEntered { player });
            }
            (false, true) => {
                let had = inside.remove(&player).unwrap_or_default();
                for item in on(player).filter(|e| !had.contains(e)) {
                    out.write(CarriedOutOfDepot { player, item });
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::LaunchOptions;
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

    /// A corridor's door swings open into its room, not back into the
    /// corridor (narrow: the leaf would pin whoever's coming through).
    #[test]
    fn doors_swing_into_their_rooms() {
        for id in ids() {
            let (r, g) = (ug::raster(id).unwrap(), geo(id).unwrap());
            let first = r.portals.len();
            for (k, d) in r.doors.iter().enumerate() {
                let Some(room) = d.room else { continue };
                let leaf = &g.doors[first + k];
                let open = Quat::from_rotation_y(leaf.yaw + 1.75) * Vec3::X * leaf.width * 0.9;
                let tip = leaf.hinge + open;
                let name = &ug::layout(id).unwrap().rooms[room as usize].id;
                assert_eq!(
                    r.at([tip.x, tip.z]),
                    Cell::Room(room),
                    "{id}: the door into `{name}` at {:?} swings to {tip:?}",
                    d.at
                );
                // Its whole swing clear of furniture and walls.
                for a in [0.3f32, 0.6, 0.9, 1.2, 1.5, 1.75] {
                    for f in [0.35f32, 0.65, 0.95] {
                        let p = leaf.hinge
                            + Quat::from_rotation_y(leaf.yaw + a) * Vec3::X * leaf.width * f
                            + Vec3::Y * 1.0;
                        let hit = g.pieces.iter().find(|piece| {
                            let Shape::Box(size) = piece.shape else {
                                return false;
                            };
                            let l = piece.at.compute_affine().inverse().transform_point3(p);
                            piece.collide && l.abs().cmplt(size * 0.5 - 0.02).all()
                        });
                        assert!(
                            hit.is_none(),
                            "{id}: the door into `{name}` at {:?} swings into {:?} at {p:?}",
                            d.at,
                            hit.map(|h| (h.shape, h.at.translation))
                        );
                    }
                }
            }
        }
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
            assert!(!sites.is_empty(), "{map}: a site");
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
                // Every half metre, at head height and just under the
                // ceiling (where a slit between a lintel and a wall would
                // be).
                for j in (0..r.nz).step_by(2) {
                    for i in (0..r.nx).step_by(2) {
                        if r.get(i, j).flat() {
                            let c = r.centre(i, j);
                            let top = r.ceil[j * r.nx + i] - 0.15;
                            for y in [1.5, top] {
                                from.push(xf.transform_point(Vec3::new(
                                    c[0],
                                    r.floors[j * r.nx + i] + y,
                                    c[1],
                                )));
                            }
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

    /// The robots know each site is dark and where its lamps are, and the
    /// site's resident crawlers come out on their round, below ground, and
    /// walk it.
    #[test]
    fn crawlers_walk_their_rounds_in_the_dark() {
        use crate::robots::indoor::Crawler;
        use crate::robots::senses::Lighting;
        for map in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let opts = LaunchOptions {
                robots: true,
                map: map.to_string(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..30 {
                app.update();
            }
            let zone = app.world().resource::<Zone>().clone();
            let sites = sites_on(&app);
            let crawlers: u32 = sites
                .iter()
                .flat_map(|(s, _)| ug::layout(&s.id).unwrap().residents())
                .map(|r| r.1)
                .sum();
            for (site, xf) in &sites {
                let l = ug::layout(&site.id).unwrap();
                let lighting = app.world().resource::<Lighting>();
                for room in &l.rooms {
                    let at = xf.transform_point(Vec3::new(
                        room.at[0],
                        l.floor_of(room) + 1.5,
                        room.at[1],
                    ));
                    assert!(lighting.in_dark(at), "{map} {}: not dark", room.id);
                    let lit = lighting.lamplight(at);
                    match room.lamp {
                        ug::Lamp::Steady | ug::Lamp::Flicker => {
                            assert!(lit > 0.2, "{map} {}: its lamp lights it {lit}", room.id)
                        }
                        _ => {}
                    }
                }
                let ground = xf.transform_point(Vec3::new(0.0, 1.5, -6.0));
                assert!(!lighting.in_dark(ground), "{map}: dark out on the surface");
            }
            let where_ = |app: &mut App| -> Vec<Vec3> {
                app.world_mut()
                    .query_filtered::<&Transform, With<Crawler>>()
                    .iter(app.world())
                    .map(|t| t.translation)
                    .collect()
            };
            let start = where_(&mut app);
            assert_eq!(start.len(), crawlers as usize, "{map}: crawlers out");
            for _ in 0..90 * 40 {
                app.update();
            }
            let end = where_(&mut app);
            assert_eq!(end.len(), start.len(), "{map}: a crawler went missing");
            let moved = start
                .iter()
                .zip(&end)
                .map(|(a, b)| a.xz().distance(b.xz()))
                .fold(0.0, f32::max);
            assert!(
                moved > 3.0,
                "{map}: no crawler walked its round ({moved:.1} m)"
            );
            for p in &end {
                assert!(
                    floor_under(&zone, *p + Vec3::Y * 0.3).is_some()
                        && p.y < zone.ground(p.x, p.z) - 2.0,
                    "{map}: a crawler out of the site at {p:?}"
                );
            }
        }
    }

    /// In the mine's unlit main drive, 10 m in front of a crawler, you're
    /// nothing; a torch on you, and it has you. (The site's own dark and
    /// grid, as registered at build.)
    #[test]
    fn underground_a_crawler_sees_you_by_your_torch() {
        use crate::robots::Awareness;
        use bevy::ecs::world::CommandQueue;
        let mut app = app("kettle_pass");
        for _ in 0..3 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let site = app
            .world()
            .resource::<crate::robots::indoor::IndoorSites>()
            .sites
            .first()
            .expect("the mine registered")
            .clone();
        let (placed, xf) = sites_on(&app).remove(0);
        let r = ug::raster(&placed.id).unwrap();
        let at = |x: f32, z: f32| xf.transform_point(Vec3::new(x, r.floor, z));
        let (them, me) = (at(-0.4, 25.5), at(-0.4, 15.0));
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = me + Vec3::Y * 0.05;
        // The head at eye height, as a headset or the desk rig keeps it
        // (headless, nothing drives it off the floor).
        app.world_mut()
            .get_mut::<crate::rig::RigInput>(player)
            .unwrap()
            .head = Transform::from_xyz(0.0, 1.6, 0.0);
        let mut queue = CommandQueue::default();
        let crawler = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::robots::indoor::spawn_crawler(&mut c, &zone, &site, them, vec![])
        };
        queue.apply(app.world_mut());
        for _ in 0..90 * 6 {
            app.update();
            let aware = app.world().get::<Awareness>(crawler).unwrap();
            // (It may hear you land on the concrete; it can't see you.)
            assert!(!aware.can_see_target, "in the dark: {aware:?}");
        }
        // A lit torch at the head, pointed at the crawler.
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let mut queue = CommandQueue::default();
        let (torch, cell) = {
            let mut c = Commands::new(&mut queue, app.world());
            let mut spawn = |id: &str| {
                crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind: catalog.kind(id).unwrap(),
                        transform: Transform::from_translation(me + Vec3::Y * 1.5),
                        condition: 1.0,
                    },
                )
            };
            (spawn("flashlight"), spawn("power_cell"))
        };
        queue.apply(app.world_mut());
        app.update();
        let socket = app
            .world_mut()
            .query::<(Entity, &crate::interact::SocketOwner)>()
            .iter(app.world())
            .find(|(_, o)| o.0 == torch)
            .map(|(e, _)| e)
            .expect("the torch's cell socket");
        {
            let mut q = bevy::ecs::system::SystemState::<(
                Commands,
                Query<&mut crate::interact::Socket>,
            )>::new(app.world_mut());
            let (mut c, mut sockets) = q.get_mut(app.world_mut()).unwrap();
            let mut s = sockets.get_mut(socket).unwrap();
            crate::interact::seat_in_socket(&mut c, cell, socket, &mut s);
            q.apply(app.world_mut());
        }
        app.world_mut()
            .entity_mut(torch)
            .insert(RigidBody::Kinematic);
        app.world_mut().write_message(crate::items::usage::UseItem {
            player: Entity::PLACEHOLDER,
            item: torch,
        });
        let aim =
            Transform::from_translation(me + Vec3::Y * 1.5).looking_at(them + Vec3::Y, Vec3::Y);
        let mut seen = false;
        for _ in 0..90 * 5 {
            *app.world_mut().get_mut::<Transform>(torch).unwrap() = aim;
            app.update();
            seen |= app
                .world()
                .get::<Awareness>(crawler)
                .unwrap()
                .can_see_target;
        }
        assert!(seen, "the torch gave you away");
    }

    /// Power, through the headset: in every site the powered doors won't
    /// open; a switch out of turn does nothing; a hand squeezed on each
    /// stage's breaker or generator pull-start, in order, turns it on;
    /// then the robots know its lamps, and its doors open.
    #[test]
    fn a_hand_on_the_switch_powers_the_site() {
        use crate::interact::InteractEvent;
        use crate::rig::Side;
        use crate::world::buildings::Door;
        for map in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let mut vr = crate::vr_harness::Vr::new(map);
            vr.step(3);
            let player = vr.player;
            let sites: Vec<Entity> = vr
                .app
                .world_mut()
                .query_filtered::<Entity, With<SitePower>>()
                .iter(vr.app.world())
                .collect();
            assert!(!sites.is_empty(), "{map}: a powered site");
            for site in sites {
                let id = vr
                    .app
                    .world()
                    .get::<super::super::buildings::Building>(site)
                    .unwrap()
                    .prefab
                    .clone();
                let l = ug::layout(&id).unwrap();
                let switches: Vec<(Entity, GlobalTransform)> = (0..l.power.len())
                    .map(|k| {
                        let mut q = vr
                            .app
                            .world_mut()
                            .query::<(Entity, &PowerSwitch, &GlobalTransform)>();
                        q.iter(vr.app.world())
                            .find(|(_, sw, _)| sw.site == site && sw.stage == k)
                            .map(|(e, _, g)| (e, *g))
                            .unwrap_or_else(|| panic!("{id}: a switch for stage {k}"))
                    })
                    .collect();
                // Out of turn: the last stage's switch, thrown first.
                if switches.len() > 1 {
                    let (last, _) = switches[switches.len() - 1];
                    vr.app
                        .world_mut()
                        .write_message(InteractEvent::Pressed { item: last, player });
                    vr.step(3);
                    let power = vr.app.world().get::<SitePower>(site).unwrap();
                    assert!(
                        power.stages.iter().all(|s| !s.on),
                        "{id}: a switch out of turn did something"
                    );
                }
                for (stage, (switch, at)) in switches.into_iter().enumerate() {
                    let held: Vec<Entity> = vr
                        .app
                        .world_mut()
                        .query::<(Entity, &PowerLocked)>()
                        .iter(vr.app.world())
                        .filter(|(_, l)| l.site == site && l.stage == stage)
                        .map(|(e, _)| e)
                        .collect();
                    assert!(!held.is_empty(), "{id}: a door on stage {stage}");
                    for d in &held {
                        vr.app
                            .world_mut()
                            .write_message(InteractEvent::Pressed { item: *d, player });
                    }
                    vr.step(60);
                    for d in &held {
                        assert!(
                            !vr.app.world().get::<Door>(*d).unwrap().open,
                            "{id}: held shut"
                        );
                    }
                    let lamps = vr
                        .app
                        .world()
                        .resource::<crate::robots::senses::Lighting>()
                        .lamps
                        .len();
                    // Stand in front of it, a hand on it, squeeze.
                    let handle = at.translation();
                    let out = at.rotation() * Vec3::NEG_Z;
                    let feet = handle + out * 0.6;
                    let zone = vr.app.world().resource::<Zone>().clone();
                    let floor = floor_under(&zone, feet).unwrap_or_else(|| {
                        panic!(
                            "{id}: the switch at {handle:?} (in front {feet:?}) isn't underground"
                        )
                    });
                    vr.stand(Vec3::new(feet.x, floor + 0.05, feet.z), 0.0);
                    vr.hand(Side::Right, handle, Quat::IDENTITY, 0.0, 0.0);
                    vr.step(2);
                    vr.grip(Side::Right, 1.0);
                    vr.step(3);
                    assert!(
                        vr.pressed().contains(&switch),
                        "{id}: switch {stage} answered the hand"
                    );
                    vr.grip(Side::Right, 0.0);
                    vr.step(5);
                    assert!(
                        vr.app.world().get::<SitePower>(site).unwrap().on(stage),
                        "{id}: stage {stage} on"
                    );
                    let now = vr
                        .app
                        .world()
                        .resource::<crate::robots::senses::Lighting>()
                        .lamps
                        .len();
                    let powered = l
                        .rooms
                        .iter()
                        .filter(|r| r.lamp == ug::Lamp::Powered && r.stage == stage)
                        .count();
                    assert_eq!(now, lamps + powered, "{id}: the robots see the new lamps");
                    for d in &held {
                        assert!(
                            vr.app.world().get::<PowerLocked>(*d).is_none(),
                            "{id}: unbolted"
                        );
                        vr.app
                            .world_mut()
                            .write_message(InteractEvent::Pressed { item: *d, player });
                    }
                    vr.step(90);
                    for d in &held {
                        assert!(
                            vr.app.world().get::<Door>(*d).unwrap().open,
                            "{id}: the door opens"
                        );
                    }
                }
            }
        }
    }

    /// Each valley's flooded corridor: standing in it you're wading (deep
    /// in Kettle's drowned drive, afloat); and the crawlers' grid stops at
    /// deep water while the bots' plan only costs it.
    #[test]
    fn floods_wade_swim_and_keep_crawlers_out() {
        for map in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let mut app = app(map);
            app.update();
            let (site, xf) = sites_on(&app).remove(0);
            let l = ug::layout(&site.id).unwrap();
            let r = ug::raster(&site.id).unwrap();
            let f = l
                .floods
                .iter()
                .find(|f| f.link.is_some())
                .expect("a flooded corridor");
            // The middle of the corridor's wettest cells.
            let cells: Vec<(usize, usize)> = (0..r.nz)
                .flat_map(|j| (0..r.nx).map(move |i| (i, j)))
                .filter(|&(i, j)| (r.water[j * r.nx + i] - f.depth).abs() < 1e-3)
                .collect();
            let (i, j) = cells[cells.len() / 2];
            let c = r.centre(i, j);
            let at = xf.transform_point(Vec3::new(c[0], r.floors[j * r.nx + i], c[1]));
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = at + Vec3::Y * 0.05;
            for _ in 0..90 {
                app.update();
            }
            let body = app.world().get::<crate::player::Body>(player).unwrap();
            assert!(
                body.wading > 0.5,
                "{map}: wading {:.2} in the flood",
                body.wading
            );
            assert_eq!(
                body.swimming,
                f.depth > crate::player::FLOAT_DEPTH - 0.05,
                "{map}: afloat in {} m",
                f.depth
            );
            let zone = app.world().resource::<Zone>().clone();
            let t = app.world().get::<Transform>(player).unwrap().translation;
            assert!(
                t.y < zone.ground(t.x, t.z) - 2.0,
                "{map}: still down in the corridor"
            );
            let residents = app.world().resource::<crate::robots::indoor::IndoorSites>();
            let nav = &residents.sites[0].nav;
            assert_eq!(
                nav.walkable(at + Vec3::Y * 0.1),
                f.depth < ug::DEEP,
                "{map}: crawlers and {} m of water",
                f.depth
            );
        }
    }

    /// Progression's hooks: going down into the Depot says so, and coming
    /// out with something picked up there says what (not what you took
    /// in).
    #[test]
    fn the_depot_knows_who_came_and_what_they_took() {
        let mut app = app("marrow_creek");
        for _ in 0..3 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let (site, xf) = sites_on(&app)
            .into_iter()
            .find(|(s, _)| s.id == DEPOT)
            .expect("the Depot on Marrow Creek");
        let l = ug::layout(&site.id).unwrap();
        let hall = l.rooms.iter().find(|r| r.id == "sorting_hall").unwrap();
        let down = xf.transform_point(Vec3::new(hall.at[0], l.floor_of(hall) + 0.1, hall.at[1]));
        assert_eq!(
            floor_under(&zone, down).map(|f| (f - down.y).abs() < 0.2),
            Some(true)
        );
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::player::Player>>()
            .iter(app.world())
            .next()
            .unwrap();
        let mine = app
            .world_mut()
            .spawn(crate::anomaly::artifacts::CarriedBy(player))
            .id();
        let count = |app: &mut App| {
            app.update();
            let e = app.world().resource::<Messages<DepotEntered>>();
            let o = app.world().resource::<Messages<CarriedOutOfDepot>>();
            (
                e.iter_current_update_messages().count(),
                o.iter_current_update_messages()
                    .map(|m| m.item)
                    .collect::<Vec<_>>(),
            )
        };
        let place = |app: &mut App, at: Vec3| {
            *app.world_mut().get_mut::<Transform>(player).unwrap() =
                Transform::from_translation(at);
        };
        place(&mut app, down);
        let mut entered = 0;
        for _ in 0..3 {
            entered += count(&mut app).0;
        }
        assert_eq!(entered, 1, "going down into the Depot");
        let found = app
            .world_mut()
            .spawn(crate::anomaly::artifacts::CarriedBy(player))
            .id();
        let up = zone.on_ground(xf.translation.x, xf.translation.z - 8.0) + Vec3::Y * 0.1;
        place(&mut app, up);
        let mut taken = Vec::new();
        for _ in 0..3 {
            taken.extend(count(&mut app).1);
        }
        assert_eq!(
            taken,
            vec![found],
            "out with what was found there, not {mine:?}"
        );
    }

    /// The kept crates are their own entities, tagged (Story opens them
    /// in the epilogue and finds K-14's by tag), standing on the floor.
    #[test]
    fn the_kept_crates_stand_tagged_in_the_depot() {
        let mut app = app("marrow_creek");
        let zone = app.world().resource::<Zone>().clone();
        let crates: Vec<(KeptCrate, Vec3)> = app
            .world_mut()
            .query::<(&KeptCrate, &GlobalTransform)>()
            .iter(app.world())
            .map(|(c, g)| (c.clone(), g.translation()))
            .collect();
        assert_eq!(crates.len(), 6, "{crates:?}");
        for tag in ["MAREK, H.", "MAREK dog", "HOLM, J."] {
            let (c, at) = crates
                .iter()
                .find(|(c, _)| c.tag == tag)
                .unwrap_or_else(|| panic!("no crate `{tag}`"));
            assert!(!c.open);
            assert_eq!(c.long, tag != "MAREK dog");
            let (site, floor) = site_under(&zone, *at + Vec3::Y * 0.2).expect("down in a site");
            assert_eq!(site, DEPOT);
            assert!(
                (at.y - floor).abs() < 0.05,
                "{tag} at {at:?}, floor {floor}"
            );
        }
        // Lev's way: the Depot's own grid, from inside the ramp's kiosk
        // to the tag office and bay K-14.
        let (_, xf) = sites_on(&app)
            .into_iter()
            .find(|(s, _)| s.id == DEPOT)
            .unwrap();
        let l = ug::layout(DEPOT).unwrap();
        let sites = app.world().resource::<crate::robots::indoor::IndoorSites>();
        let nav = site_nav(sites, &xf).expect("the Depot's grid");
        let from = xf.transform_point(Vec3::new(0.0, 0.15, -1.0));
        for id in ["tag_office", "bay_k14"] {
            let room = l.rooms.iter().find(|r| r.id == id).unwrap();
            let to = xf.transform_point(Vec3::new(room.at[0], l.floor_of(room), room.at[1]));
            let path = nav
                .path(from, to)
                .unwrap_or_else(|| panic!("no way from the ramp to {id}"));
            assert!(
                path.last().unwrap().xz().distance(to.xz()) < 1.0,
                "to {id}: {path:?}"
            );
        }
    }

    /// What the convoys took lies in the Depot's sorting hall, the latest
    /// first, and stays there (on the bins, not through the floor).
    #[test]
    fn the_lost_kit_lies_in_the_sorting_hall() {
        // (Set before the first frame, as a save is loaded at startup.)
        let mut app = crate::headless_app(
            LaunchOptions {
                robots: false,
                map: "marrow_creek".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let names: Vec<String> = catalog
            .iter()
            .take(LOST_SHOWN + 4)
            .map(|(_, d)| d.name.clone())
            .collect();
        app.world_mut()
            .resource_mut::<crate::persist::Memorial>()
            .0
            .lost = names.clone();
        for _ in 0..90 * 4 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let lost: Vec<(String, Vec3)> = app
            .world_mut()
            .query_filtered::<(&Name, &GlobalTransform), With<LostKit>>()
            .iter(app.world())
            .map(|(n, g)| (n.to_string(), g.translation()))
            .collect();
        assert_eq!(lost.len(), LOST_SHOWN, "{lost:?}");
        // The latest, not the oldest.
        assert!(lost.iter().any(|(n, _)| n == names.last().unwrap()));
        for (name, at) in &lost {
            let site = site_under(&zone, *at);
            assert!(
                site.as_ref()
                    .is_some_and(|s| s.0 == DEPOT && at.y > s.1 - 0.05),
                "{name} at {at:?}: {site:?}"
            );
        }
    }

    /// The Depot's deep job, end to end: with the Ledger job on the card the
    /// intake ledger lies in bay K-14; a bot goes in down the ramp, throws
    /// the three stages in turn, takes the ledger into its pack, swims the
    /// cistern and climbs out by the hatch, and the Depot says it carried
    /// the ledger out. (No robots: the crawlers are the other tests'.)
    #[test]
    fn the_ledger_comes_out_of_the_depot() {
        use crate::bot::{Bot, Task};
        use crate::rig::{LocalPlayer, Side};
        use crate::sim::indoor::{BuildingPlan, plan_buildings};
        use extraction_core::progression::{Job, LEDGER_ITEM};
        let mut app = crate::headless_app(
            LaunchOptions {
                robots: false,
                map: "marrow_creek".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.world_mut()
            .resource_mut::<crate::progression::Progression>()
            .0
            .job = Some(Job::Ledger);
        for _ in 0..90 * 3 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let kind = app
            .world()
            .resource::<crate::items::Catalog>()
            .kind(LEDGER_ITEM)
            .expect("the ledger in the catalog");
        let (ledger, at) = app
            .world_mut()
            .query::<(Entity, &crate::items::Item, &GlobalTransform)>()
            .iter(app.world())
            .find(|(_, i, _)| i.kind() == kind)
            .map(|(e, _, g)| (e, g.translation()))
            .expect("the ledger is out");
        let (site, floor) = site_under(&zone, at).expect("the ledger is down in a site");
        assert_eq!(site, DEPOT);
        assert!(
            (at.y - floor).abs() < 0.3,
            "on the floor: {at:?}, floor {floor}"
        );
        let (placed, xf) = sites_on(&app)
            .into_iter()
            .find(|(s, _)| s.id == DEPOT)
            .unwrap();
        let l = ug::layout(&placed.id).unwrap();
        let bay = l.rooms.iter().find(|r| r.id == "bay_k14").unwrap();
        let local = xf.compute_affine().inverse().transform_point3(at);
        assert!(
            (local.x - bay.at[0]).abs() < bay.size[0] * 0.5
                && (local.z - bay.at[1]).abs() < bay.size[1] * 0.5,
            "in bay K-14: {local:?}"
        );
        let plan: BuildingPlan = app
            .world_mut()
            .run_system_once(
                |b: Query<(Entity, &super::super::buildings::Building, &Transform)>,
                 d: Query<(Entity, &super::super::buildings::DoorLeaf, &ChildOf)>| {
                    plan_buildings(&b, &d)
                },
            )
            .unwrap()
            .into_iter()
            .find(|p| p.name == DEPOT)
            .unwrap();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
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
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = Vec3::new(start.x, zone.ground(start.x, start.z) + 0.1, start.z);
        app.update();
        let run = |app: &mut App, tasks: Vec<Task>, what: &str| {
            app.world_mut().entity_mut(player).insert(Bot::new(tasks));
            for _ in 0..90 * 150 {
                app.update();
                if app.world().get::<Bot>(player).unwrap().done() {
                    break;
                }
            }
            let bot = app.world().get::<Bot>(player).unwrap();
            assert_eq!(bot.failed, 0, "{what}: {:?}", bot.log);
        };
        let route = |app: &App, to: Vec2, within: f32| {
            let pos = app.world().get::<Transform>(player).unwrap().translation;
            crate::sim::raider::indoor_route(&plan, pos, to, within)
                .unwrap_or_else(|| panic!("no route from {pos:?} to {to:?}"))
        };
        let mut switches: Vec<(usize, Entity, Vec3)> = app
            .world_mut()
            .query::<(Entity, &PowerSwitch, &GlobalTransform)>()
            .iter(app.world())
            .filter(|(_, sw, _)| sw.site == plan.entity)
            .map(|(e, sw, g)| (sw.stage, e, g.translation()))
            .collect();
        switches.sort_by_key(|s| s.0);
        assert_eq!(switches.len(), 3, "three stages");
        for (stage, switch, at) in switches {
            let mut tasks = route(&app, plan.to_local(at).xz(), 0.5);
            tasks.push(Task::Press(switch));
            tasks.push(Task::Wait(1.0));
            run(&mut app, tasks, "to a switch");
            assert!(
                app.world().get::<SitePower>(plan.entity).unwrap().on(stage),
                "stage {stage} on"
            );
        }
        let at = app
            .world()
            .get::<GlobalTransform>(ledger)
            .unwrap()
            .translation();
        let mut tasks = route(&app, plan.to_local(at).xz(), 0.3);
        tasks.push(Task::Grab {
            item: ledger,
            side: Side::Right,
        });
        tasks.push(Task::Wait(0.5));
        tasks.push(Task::Stow(Side::Right));
        tasks.push(Task::Wait(0.5));
        run(&mut app, tasks, "to the ledger");
        assert_eq!(
            app.world()
                .get::<crate::anomaly::artifacts::CarriedBy>(ledger)
                .map(|c| c.0),
            Some(player),
            "the ledger on you"
        );
        let mut out = Vec::new();
        let tasks = route(&app, exit, 0.6);
        app.world_mut().entity_mut(player).insert(Bot::new(tasks));
        for _ in 0..90 * 150 {
            app.update();
            out.extend(
                app.world()
                    .resource::<Messages<CarriedOutOfDepot>>()
                    .iter_current_update_messages()
                    .map(|m| m.item),
            );
            if app.world().get::<Bot>(player).unwrap().done() {
                break;
            }
        }
        for _ in 0..10 {
            app.update();
            out.extend(
                app.world()
                    .resource::<Messages<CarriedOutOfDepot>>()
                    .iter_current_update_messages()
                    .map(|m| m.item),
            );
        }
        let at = app.world().get::<Transform>(player).unwrap().translation;
        assert!(
            plan.to_local(at).xz().distance(exit) < 1.2,
            "out by the hatch: {at:?}"
        );
        assert!(out.contains(&ledger), "carried the ledger out: {out:?}");
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
            let sites: Vec<BuildingPlan> = plans
                .iter()
                .filter(|p| ug::is_site(&p.name))
                .cloned()
                .collect();
            assert!(!sites.is_empty(), "{map}: a site plan");
            for plan in sites {
                let map = &plan.name;
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
                // On the power in stages: walk to each switch in turn and
                // throw it (the deepest loot, or the way out, is behind it).
                let mut switches: Vec<(usize, Entity, Vec3)> = app
                    .world_mut()
                    .query::<(Entity, &PowerSwitch, &GlobalTransform)>()
                    .iter(app.world())
                    .filter(|(_, sw, _)| sw.site == plan.entity)
                    .map(|(e, sw, g)| (sw.stage, e, g.translation()))
                    .collect();
                switches.sort_by_key(|s| s.0);
                for (stage, switch, at) in switches {
                    walk(&mut app, plan.to_local(at).xz(), 0.5, "to a switch");
                    app.world_mut()
                        .entity_mut(player)
                        .insert(Bot::new(vec![crate::bot::Task::Press(switch)]));
                    for _ in 0..90 * 3 {
                        app.update();
                    }
                    assert!(
                        app.world().get::<SitePower>(plan.entity).unwrap().on(stage),
                        "{map}: stage {stage} on"
                    );
                }
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
                // From anywhere in the site (the deepest room, the stairs under
                // the kiosks), the way out is one of the kiosks' doors.
                for c in (0..plan.grid.n * plan.grid.n).step_by(37) {
                    let (i, j) = (c % plan.grid.n, c / plan.grid.n);
                    if plan.grid.cost[c] == extraction_core::nav::BLOCKED || plan.outside[c] {
                        continue;
                    }
                    let from = Vec2::from_array(plan.grid.centre(i, j));
                    if let Some(to) = crate::sim::raider::way_out(&plan, from, &|_| false) {
                        assert!(
                            plan.entries.iter().any(|e| e.distance(to) < 0.1),
                            "{map}: from {from:?} the way out is {to:?}, not a kiosk door"
                        );
                    }
                }
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
}
