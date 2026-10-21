//! Rime Fell's structures (`docs/FIFTH_VALLEY.md`): the ore mill stepped
//! down the slope under the mine, the observatory on the north-west
//! summit, and the funicular's two stations and its car stopped on the
//! line. Cold-country mining works of the fifties to the seventies:
//! board and batten, corrugated iron, rubble stone and concrete.
//!
//! Built from the same [`Builder`] as every other prefab; the pieces are
//! their own visuals until models are made for them.
//!
//! The mill climbs the hill toward +Z in four bays, each a storey higher
//! than the one in front of it ([`MILL_FLOORS`]). It has no ground floor
//! of its own: [`stepped_floor`] is the floor under any point of it, for
//! the bots' indoor plans (`sim::indoor`).

use bevy::prelude::*;
use extraction_core::map::Entrance;

use super::buildings::{
    Builder, Geo, Opening, OpeningKind, Ruin, Shape, door, gap, wide_window, window,
};
use crate::render::materials::Mat;

/// Every prefab built here.
pub const PREFABS: [&str; 5] = [
    "ore_mill",
    "observatory",
    "funicular_lower",
    "funicular_upper",
    "funicular_car",
];

pub fn geo(id: &str) -> Option<Geo> {
    let mut b = Builder::default();
    match id {
        "ore_mill" => mill(&mut b),
        "observatory" => observatory(&mut b),
        "funicular_lower" => lower_station(&mut b),
        "funicular_upper" => upper_station(&mut b),
        "funicular_car" => car(&mut b),
        _ => return None,
    }
    Some(b.finish())
}

// ---- Shared pieces ------------------------------------------------------------

fn r2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0 + 0.0
}

/// An outside wall, as `Builder::wall`, its doors and doorways kept as the
/// prefab's entrances (`map::prefab_entrances`). Outside is the side
/// `Builder::shell`'s walls face, their corners taken clockwise seen from
/// above: going from `a` to `c` along +X, outside is -Z.
#[allow(clippy::too_many_arguments)]
fn outer(b: &mut Builder, a: Vec2, c: Vec2, y0: f32, h: f32, thick: f32, mat: Mat, ops: &[Opening]) {
    b.wall(a, c, y0, h, thick, mat, ops);
    let dir = (c - a).normalize();
    let out = Vec2::new(dir.y, -dir.x);
    for o in ops
        .iter()
        .filter(|o| matches!(o.kind, OpeningKind::Door | OpeningKind::Gap))
    {
        let m = a + dir * o.centre + out * (thick * 0.5);
        b.geo.entrances.push(Entrance {
            at: [r2(m.x), r2(m.y)],
            out: [r2(out.x), r2(out.y)],
            width: r2(o.width),
            clear: 1.0,
        });
    }
}

/// A small window high in a wall (dirty panes over a machine floor).
fn high(centre: f32) -> Opening {
    Opening {
        centre,
        width: 0.9,
        bottom: 3.1,
        top: 3.9,
        kind: OpeningKind::Window,
    }
}

fn pane(centre: f32, width: f32, bottom: f32, top: f32) -> Opening {
    Opening {
        centre,
        width,
        bottom,
        top,
        kind: OpeningKind::Window,
    }
}

/// Builds `f` in a frame of its own and adds it at `at` (a turn about Y
/// and an offset): for the `Builder`'s helpers that build round the
/// origin (`shell`, `plinth`, `gable_roof`).
fn place(b: &mut Builder, at: Transform, f: impl FnOnce(&mut Builder)) {
    let mut sub = Builder::default();
    f(&mut sub);
    let g = sub.finish();
    let base = b.geo.pieces.len();
    let yaw = at.rotation.to_euler(EulerRot::YXZ).0;
    b.geo.pieces.extend(g.pieces.into_iter().map(|mut p| {
        p.at = at * p.at;
        p
    }));
    b.geo.loot.extend(g.loot.into_iter().map(|mut l| {
        l.at = at.transform_point(l.at);
        l
    }));
    b.geo.doors.extend(g.doors.into_iter().map(|mut d| {
        d.hinge = at.transform_point(d.hinge);
        d.yaw += yaw;
        d
    }));
    b.geo
        .lights
        .extend(g.lights.into_iter().map(|p| at.transform_point(p)));
    b.geo.props.extend(g.props.into_iter().map(|mut d| {
        d.models = d.models.into_iter().map(|(n, t)| (n, at * t)).collect();
        d.pieces = d.pieces.start + base..d.pieces.end + base;
        d
    }));
    b.geo.entrances.extend(g.entrances.into_iter().map(|e| {
        let p = at.transform_point(Vec3::new(e.at[0], 0.0, e.at[1]));
        let o = at.rotation * Vec3::new(e.out[0], 0.0, e.out[1]);
        Entrance {
            at: [r2(p.x), r2(p.z)],
            out: [r2(o.x), r2(o.z)],
            ..e
        }
    }));
    b.geo.ladders.extend(g.ladders.into_iter().map(|mut l| {
        l.bottom = at.transform_point(l.bottom);
        l.top = at.transform_point(l.top);
        l.out = at.rotation * l.out;
        l.over = l.over.map(|o| at.transform_point(o));
        l
    }));
}

/// The turn about X that lays a box's +Z along `u` (a direction in the
/// y-z plane, given as (z, y)).
fn tilt(u: Vec2) -> Quat {
    Quat::from_rotation_x(-u.y.atan2(u.x))
}

/// A wall's sloped top over its square part, at `x`, `thick` across: a
/// slab whose top edge runs from `a` to `c` (each (z, y), `a` the nearer
/// -Z), reaching down past `base` (the square part's top) all along. It is
/// a little thinner than the wall it laps down into.
fn rake(b: &mut Builder, x: f32, a: Vec2, c: Vec2, base: f32, thick: f32, mat: Mat) {
    let d = c - a;
    let len = d.length();
    let u = d / len;
    let n = Vec2::new(-u.y, u.x);
    let hh = (a.y.max(c.y) - base) / u.x + 0.05;
    let mid = (a + c) * 0.5 - n * (hh * 0.5);
    b.piece(
        Shape::Box(Vec3::new(thick, hh, len)),
        Transform::from_xyz(x, mid.y, mid.x).with_rotation(tilt(u)),
        mat,
        true,
    );
}

/// A pitched slab (a shed roof) from `x0` to `x1`, its underside along
/// the line from `a` to `c` (each (z, y)).
fn pitched(b: &mut Builder, x0: f32, x1: f32, a: Vec2, c: Vec2, t: f32, mat: Mat) {
    let d = c - a;
    let len = d.length();
    let u = d / len;
    let n = Vec2::new(-u.y, u.x);
    let mid = (a + c) * 0.5 + n * (t * 0.5);
    b.piece(
        Shape::Box(Vec3::new(x1 - x0, t, len)),
        Transform::from_xyz((x0 + x1) * 0.5, mid.y, mid.x).with_rotation(tilt(u)),
        mat,
        true,
    );
}

/// A bar from `from` to `to`, `w` square.
fn rod(b: &mut Builder, from: Vec3, to: Vec3, w: f32, mat: Mat, collide: bool) {
    let d = to - from;
    let up = if d.normalize().y.abs() > 0.99 {
        Vec3::X
    } else {
        Vec3::Y
    };
    b.piece(
        Shape::Box(Vec3::new(w, w, d.length())),
        Transform::from_translation((from + to) * 0.5).looking_to(d, up),
        mat,
        collide,
    );
}

/// A ladder's rails and rungs against a wall, standing out toward `out`
/// from the line at `x`/`z`, from `y0` to `y1`; climbed with the hands
/// from `y0 + 0.3` to `y1 - 1.0`, which puts you at `over`.
fn ladder(b: &mut Builder, at: Vec2, out: Vec3, y0: f32, y1: f32, over: Vec3) {
    let side = Vec3::new(out.z, 0.0, -out.x) * 0.2;
    let foot = Vec3::new(at.x, y0, at.y);
    for s in [-1.0, 1.0] {
        let p = foot + side * s;
        b.bx(
            Vec3::new(0.05, y1 - y0, 0.05),
            p + Vec3::Y * ((y1 - y0) * 0.5),
            Mat::DarkMetal,
        );
    }
    let mut y = y0 + 0.3;
    while y < y1 - 0.05 {
        let w = if out.x.abs() > 0.5 {
            Vec3::new(0.03, 0.03, 0.4)
        } else {
            Vec3::new(0.4, 0.03, 0.03)
        };
        b.piece(
            Shape::Box(w),
            Transform::from_translation(foot + Vec3::Y * (y - y0)),
            Mat::DarkMetal,
            false,
        );
        y += 0.3;
    }
    b.ladder(
        foot + Vec3::Y * 0.3,
        foot + Vec3::Y * (y1 - y0 - 1.0),
        0.4,
        out,
        Some(over),
    );
}

/// A slatted bench `len` long across X, its back against the wall at
/// `back` (z), facing `face` (+1: toward +Z). Its slats are what you sit
/// on; the frame is iron.
fn slat_bench(b: &mut Builder, x: f32, y: f32, back: f32, face: f32, len: f32) {
    for k in 0..3 {
        let z = back + face * (0.1 + 0.13 * k as f32 + 0.065);
        b.bx(
            Vec3::new(len, 0.03, 0.11),
            Vec3::new(x, y + 0.435, z),
            Mat::Timber,
        );
    }
    for k in 0..2 {
        b.deco(
            Vec3::new(len, 0.1, 0.025),
            Vec3::new(x, y + 0.65 + 0.2 * k as f32, back + face * 0.0425),
            Mat::Timber,
        );
    }
    for sx in [-1.0, 1.0] {
        let fx = x + sx * (len * 0.5 - 0.12);
        b.bx(
            Vec3::new(0.04, 0.42, 0.4),
            Vec3::new(fx, y + 0.21, back + face * 0.3),
            Mat::DarkMetal,
        );
        b.deco(
            Vec3::new(0.04, 0.5, 0.03),
            Vec3::new(fx, y + 0.75, back + face * 0.015),
            Mat::DarkMetal,
        );
    }
}

// ---- The ore mill ---------------------------------------------------------------

/// Each bay's floor over the placement's origin: they step up the hill a
/// storey at a time, each a sill's height over its terrace (the map
/// terraces the ground at 0, 4, 8 and 12 m).
pub const MILL_FLOORS: [f32; 4] = [0.15, 4.15, 8.15, 12.15];
/// How deep each bay is (z), and the mill's half width (x) and half length.
const BAY: f32 = 10.0;
const MILL_HW: f32 = 10.0;
const MILL_HD: f32 = 20.0;
/// Outside walls and the bays' fronts: timber framing under corrugated
/// iron.
const WALL: f32 = 0.3;
/// The inside face of the side walls (|x|).
const INNER: f32 = MILL_HW - WALL;
/// A bay's shed roof over its floor: low at its front, high at its back.
const EAVE_FRONT: f32 = 5.0;
const EAVE_BACK: f32 = 6.6;
/// The headhouse over the top bay's west half: its eaves.
const HEAD_EAVE: f32 = 19.95;
/// The stairs from bay to bay: rise and run of a step, the flight's width.
pub const RISE: f32 = 0.2;
pub const TREAD: f32 = 0.28;
const STAIR_W: f32 = 1.3;

/// Where bay `k` starts (z): the face of its front wall.
fn bay_z0(k: usize) -> f32 {
    -MILL_HD + BAY * k as f32
}

/// Which bay a z is over: its front wall belongs to it (the doorway in
/// it is at its floor).
fn bay_of(z: f32) -> usize {
    (((z + MILL_HD + WALL * 0.5) / BAY).floor() as i32).clamp(0, 3) as usize
}

/// The stair up from bay `k` (0..3) to the next: it climbs along the
/// bay's back wall, toward +X from even bays and -X from odd ones, to a
/// landing at the side wall in front of the doorway up. Returns
/// (direction along x, the flight's foot |x|, the landing's inner edge
/// |x|, the flight's z extent).
fn stair_plan(k: usize) -> (f32, f32, f32, (f32, f32)) {
    let dir = if k % 2 == 0 { 1.0 } else { -1.0 };
    let zb = bay_z0(k) + BAY - WALL * 0.5;
    let land = INNER - STAIR_W;
    let steps = ((MILL_FLOORS[1] - MILL_FLOORS[0]) / RISE).round() as usize;
    let foot = land - (steps - 1) as f32 * TREAD;
    (dir, foot, land, (zb - STAIR_W, zb))
}

/// The floor under a point of a prefab that has more than one (the ore
/// mill), prefab-local; `None` for every other prefab. Outside the walls
/// it's where the ground is meant to be: the terrace beside each bay at
/// its floor, the yard in front at the lowest, the bank behind at the
/// highest.
pub fn stepped_floor(id: &str, p: Vec2) -> Option<f32> {
    if id != "ore_mill" {
        return None;
    }
    let (x, z) = (p.x, p.y);
    if z < -MILL_HD {
        return Some(MILL_FLOORS[0]);
    }
    if z > MILL_HD {
        return Some(MILL_FLOORS[3]);
    }
    let k = bay_of(z);
    if k < 3 {
        let (dir, foot, land, (za, zb)) = stair_plan(k);
        let ax = x * dir;
        if z >= za && z <= zb && ax >= foot && ax <= INNER {
            let f = MILL_FLOORS[k];
            if ax >= land {
                return Some(MILL_FLOORS[k + 1]);
            }
            let i = ((ax - foot) / TREAD).floor() + 1.0;
            return Some(f + RISE * i);
        }
    }
    Some(MILL_FLOORS[k])
}

/// The ore mill: four bays stepped up the slope, each a timber and
/// corrugated-iron hall on a concrete footing that holds the hill behind
/// the bay below, its own shed roof stepping up the hill, and the
/// headhouse over the top bay's west half with the ore bin in it. Ore came
/// in at the back door on the top floor and went down through the works
/// by gravity: the jaw crusher under the bin (top), the ball mill
/// (second), the flotation cells (third), and the concentrate shed at the
/// front, with the thickener and the foreman's office.
fn mill(b: &mut Builder) {
    for k in 0..4 {
        mill_bay(b, k);
    }
    mill_back(b);
    headhouse(b);
    for k in 0..3 {
        mill_stair(b, k);
    }
    concentrate_shed(b);
    flotation_floor(b);
    ball_mill_floor(b);
    crusher_floor(b);
}

/// One bay's footing, front wall, side walls and roof.
fn mill_bay(b: &mut Builder, k: usize) {
    let f = MILL_FLOORS[k];
    let z0 = bay_z0(k);
    let z1 = z0 + BAY;
    let roof = |z: f32| f + EAVE_FRONT + (EAVE_BACK - EAVE_FRONT) * (z - z0) / BAY;
    // The footing under the floor, down past the bay in front's floor:
    // its front face is that bay's back wall, holding the hill.
    let front = if k == 0 { z0 } else { z0 - WALL * 0.5 };
    let back = if k == 3 { z1 } else { z1 - WALL * 0.5 };
    let bottom = if k == 0 { -1.5 } else { MILL_FLOORS[k - 1] - 1.0 };
    b.bx(
        Vec3::new(2.0 * MILL_HW, f - bottom, back - front),
        Vec3::new(0.0, (f + bottom) * 0.5, (front + back) * 0.5),
        Mat::Concrete,
    );
    // The front: the mill's front door on the lowest, and above each
    // bay in front's roof a row of small high windows; inside, the
    // doorway at the top of the stair up from the bay in front.
    let fz = if k == 0 { z0 + WALL * 0.5 } else { z0 };
    let (a, c) = (Vec2::new(-MILL_HW, fz), Vec2::new(MILL_HW, fz));
    if k == 0 {
        outer(
            b,
            a,
            c,
            f,
            EAVE_FRONT + 0.05,
            WALL,
            Mat::Corrugated,
            &[high(3.0), door(7.0), high(12.0), high(16.5)],
        );
    } else {
        let (dir, _, _, _) = stair_plan(k - 1);
        let s = MILL_HW + dir * (INNER - STAIR_W * 0.5);
        b.wall(
            a,
            c,
            f,
            EAVE_FRONT + 0.05,
            WALL,
            Mat::Corrugated,
            &[gap(s, 1.1, 2.1), high(5.0), high(10.0), high(15.0)],
        );
    }
    // The sides: on the east (+X) a door out onto the bay's terrace in
    // the middle; on the top bay the west side is the headhouse's, up to
    // its eaves.
    let za = fz + WALL * 0.5;
    let zb = if k == 3 { z1 - WALL } else { z1 - WALL * 0.5 };
    let xs = MILL_HW - WALL * 0.5;
    let mid = z0 + BAY * 0.5;
    outer(
        b,
        Vec2::new(xs, za),
        Vec2::new(xs, zb),
        f,
        EAVE_FRONT,
        WALL,
        Mat::Corrugated,
        &[high(1.6), door(mid - za), high(zb - za - 1.6)],
    );
    let top = |z: f32| Vec2::new(z, roof(z) + 0.05);
    rake(
        b,
        xs,
        top(za),
        top(zb),
        f + EAVE_FRONT,
        WALL - 0.02,
        Mat::Corrugated,
    );
    if k == 3 {
        b.wall(
            Vec2::new(-xs, zb),
            Vec2::new(-xs, za),
            f,
            HEAD_EAVE - f,
            WALL,
            Mat::Corrugated,
            &[high(2.0), high(7.5), pane(4.75, 1.2, 5.9, 6.9)],
        );
    } else {
        b.wall(
            Vec2::new(-xs, zb),
            Vec2::new(-xs, za),
            f,
            EAVE_FRONT,
            WALL,
            Mat::Corrugated,
            &[high(2.5), high(7.0)],
        );
        rake(
            b,
            -xs,
            top(za),
            top(zb),
            f + EAVE_FRONT,
            WALL - 0.02,
            Mat::Corrugated,
        );
    }
    // The roof, over the front wall onto the bay in front's (on the top
    // bay, east of the headhouse and out over the back wall), and its
    // gutter board along the front.
    let (x0, x1) = if k == 3 {
        (-2.0, MILL_HW + 0.3)
    } else {
        (-MILL_HW - 0.3, MILL_HW + 0.3)
    };
    let zf = fz - WALL * 0.5 - 0.45;
    let zr = if k == 3 {
        z1 + 0.35
    } else {
        z1 - WALL * 0.5 + 0.05
    };
    pitched(
        b,
        x0,
        x1,
        Vec2::new(zf, roof(zf)),
        Vec2::new(zr, roof(zr)),
        0.12,
        Mat::Corrugated,
    );
    b.deco(
        Vec3::new(x1 - x0, 0.22, 0.04),
        Vec3::new((x0 + x1) * 0.5, roof(zf) + 0.03, zf - 0.045),
        Mat::DarkMetal,
    );
    // A vent stack through the second bay's roof, and the shed's stove
    // pipe through the first's.
    if k == 2 || k == 0 {
        let (x, z, r, h) = if k == 2 {
            (6.0, z0 + 6.0, 0.32, 4.6)
        } else {
            (8.6, z0 + 2.4, 0.09, 1.6)
        };
        let y = roof(z) + 0.06;
        b.piece(
            Shape::Cylinder(r, h),
            Transform::from_xyz(x, y + h * 0.5, z),
            Mat::RustyMetal,
            false,
        );
    }
}

/// The top bay's back wall, where the ore came in: the door off the bank
/// behind, and the headhouse's taller stretch.
fn mill_back(b: &mut Builder) {
    let f = MILL_FLOORS[3];
    let z = MILL_HD - WALL * 0.5;
    let z0 = bay_z0(3);
    let roof = |z: f32| f + EAVE_FRONT + (EAVE_BACK - EAVE_FRONT) * (z - z0) / BAY;
    outer(
        b,
        Vec2::new(MILL_HW, z),
        Vec2::new(-2.0 + WALL * 0.5, z),
        f,
        roof(MILL_HD) + 0.05 - f,
        WALL,
        Mat::Corrugated,
        &[high(2.0), door(6.0), high(10.0)],
    );
    b.wall(
        Vec2::new(-2.0 + WALL * 0.5, z),
        Vec2::new(-MILL_HW, z),
        f,
        HEAD_EAVE - f,
        WALL,
        Mat::Corrugated,
        &[high(2.5), pane(4.0, 1.4, 5.9, 6.9)],
    );
    // The tub track in from the back door, its last tub left on it.
    for x in [3.62, 4.38] {
        b.deco(
            Vec3::new(0.06, 0.05, 4.2),
            Vec3::new(x, f + 0.025, MILL_HD - WALL - 2.1),
            Mat::DarkMetal,
        );
    }
    b.furnish(
        "mine_tub",
        Vec3::new(0.86, 0.95, 1.36),
        Vec3::new(4.0, f + 0.05, 16.6),
        0.0,
        None,
    );
}

/// The headhouse over the top bay's west half: taller walls round the ore
/// bin, a gable roof that tops the mill.
fn headhouse(b: &mut Builder) {
    let f = MILL_FLOORS[3];
    let z0 = bay_z0(3);
    let top = f + EAVE_FRONT + 0.05;
    b.wall(
        Vec2::new(-MILL_HW, z0),
        Vec2::new(-2.0 + WALL * 0.5, z0),
        top,
        HEAD_EAVE - top,
        WALL,
        Mat::Corrugated,
        &[pane(4.0, 1.4, 0.8, 1.8)],
    );
    // Its east wall over the top bay's roof, a band of windows in it.
    b.wall(
        Vec2::new(-2.0, z0 + WALL * 0.5),
        Vec2::new(-2.0, MILL_HD - WALL),
        f + EAVE_FRONT,
        HEAD_EAVE - f - EAVE_FRONT,
        WALL,
        Mat::Corrugated,
        &[pane(2.5, 1.2, 1.75, 2.55), pane(7.0, 1.2, 1.75, 2.55)],
    );
    let (cx, cz) = ((-MILL_HW - 2.0) * 0.5, (z0 + MILL_HD - WALL * 0.5) * 0.5);
    place(b, Transform::from_xyz(cx, 0.0, cz), |s| {
        s.gable_roof_with(
            MILL_HW - 2.0,
            MILL_HD - WALL * 0.5 - z0,
            HEAD_EAVE,
            25.0,
            0.35,
            Mat::Corrugated,
            Mat::Corrugated,
            false,
        );
    });
}

/// The stair from bay `k` up to the next: a solid concrete flight along
/// the bay's back wall (the next bay's footing), its rail along the open
/// side, and a landing at the side wall in front of the doorway up.
fn mill_stair(b: &mut Builder, k: usize) {
    let f = MILL_FLOORS[k];
    let up = MILL_FLOORS[k + 1];
    let (dir, foot, land, (za, zb)) = stair_plan(k);
    let zc = (za + zb) * 0.5;
    b.bx(
        Vec3::new(INNER - land, up - f, STAIR_W),
        Vec3::new(dir * (INNER + land) * 0.5, (f + up) * 0.5, zc),
        Mat::Concrete,
    );
    let steps = ((up - f) / RISE).round() as usize;
    for i in 1..steps {
        let top = f + RISE * i as f32;
        let x0 = foot + (i - 1) as f32 * TREAD;
        b.bx(
            Vec3::new(TREAD, top - f, STAIR_W),
            Vec3::new(dir * (x0 + TREAD * 0.5), (f + top) * 0.5, zc),
            Mat::Concrete,
        );
        // A steel nosing on each tread's edge, a hand proud.
        b.deco(
            Vec3::new(0.05, 0.012, STAIR_W - 0.1),
            Vec3::new(dir * (x0 + 0.025), top + 0.006, zc),
            Mat::DarkMetal,
        );
    }
    // The rail on the open side: posts, a handrail along the nosings a
    // metre over them and a knee rail, round the landing's edge.
    let rz = za + 0.03;
    let nose = |ax: f32| f + RISE + (ax - foot) * (up - f - 2.0 * RISE) / (land - foot - TREAD);
    let (a, c) = (foot + 0.05, land);
    for h in [0.95, 0.5] {
        rod(
            b,
            Vec3::new(dir * a, nose(a) + h, rz),
            Vec3::new(dir * c, nose(c) + h, rz),
            0.045,
            Mat::PaintedMetal,
            true,
        );
        rod(
            b,
            Vec3::new(dir * land, up + h, rz),
            Vec3::new(dir * (INNER - 0.03), up + h, rz),
            0.045,
            Mat::PaintedMetal,
            true,
        );
    }
    for ax in [foot + 0.1, (foot + land) * 0.5, land - 0.1, INNER - 0.06] {
        let i = (((ax - foot) / TREAD).floor() + 1.0).min(steps as f32);
        let y = f + RISE * i;
        let to = if ax >= land {
            up + 0.97
        } else {
            nose(ax) + 0.97
        };
        b.bx(
            Vec3::new(0.05, to - y, 0.05),
            Vec3::new(dir * ax, (y + to) * 0.5, rz),
            Mat::PaintedMetal,
        );
    }
}

/// The front bay: the concentrate shed. The thickener's round tank with
/// its bridge, sacks of concentrate on pallets waiting for lorries that
/// never came, and the foreman's office in the corner by the side door.
fn concentrate_shed(b: &mut Builder) {
    let f = MILL_FLOORS[0];
    // The office: plank partitions, a window onto the shed, a ceiling.
    let (ox, oz) = (4.5, -16.4);
    b.wall(
        Vec2::new(ox - 0.06, oz),
        Vec2::new(INNER, oz),
        f,
        2.6,
        0.12,
        Mat::Planks,
        &[pane(2.6, 1.6, 1.0, 2.0)],
    );
    b.wall(
        Vec2::new(ox, -MILL_HD + WALL),
        Vec2::new(ox, oz - 0.06),
        f,
        2.6,
        0.12,
        Mat::Planks,
        &[door(1.6)],
    );
    b.bx(
        Vec3::new(INNER - ox + 0.06, 0.05, oz + MILL_HD - WALL + 0.06),
        Vec3::new(
            (ox - 0.06 + INNER) * 0.5,
            f + 2.625,
            (-MILL_HD + WALL + oz + 0.06) * 0.5,
        ),
        Mat::Planks,
    );
    b.table(Vec3::new(7.0, f, -19.0), 0.0, Some("driver_cache"));
    b.chair(Vec3::new(7.0, f, -18.25), 0.0);
    b.locker(
        Vec3::new(9.35, f, -17.2),
        -std::f32::consts::FRAC_PI_2,
    );
    b.shelf(Vec3::new(5.2, f, -19.45), 0.0, None);
    b.stove(Vec3::new(8.9, f, -19.25), 0.0);
    b.furnish(
        "tally_board",
        Vec3::new(0.6, 0.8, 0.04),
        Vec3::new(8.6, f + 1.2, -16.4 - 0.08),
        std::f32::consts::PI,
        None,
    );
    // The thickener: a concrete tank, its bridge across with the drive
    // in the middle, the rake's shaft down into the slurry.
    let (tx, tz, tr) = (-2.5, -13.8, 2.2);
    b.piece(
        Shape::Cylinder(tr, 1.5),
        Transform::from_xyz(tx, f + 0.75, tz),
        Mat::Concrete,
        true,
    );
    b.piece(
        Shape::Cylinder(tr - 0.15, 0.02),
        Transform::from_xyz(tx, f + 1.51, tz),
        Mat::Silt,
        false,
    );
    for x in [tx - tr + 0.2, tx + tr - 0.2] {
        b.bx(
            Vec3::new(0.3, 0.3, 0.6),
            Vec3::new(x, f + 1.65, tz),
            Mat::PaintedMetal,
        );
    }
    b.bx(
        Vec3::new(2.0 * tr, 0.3, 0.5),
        Vec3::new(tx, f + 1.95, tz),
        Mat::PaintedMetal,
    );
    b.bx(
        Vec3::new(0.7, 0.55, 0.7),
        Vec3::new(tx, f + 2.375, tz),
        Mat::DarkMetal,
    );
    // Sacks of concentrate, two tiers on each pallet.
    for (x, z) in [(-8.9, -19.0), (-7.5, -19.0), (-8.9, -17.9)] {
        b.pallet(Vec3::new(x, f, z), 0.0);
        for (k, (dx, dz)) in [(-0.3, -0.18), (0.3, -0.18), (-0.3, 0.18), (0.3, 0.18)]
            .into_iter()
            .enumerate()
        {
            for tier in 0..2 {
                if tier == 1 && k == 3 {
                    continue;
                }
                b.bx(
                    Vec3::new(0.56, 0.2, 0.34),
                    Vec3::new(x + dx, f + 0.244 + 0.2 * tier as f32, z + dz),
                    Mat::Canvas,
                );
            }
        }
    }
    b.pallet(Vec3::new(-8.9, f, -14.0), std::f32::consts::FRAC_PI_2);
    b.crate_box(Vec3::new(-8.9, f + 0.144, -14.0), 0.7, Some("workshop"));
    for (x, z) in [(8.9, -12.9), (8.3, -13.2), (8.95, -13.5)] {
        b.barrel(Vec3::new(x, f, z));
    }
}

/// The second bay up: the flotation floor. A bank of cells along the
/// middle with the operators' catwalk along its north side, up a ship's
/// ladder; the conditioner, reagent drums, the sampler's bench.
fn flotation_floor(b: &mut Builder) {
    let f = MILL_FLOORS[1];
    let (x0, x1, z0, z1) = (-7.4, 3.4, -6.6, -5.0);
    let (cx, cz) = ((x0 + x1) * 0.5, (z0 + z1) * 0.5);
    b.bx(
        Vec3::new(x1 - x0, 1.3, z1 - z0),
        Vec3::new(cx, f + 0.65, cz),
        Mat::PaintedMetal,
    );
    // The froth on the cells, the launder it ran over into, the ribs
    // between cells and each cell's impeller drive on top.
    b.deco(
        Vec3::new(x1 - x0 - 0.2, 0.01, z1 - z0 - 0.2),
        Vec3::new(cx, f + 1.305, cz),
        Mat::Silt,
    );
    b.deco(
        Vec3::new(x1 - x0, 0.25, 0.3),
        Vec3::new(cx, f + 1.075, z0 - 0.15),
        Mat::RustyMetal,
    );
    let n = 6;
    let w = (x1 - x0) / n as f32;
    for i in 0..n {
        let x = x0 + w * (i as f32 + 0.5);
        if i > 0 {
            b.deco(
                Vec3::new(0.06, 1.34, z1 - z0 + 0.04),
                Vec3::new(x - w * 0.5, f + 0.67, cz),
                Mat::DarkMetal,
            );
        }
        b.bx(
            Vec3::new(0.4, 0.4, 0.4),
            Vec3::new(x, f + 1.5, cz),
            Mat::DarkMetal,
        );
        b.piece(
            Shape::Cylinder(0.12, 0.25),
            Transform::from_xyz(x, f + 1.825, cz),
            Mat::PaintedMetal,
            false,
        );
    }
    // The catwalk: grating on posts at the cells' tops, a rail on its
    // open side, and a ship's ladder up to it at the west end.
    let deck = f + 1.3;
    let (wz0, wz1) = (z1, z1 + 0.8);
    b.bx(
        Vec3::new(x1 - x0, 0.06, wz1 - wz0),
        Vec3::new(cx, deck - 0.03, (wz0 + wz1) * 0.5),
        Mat::DarkMetal,
    );
    let rz = wz1 - 0.04;
    for x in [x0 + 0.1, x0 + 2.8, cx + 0.4, x1 - 2.6, x1 - 0.1] {
        b.bx(
            Vec3::new(0.06, deck + 1.0 - f, 0.06),
            Vec3::new(x, (f + deck + 1.0) * 0.5, rz),
            Mat::PaintedMetal,
        );
    }
    for h in [0.5, 1.0] {
        b.bx(
            Vec3::new(x1 - x0, 0.045, 0.045),
            Vec3::new(cx, deck + h - 0.02, rz),
            Mat::PaintedMetal,
        );
    }
    let steps = 6;
    let rise = (deck - f) / steps as f32;
    for j in 1..steps {
        let xe = x0 - (steps - j) as f32 * 0.25;
        b.bx(
            Vec3::new(0.25, rise * j as f32, wz1 - wz0),
            Vec3::new(xe + 0.125, f + rise * j as f32 * 0.5, (wz0 + wz1) * 0.5),
            Mat::DarkMetal,
        );
    }
    // The conditioner, its agitator's drive on top.
    b.piece(
        Shape::Cylinder(1.0, 2.0),
        Transform::from_xyz(-6.5, f + 1.0, -8.6),
        Mat::PaintedMetal,
        true,
    );
    b.bx(
        Vec3::new(0.45, 0.4, 0.45),
        Vec3::new(-6.5, f + 2.2, -8.6),
        Mat::DarkMetal,
    );
    // The pump at the bank's tail.
    b.bx(
        Vec3::new(0.8, 0.7, 0.6),
        Vec3::new(4.2, f + 0.35, -5.8),
        Mat::PaintedMetal,
    );
    for (x, z) in [(5.5, -9.3), (6.2, -9.35), (5.85, -8.75)] {
        b.barrel(Vec3::new(x, f, z));
    }
    // The sampler's bench and chair, her shelf of bottles by the west
    // wall.
    b.table(Vec3::new(1.0, f, -9.3), 0.0, Some("tools"));
    b.chair(Vec3::new(1.0, f, -8.55), 0.0);
    b.shelf(
        Vec3::new(-INNER + 0.2, f, -3.0),
        std::f32::consts::FRAC_PI_2,
        Some("medical"),
    );
}

/// The third bay: the ball mill lying across on its piers, its girth gear
/// and drive on the north side; the millwright's bench, a stack of liner
/// plates.
fn ball_mill_floor(b: &mut Builder) {
    let f = MILL_FLOORS[2];
    let (mx, my, mz, r, len) = (-1.5, f + 1.9, 4.2, 1.4, 5.0);
    let across = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    b.piece(
        Shape::Cylinder(r, len),
        Transform::from_xyz(mx, my, mz).with_rotation(across),
        Mat::PaintedMetal,
        true,
    );
    // Bolted liner seams round the drum, and its trunnions on concrete
    // piers.
    for k in 1..4 {
        b.piece(
            Shape::Cylinder(r + 0.02, 0.08),
            Transform::from_xyz(mx - len * 0.5 + len * k as f32 / 4.0, my, mz)
                .with_rotation(across),
            Mat::DarkMetal,
            false,
        );
    }
    for x in [mx - len * 0.5 - 0.3, mx + len * 0.5 + 0.3] {
        b.piece(
            Shape::Cylinder(0.5, 0.6),
            Transform::from_xyz(x, my, mz).with_rotation(across),
            Mat::RustyMetal,
            true,
        );
        b.bx(
            Vec3::new(0.9, my - 0.5 - f, 1.6),
            Vec3::new(x, (f + my - 0.5) * 0.5, mz),
            Mat::Concrete,
        );
    }
    let gx = mx + len * 0.5 - 0.35;
    b.piece(
        Shape::Cylinder(1.75, 0.22),
        Transform::from_xyz(gx, my, mz).with_rotation(across),
        Mat::RustyMetal,
        true,
    );
    b.furnish(
        "gearbox",
        Vec3::new(1.0, 0.75, 0.85),
        Vec3::new(gx, f, mz + 2.2),
        0.0,
        None,
    );
    b.furnish(
        "drive_motor",
        Vec3::new(1.7, 1.1, 0.8),
        Vec3::new(gx - 1.75, f, mz + 2.2),
        std::f32::consts::PI,
        None,
    );
    b.piece(
        Shape::Cylinder(0.25, 0.3),
        Transform::from_xyz(gx, f + 0.95, mz + 1.6)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Mat::DarkMetal,
        false,
    );
    // The feed chute down from the crusher floor onto the feed end.
    let (c0, c1) = (my + 0.45, f + EAVE_FRONT + 0.7);
    b.deco(
        Vec3::new(0.6, c1 - c0, 0.6),
        Vec3::new(mx - len * 0.5 - 0.3, (c0 + c1) * 0.5, mz),
        Mat::RustyMetal,
    );
    b.workbench(Vec3::new(6.0, f, 0.6), 0.0, Some("workshop"));
    b.furnish(
        "tool_chest",
        Vec3::new(0.9, 0.5, 0.5),
        Vec3::new(-INNER + 0.3, f, 6.5),
        std::f32::consts::FRAC_PI_2,
        Some("tools"),
    );
    b.pallet(Vec3::new(-8.6, f, 3.0), std::f32::consts::FRAC_PI_2);
    for k in 0..4 {
        b.bx(
            Vec3::new(0.7, 0.04, 1.1),
            Vec3::new(-8.6, f + 0.164 + 0.04 * k as f32, 3.0),
            Mat::RustyMetal,
        );
    }
    for (x, z) in [(9.1, 7.5), (8.5, 7.6)] {
        b.barrel(Vec3::new(x, f, z));
    }
}

/// The top bay: the ore bin up in the headhouse on its posts, its hopper
/// over the jaw crusher, a ladder up the bin to the grizzly over it; the
/// tub track in from the back door; a bench and the switchgear.
fn crusher_floor(b: &mut Builder) {
    let f = MILL_FLOORS[3];
    let (bx0, bx1, bz0, bz1) = (-9.3, -4.3, 12.5, 18.5);
    let (bcx, bcz) = ((bx0 + bx1) * 0.5, (bz0 + bz1) * 0.5);
    let (bin_lo, bin_hi) = (f + 3.2, f + 5.6);
    b.bx(
        Vec3::new(bx1 - bx0, bin_hi - bin_lo, bz1 - bz0),
        Vec3::new(bcx, (bin_lo + bin_hi) * 0.5, bcz),
        Mat::Planks,
    );
    // Iron bands round the bin, the grizzly bars over its top.
    for y in [bin_lo + 0.5, bin_hi - 0.5] {
        b.deco(
            Vec3::new(bx1 - bx0 + 0.04, 0.12, bz1 - bz0 + 0.04),
            Vec3::new(bcx, y, bcz),
            Mat::RustyMetal,
        );
    }
    for k in 0..7 {
        let x = bx0 + 0.6 + k as f32 * 0.6;
        b.deco(
            Vec3::new(0.08, 0.08, bz1 - bz0 - 0.6),
            Vec3::new(x, bin_hi + 0.04, bcz),
            Mat::DarkMetal,
        );
    }
    // The hopper under it, narrowing to the chute into the crusher.
    b.bx(
        Vec3::new(3.4, 0.6, 4.0),
        Vec3::new(bcx, bin_lo - 0.3, bcz),
        Mat::RustyMetal,
    );
    b.bx(
        Vec3::new(2.0, 0.6, 2.4),
        Vec3::new(bcx, bin_lo - 0.9, bcz),
        Mat::RustyMetal,
    );
    b.bx(
        Vec3::new(0.8, 0.8, 0.8),
        Vec3::new(bcx, bin_lo - 1.6, bcz),
        Mat::RustyMetal,
    );
    for x in [bx0 + 0.15, bx1 - 0.15] {
        for z in [bz0 + 0.15, bz1 - 0.15] {
            b.bx(
                Vec3::new(0.3, bin_lo - f, 0.3),
                Vec3::new(x, (f + bin_lo) * 0.5, z),
                Mat::Timber,
            );
        }
    }
    // The jaw crusher, its two flywheels.
    b.bx(
        Vec3::new(1.4, 1.2, 1.8),
        Vec3::new(bcx, f + 0.6, bcz),
        Mat::PaintedMetal,
    );
    for x in [bcx - 0.9, bcx + 0.9] {
        b.piece(
            Shape::Cylinder(0.75, 0.16),
            Transform::from_xyz(x, f + 0.95, bcz + 0.4)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            Mat::DarkMetal,
            true,
        );
    }
    b.piece(
        Shape::Cylinder(0.08, 2.0),
        Transform::from_xyz(bcx, f + 0.95, bcz + 0.4)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        Mat::DarkMetal,
        false,
    );
    // The ladder up the bin's east face to the grizzly, and what the
    // last shift left up there.
    let lz = 14.0;
    ladder(
        b,
        Vec2::new(bx1 + 0.17, lz),
        Vec3::X,
        f,
        bin_hi + 1.0,
        Vec3::new(bx1 - 0.6, bin_hi + 0.1, lz),
    );
    b.crate_box(
        Vec3::new(bx0 + 0.9, bin_hi + 0.08, 17.4),
        0.6,
        Some("ammo_common"),
    );
    b.workbench(Vec3::new(6.6, f, 19.25), 0.0, Some("tools"));
    b.furnish(
        "switch_cabinet",
        Vec3::new(0.8, 1.4, 0.4),
        Vec3::new(INNER - 0.25, f, 12.6),
        -std::f32::consts::FRAC_PI_2,
        None,
    );
}

// ---- The observatory ----------------------------------------------------------

/// The observatory's floor over its footing, and its storeys' floors.
const OBS_FLOOR: f32 = 0.15;
const OBS_UPPER: f32 = 3.4;
const OBS_ROOF: f32 = 6.5;

/// The weather and radio observatory: a squat two-storey block of rubble
/// stone with walls 60 cm thick, a porch for an airlock, and a flat roof
/// behind a parapet carrying the dome, the instrument cage, the
/// anemometer's mast and the radio mast. Inside: the instrument room
/// with the radio desk, the barometers, a bunk and the stove; a ladder
/// through a hatch to the store upstairs, another through the roof.
fn observatory(b: &mut Builder) {
    let f = OBS_FLOOR;
    let (hx, zf, zb, t) = (5.5, -3.3, 5.0, 0.6);
    let top = OBS_ROOF + 1.0;
    b.bx(
        Vec3::new(2.0 * hx + 0.2, f + 1.0, zb - zf + 0.2),
        Vec3::new(0.0, (f - 1.0) * 0.5, (zf + zb) * 0.5),
        Mat::Concrete,
    );
    let (ix, iz0, iz1) = (hx - t, zf + t, zb - t);
    b.deco(
        Vec3::new(2.0 * ix - 0.02, 0.03, iz1 - iz0 - 0.02),
        Vec3::new(0.0, f + 0.015, (iz0 + iz1) * 0.5),
        Mat::WoodFloor,
    );
    let up = |s: f32| pane(s, 0.8, OBS_UPPER + 0.75 - f, OBS_UPPER + 1.75 - f);
    b.wall(
        Vec2::new(-hx, zf + t * 0.5),
        Vec2::new(hx, zf + t * 0.5),
        f,
        top - f,
        t,
        Mat::Boulder,
        &[window(2.0), door(hx), window(9.0), up(2.0), up(9.0)],
    );
    b.wall(
        Vec2::new(hx, zb - t * 0.5),
        Vec2::new(-hx, zb - t * 0.5),
        f,
        top - f,
        t,
        Mat::Boulder,
        &[window(3.0), window(7.0), up(3.0), up(8.0)],
    );
    b.wall(
        Vec2::new(hx - t * 0.5, iz0),
        Vec2::new(hx - t * 0.5, iz1),
        f,
        top - f,
        t,
        Mat::Boulder,
        &[window(3.2), up(2.0), up(5.0)],
    );
    b.wall(
        Vec2::new(-hx + t * 0.5, iz1),
        Vec2::new(-hx + t * 0.5, iz0),
        f,
        top - f,
        t,
        Mat::Boulder,
        &[window(3.5), up(2.0), up(5.0)],
    );
    // A band of dressed stone at the upper floor, the parapet's coping.
    let band = |b: &mut Builder, y: f32, h: f32, d: f32, m: Mat| {
        for z in [zf - d * 0.5, zb + d * 0.5] {
            b.deco(
                Vec3::new(2.0 * hx + 2.0 * d, h, d),
                Vec3::new(0.0, y, z),
                m,
            );
        }
        for x in [-hx - d * 0.5, hx + d * 0.5] {
            b.deco(Vec3::new(d, h, zb - zf), Vec3::new(x, y, (zf + zb) * 0.5), m);
        }
    };
    band(b, OBS_UPPER - 0.1, 0.25, 0.08, Mat::DressedStone);
    for (z, len) in [(zf + t * 0.5, 2.0 * hx + 0.1), (zb - t * 0.5, 2.0 * hx + 0.1)] {
        b.deco(
            Vec3::new(len, 0.08, t + 0.1),
            Vec3::new(0.0, top + 0.04, z),
            Mat::Concrete,
        );
    }
    for x in [-hx + t * 0.5, hx - t * 0.5] {
        b.deco(
            Vec3::new(t + 0.1, 0.08, iz1 - iz0 - 0.1),
            Vec3::new(x, top + 0.04, (iz0 + iz1) * 0.5),
            Mat::Concrete,
        );
    }
    // The upper floor and the roof, each with its hatch at the back.
    let slab = |b: &mut Builder, y: f32, hole: (f32, f32)| {
        let (h0, h1) = hole;
        let zh = iz1 - 0.8;
        b.bx(
            Vec3::new(2.0 * ix, 0.25, zh - iz0),
            Vec3::new(0.0, y - 0.125, (iz0 + zh) * 0.5),
            Mat::Concrete,
        );
        b.bx(
            Vec3::new(h0 + ix, 0.25, iz1 - zh),
            Vec3::new((h0 - ix) * 0.5, y - 0.125, (zh + iz1) * 0.5),
            Mat::Concrete,
        );
        b.bx(
            Vec3::new(ix - h1, 0.25, iz1 - zh),
            Vec3::new((h1 + ix) * 0.5, y - 0.125, (zh + iz1) * 0.5),
            Mat::Concrete,
        );
    };
    slab(b, OBS_UPPER, (0.65, 1.75));
    slab(b, OBS_ROOF, (-1.15, -0.05));
    let lz = iz1 - 0.07;
    ladder(
        b,
        Vec2::new(1.2, lz),
        Vec3::NEG_Z,
        f,
        OBS_UPPER + 1.0,
        Vec3::new(1.2, OBS_UPPER + 0.05, iz1 - 1.2),
    );
    ladder(
        b,
        Vec2::new(-0.6, lz),
        Vec3::NEG_Z,
        OBS_UPPER,
        OBS_ROOF + 1.0,
        Vec3::new(-0.6, OBS_ROOF + 0.05, iz1 - 1.2),
    );
    // The roof hatch's lid, thrown back on the deck.
    b.bx(
        Vec3::new(1.1, 0.04, 0.8),
        Vec3::new(-0.6, OBS_ROOF + 0.02, iz1 - 1.2),
        Mat::DarkMetal,
    );
    // The porch: the outer door, stone walls, a slab over it.
    let (pw, pz) = (1.4, -5.0);
    b.bx(
        Vec3::new(2.0 * pw, f + 1.0, zf - 0.1 - pz),
        Vec3::new(0.0, (f - 1.0) * 0.5, (zf - 0.1 + pz) * 0.5),
        Mat::Concrete,
    );
    outer(
        b,
        Vec2::new(-pw, pz + 0.15),
        Vec2::new(pw, pz + 0.15),
        f,
        2.5,
        0.3,
        Mat::Boulder,
        &[door(pw)],
    );
    b.wall(
        Vec2::new(pw - 0.15, pz + 0.3),
        Vec2::new(pw - 0.15, zf),
        f,
        2.5,
        0.3,
        Mat::Boulder,
        &[pane(0.7, 0.4, 1.2, 1.7)],
    );
    b.wall(
        Vec2::new(-pw + 0.15, zf),
        Vec2::new(-pw + 0.15, pz + 0.3),
        f,
        2.5,
        0.3,
        Mat::Boulder,
        &[],
    );
    b.bx(
        Vec3::new(2.0 * pw + 0.2, 0.2, zf - pz + 0.25),
        Vec3::new(0.0, f + 2.6, (zf + pz - 0.1) * 0.5 + 0.075),
        Mat::Concrete,
    );
    b.bx(
        Vec3::new(1.6, 0.4, 0.4),
        Vec3::new(0.0, f - 0.32, pz - 0.2),
        Mat::Concrete,
    );
    // The instrument room: the radio desk under the east window (a
    // terminal's place), the barometers' shelf, a double bunk, the stove,
    // a first-aid box.
    b.table(
        Vec3::new(ix - 0.55, f, 0.5),
        std::f32::consts::FRAC_PI_2,
        None,
    );
    b.chair(Vec3::new(ix - 1.35, f, 0.5), -std::f32::consts::FRAC_PI_2);
    b.bx(
        Vec3::new(0.35, 0.45, 0.9),
        Vec3::new(ix - 0.3, f + 0.785 + 0.225, 0.6),
        Mat::PaintedMetal,
    );
    b.deco(
        Vec3::new(0.01, 0.12, 0.5),
        Vec3::new(ix - 0.48, f + 1.1, 0.6),
        Mat::Glass,
    );
    b.shelf(Vec3::new(3.9, f, iz1 - 0.2), 0.0, Some("tools"));
    b.piece(
        Shape::Cylinder(0.16, 0.05),
        Transform::from_xyz(0.2, f + 1.7, iz1 - 0.025)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Mat::PaintedMetal,
        false,
    );
    b.bed(
        Vec3::new(-ix + 1.0, f, iz1 - 1.0),
        std::f32::consts::FRAC_PI_2,
    );
    for (x, z) in [(-ix + 0.05, iz1 - 0.55), (-ix + 1.95, iz1 - 0.55)] {
        for dz in [0.0, -0.9] {
            b.bx(
                Vec3::new(0.06, 1.75, 0.06),
                Vec3::new(x, f + 0.875, z + dz),
                Mat::Timber,
            );
        }
    }
    b.bx(
        Vec3::new(1.95, 0.08, 0.9),
        Vec3::new(-ix + 1.0, f + 1.35, iz1 - 1.0),
        Mat::Timber,
    );
    b.deco(
        Vec3::new(1.85, 0.12, 0.85),
        Vec3::new(-ix + 1.0, f + 1.45, iz1 - 1.0),
        Mat::Canvas,
    );
    b.stove(
        Vec3::new(-ix + 0.5, f, -1.6),
        std::f32::consts::FRAC_PI_2,
    );
    b.crate_box(Vec3::new(-ix + 0.6, f, -0.3), 0.6, Some("medical"));
    // Upstairs: the store, the observer's chest.
    b.table(Vec3::new(3.0, OBS_UPPER, -1.9), 0.0, None);
    b.furnish(
        "tool_chest",
        Vec3::new(0.9, 0.5, 0.5),
        Vec3::new(3.8, OBS_UPPER, iz1 - 0.4),
        0.0,
        Some("driver_cache"),
    );
    b.crate_box(Vec3::new(-ix + 0.6, OBS_UPPER, -1.8), 0.7, None);
    b.bed(Vec3::new(-ix + 0.6, OBS_UPPER, 1.6), 0.0);
    // On the roof: the dome over the east end, white, on its drum.
    let (dx, dz, dr) = (2.6, 0.9, 2.0);
    let drum = OBS_ROOF + 1.2;
    b.piece(
        Shape::Cylinder(dr, 1.2),
        Transform::from_xyz(dx, OBS_ROOF + 0.6, dz),
        Mat::Plaster,
        true,
    );
    b.piece(
        Shape::Cylinder(dr + 0.04, 0.1),
        Transform::from_xyz(dx, drum, dz),
        Mat::DarkMetal,
        false,
    );
    let n = 9;
    for i in 0..n {
        let a0 = i as f32 / n as f32 * std::f32::consts::FRAC_PI_2;
        let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::FRAC_PI_2;
        let (y0, y1) = (dr * a0.sin(), dr * a1.sin());
        let r = dr * ((a0 + a1) * 0.5).cos();
        b.piece(
            Shape::Cylinder(r, y1 - y0 + 0.01),
            Transform::from_xyz(dx, drum + (y0 + y1) * 0.5, dz),
            Mat::Plaster,
            false,
        );
    }
    // Its shutter: a dark slot over the top.
    b.deco(
        Vec3::new(0.45, 0.06, 1.2),
        Vec3::new(dx, drum + dr + 0.02, dz),
        Mat::DarkMetal,
    );
    // The instrument cage: the louvred screen on its legs, a wire cage
    // round it.
    let (sx, sz) = (-0.2, -1.9);
    for (lx, lz) in [(-0.4, -0.25), (0.4, -0.25), (-0.4, 0.25), (0.4, 0.25)] {
        b.bx(
            Vec3::new(0.05, 1.1, 0.05),
            Vec3::new(sx + lx, OBS_ROOF + 0.55, sz + lz),
            Mat::Timber,
        );
    }
    b.bx(
        Vec3::new(0.9, 0.6, 0.6),
        Vec3::new(sx, OBS_ROOF + 1.4, sz),
        Mat::Plaster,
    );
    b.deco(
        Vec3::new(1.0, 0.05, 0.7),
        Vec3::new(sx, OBS_ROOF + 1.725, sz),
        Mat::Plaster,
    );
    for k in 0..4 {
        b.deco(
            Vec3::new(0.86, 0.02, 0.012),
            Vec3::new(sx, OBS_ROOF + 1.2 + 0.12 * k as f32, sz - 0.306),
            Mat::DarkMetal,
        );
    }
    let (cw, cd, ch) = (1.6, 1.3, 2.0);
    for (lx, lz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        b.bx(
            Vec3::new(0.04, ch, 0.04),
            Vec3::new(sx + lx * cw * 0.5, OBS_ROOF + ch * 0.5, sz + lz * cd * 0.5),
            Mat::DarkMetal,
        );
    }
    for y in [OBS_ROOF + 1.0, OBS_ROOF + ch - 0.02] {
        for lz in [-1.0, 1.0] {
            b.deco(
                Vec3::new(cw, 0.025, 0.025),
                Vec3::new(sx, y, sz + lz * cd * 0.5),
                Mat::DarkMetal,
            );
        }
        for lx in [-1.0, 1.0] {
            b.deco(
                Vec3::new(0.025, 0.025, cd - 0.04),
                Vec3::new(sx + lx * cw * 0.5, y, sz),
                Mat::DarkMetal,
            );
        }
    }
    // The anemometer's mast: a pipe, the cups on their arms at the top, a
    // vane under them.
    let (ax, az, ah) = (1.6, 3.6, 12.0);
    b.piece(
        Shape::Cylinder(0.05, ah - OBS_ROOF),
        Transform::from_xyz(ax, (OBS_ROOF + ah) * 0.5, az),
        Mat::PaintedMetal,
        true,
    );
    for k in 0..3 {
        let a = k as f32 * std::f32::consts::TAU / 3.0;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let at = Vec3::new(ax, ah + 0.1, az);
        rod(b, at, at + d * 0.45, 0.02, Mat::DarkMetal, false);
        b.piece(
            Shape::Cylinder(0.07, 0.1),
            Transform::from_translation(at + d * 0.45)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Mat::DarkMetal,
            false,
        );
    }
    b.piece(
        Shape::Cylinder(0.05, 0.2),
        Transform::from_xyz(ax, ah + 0.1, az),
        Mat::DarkMetal,
        false,
    );
    b.deco(
        Vec3::new(0.9, 0.03, 0.03),
        Vec3::new(ax - 0.2, ah - 0.5, az),
        Mat::DarkMetal,
    );
    b.deco(
        Vec3::new(0.02, 0.22, 0.3),
        Vec3::new(ax - 0.62, ah - 0.5, az),
        Mat::DarkMetal,
    );
    // The radio mast: a square lattice from the roof's south-west corner
    // to 20 m, its dipole at the top, guyed to the parapet.
    let (mx, mz, mw, mh) = (-3.6, -1.6, 0.5, 19.3);
    for (lx, lz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        b.bx(
            Vec3::new(0.05, mh - OBS_ROOF, 0.05),
            Vec3::new(mx + lx * mw * 0.5, (OBS_ROOF + mh) * 0.5, mz + lz * mw * 0.5),
            Mat::DarkMetal,
        );
    }
    let mut y = OBS_ROOF + 0.9;
    let mut k = 0;
    while y < mh - 0.2 {
        for lz in [-1.0, 1.0] {
            b.deco(
                Vec3::new(mw, 0.03, 0.03),
                Vec3::new(mx, y, mz + lz * mw * 0.5),
                Mat::DarkMetal,
            );
        }
        for lx in [-1.0, 1.0] {
            b.deco(
                Vec3::new(0.03, 0.03, mw - 0.05),
                Vec3::new(mx + lx * mw * 0.5, y, mz),
                Mat::DarkMetal,
            );
        }
        // A diagonal across each face, alternating.
        let s = if k % 2 == 0 { 1.0 } else { -1.0 };
        for lz in [-1.0, 1.0] {
            rod(
                b,
                Vec3::new(mx - s * mw * 0.5, y, mz + lz * mw * 0.5),
                Vec3::new(mx + s * mw * 0.5, y + 1.0, mz + lz * mw * 0.5),
                0.025,
                Mat::DarkMetal,
                false,
            );
        }
        y += 1.0;
        k += 1;
    }
    b.piece(
        Shape::Cylinder(0.03, 0.8),
        Transform::from_xyz(mx, mh + 0.4, mz),
        Mat::DarkMetal,
        false,
    );
    b.deco(
        Vec3::new(2.6, 0.04, 0.04),
        Vec3::new(mx, mh - 0.1, mz),
        Mat::DarkMetal,
    );
    let guy = Vec3::new(mx, 14.0, mz);
    for (gx, gz) in [(hx - 0.3, zf + 0.3), (-hx + 0.3, zb - 0.3), (hx - 0.3, zb - 0.3)] {
        rod(
            b,
            guy,
            Vec3::new(gx, top + 0.05, gz),
            0.015,
            Mat::DarkMetal,
            false,
        );
    }
    b.crate_box(
        Vec3::new(ix - 0.5, OBS_ROOF, iz1 - 0.5),
        0.6,
        Some("ammo_common"),
    );
}

// ---- The funicular ----------------------------------------------------------

/// The funicular's grade: rise over run. The line climbs toward the
/// stations' and the car's -Z.
pub const GRADE: f32 = 0.20;

/// A station's platform along its -X face, from `x0` to `x1`: in steps
/// each `seg` long, climbing toward -Z by `rise` a step from `y0` at the
/// +Z end; under a lean-to roof off the wall at `x1`, from `wall_y` down
/// to `edge_y` over the edge.
#[allow(clippy::too_many_arguments)]
fn platform(
    b: &mut Builder,
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
    y0: f32,
    rise: f32,
    seg: f32,
    wall_y: f32,
    edge_y: f32,
) {
    let n = ((z1 - z0) / seg).round() as usize;
    let seg = (z1 - z0) / n as f32;
    let top = |i: usize| y0 + rise * i as f32;
    for i in 0..n {
        let (za, zb) = (z1 - seg * (i + 1) as f32, z1 - seg * i as f32);
        let y = top(i);
        b.bx(
            Vec3::new(x1 - x0, y + 0.6, zb - za),
            Vec3::new((x0 + x1) * 0.5, (y - 0.6) * 0.5, (za + zb) * 0.5),
            Mat::Concrete,
        );
        // The white line along the edge.
        b.deco(
            Vec3::new(0.12, 0.01, zb - za),
            Vec3::new(x0 + 0.1, y + 0.005, (za + zb) * 0.5),
            Mat::Plaster,
        );
    }
    // The lean-to: its roof's underside runs from `edge_y` a little past
    // the edge to `wall_y` at the wall (and into it); a beam under it on
    // posts standing on the steps.
    let (ra, rc) = (Vec2::new(x0 - 0.2, edge_y), Vec2::new(x1 + 0.05, wall_y));
    let under = |x: f32| ra.y + (rc.y - ra.y) * (x - ra.x) / (rc.x - ra.x);
    let px = x0 + 0.25;
    let beam = under(px);
    let posts = ((z1 - z0) / 4.0).round() as usize;
    for k in 0..=posts {
        let z = z1 - 0.3 - (z1 - z0 - 0.6) * k as f32 / posts as f32;
        let i = (((z1 - z) / seg).floor() as usize).min(n - 1);
        let y = top(i);
        b.bx(
            Vec3::new(0.14, beam - 0.2 - y, 0.14),
            Vec3::new(px, (y + beam - 0.2) * 0.5, z),
            Mat::Timber,
        );
    }
    b.bx(
        Vec3::new(0.16, 0.2, z1 - z0),
        Vec3::new(px, beam - 0.1, (z0 + z1) * 0.5),
        Mat::Timber,
    );
    let d = rc - ra;
    let ang = d.y.atan2(d.x);
    let c = (ra + rc) * 0.5 + Vec2::new(-ang.sin(), ang.cos()) * 0.04;
    b.piece(
        Shape::Box(Vec3::new(d.length(), 0.08, z1 - z0 + 0.3)),
        Transform::from_xyz(c.x, c.y, (z0 + z1) * 0.5).with_rotation(Quat::from_rotation_z(ang)),
        Mat::Corrugated,
        true,
    );
}

/// A board on a wall: `size` (x across, y up, the board's thickness),
/// its back on the plane at `at` facing `out` (±X or ±Z), with ruled
/// lines on it (a timetable).
fn board(b: &mut Builder, at: Vec3, out: Vec3, w: f32, h: f32) {
    let across = Vec3::new(out.z.abs(), 0.0, out.x.abs());
    let size = across * w + Vec3::Y * h + out.abs() * 0.03;
    b.deco(size, at + out * 0.015, Mat::Planks);
    for k in 0..5 {
        let y = at.y + h * 0.5 - 0.15 - k as f32 * (h - 0.3) / 4.0;
        let s = across * (w - 0.2) + Vec3::Y * 0.025 + out.abs() * 0.01;
        b.deco(
            s,
            Vec3::new(at.x, y, at.z) + out * 0.034,
            if k == 0 { Mat::DarkMetal } else { Mat::Plaster },
        );
    }
}

/// The valley station: a gabled timber hall on a rubble-stone base, its
/// long axis along the line, beside the track (which runs past its -X
/// face, outside). The platform steps up along that face under a
/// lean-to; a door onto it from the waiting hall, another at the +Z end
/// off the lane. Inside, the ticket office behind its window and the
/// waiting hall with its benches and the timetable.
fn lower_station(b: &mut Builder) {
    let f = 0.15;
    let (w, d, h) = (6.6, 15.6, 4.0);
    let cx = 1.6;
    place(b, Transform::from_xyz(cx, 0.0, 0.0), |s| {
        s.plinth(w, d, f, Mat::Boulder, Mat::WoodFloor);
        s.shell(
            w,
            d,
            f,
            h,
            0.2,
            Mat::Planks,
            [
                &[wide_window(1.8, 1.4), wide_window(4.8, 1.4)],
                &[window(2.0), window(5.5), window(9.0), window(12.5)],
                &[window(1.3), door(3.3), window(5.3)],
                &[
                    door(1.0),
                    wide_window(3.5, 1.6),
                    wide_window(7.5, 1.6),
                    window(12.5),
                ],
            ],
        );
        s.gable_roof(w + 0.36, d + 0.36, f + h, 35.0, 0.45, Mat::Shingles, Mat::Planks);
        // The ticket office's partition, its window onto the hall and its
        // door; the counter behind the window, the clerk's desk and the
        // lost-property shelf.
        let pz = -3.0;
        s.wall(
            Vec2::new(-w * 0.5, pz),
            Vec2::new(w * 0.5, pz),
            f,
            h,
            0.12,
            Mat::Planks,
            &[pane(2.2, 0.8, 0.95, 1.6), door(5.2)],
        );
        s.counter(Vec3::new(-1.1, f, pz - 0.45), 0.0, 1.6, None);
        s.table(Vec3::new(1.5, f, -7.1), 0.0, Some("tools"));
        s.chair(Vec3::new(1.5, f, -6.4), 0.0);
        s.shelf(
            Vec3::new(-w * 0.5 + 0.35, f, -5.5),
            std::f32::consts::FRAC_PI_2,
            Some("medical"),
        );
        // The waiting hall: benches along both walls, the stove, the
        // timetable by the ticket window.
        let half = std::f32::consts::FRAC_PI_2;
        s.pew(Vec3::new(w * 0.5 - 0.3, f, 2.95), -half);
        s.pew(Vec3::new(w * 0.5 - 0.3, f, 6.45), -half);
        s.pew(Vec3::new(-w * 0.5 + 0.3, f, 2.3), half);
        s.stove(Vec3::new(-w * 0.5 + 0.6, f, 5.6), half);
        board(s, Vec3::new(0.35, f + 1.85, pz + 0.06), Vec3::Z, 1.0, 0.9);
        // A chimney up from the office through the roof.
        let ridge = f + h + (w + 0.36) * 0.5 * 35f32.to_radians().tan();
        s.bx(
            Vec3::new(0.6, ridge + 0.8 - (f + h + 0.04), 0.6),
            Vec3::new(1.4, (ridge + 0.8 + f + h + 0.04) * 0.5, -5.6),
            Mat::Boulder,
        );
    });
    // The platform along the -X face, stepping up the grade toward -Z,
    // with a bench and the timetable on the wall.
    let (x0, x1) = (-5.0, cx - w * 0.5 - 0.1);
    platform(b, x0, x1, -8.0, 8.0, f, 0.15, 2.0, f + h - 0.25, f + h - 0.65);
    board(b, Vec3::new(x1, f + 1.7, 2.0), Vec3::NEG_X, 1.2, 0.9);
    slat_bench_x(b, x1, f + 0.3, 3.0, 1.6);
}

/// A slatted bench against a wall at `wall` (x) facing -X, at `z`.
fn slat_bench_x(b: &mut Builder, wall: f32, y: f32, z: f32, len: f32) {
    for k in 0..3 {
        b.bx(
            Vec3::new(0.11, 0.03, len),
            Vec3::new(wall - 0.1 - 0.13 * k as f32 - 0.065, y + 0.435, z),
            Mat::Timber,
        );
    }
    for k in 0..2 {
        b.deco(
            Vec3::new(0.025, 0.1, len),
            Vec3::new(wall - 0.0425, y + 0.65 + 0.2 * k as f32, z),
            Mat::Timber,
        );
    }
    for sz in [-1.0, 1.0] {
        let fz = z + sz * (len * 0.5 - 0.12);
        b.bx(
            Vec3::new(0.4, 0.42, 0.04),
            Vec3::new(wall - 0.3, y + 0.21, fz),
            Mat::DarkMetal,
        );
        b.deco(
            Vec3::new(0.03, 0.5, 0.04),
            Vec3::new(wall - 0.015, y + 0.75, fz),
            Mat::DarkMetal,
        );
    }
}

/// The top station and winding house: a concrete block on a stone base,
/// the bull wheel standing in its tall housing at the -Z end behind
/// windows two storeys high, the housing's roof the station's highest
/// point. The same stepped platform as the valley station's along the
/// -X face; doors onto it and at the +Z end. Inside the motor room: the
/// drive, the brake, the control desk at the window over the line, a
/// spare rope on its reel, oil drums.
fn upper_station(b: &mut Builder) {
    let f = 0.15;
    let (x0, x1, z0, z1, t) = (-3.2, 6.0, -7.0, 7.0, 0.3);
    let (hx, hz) = (1.8, -1.0);
    let (eave, top) = (5.8, 11.0);
    // The footing and floor.
    b.bx(
        Vec3::new(x1 - x0, f + 1.0, z1 - z0),
        Vec3::new((x0 + x1) * 0.5, (f - 1.0) * 0.5, 0.0),
        Mat::Boulder,
    );
    let (ix0, ix1, iz0, iz1) = (x0 + t, x1 - t, z0 + t, z1 - t);
    let tall = |s: f32, w: f32| pane(s, w, 1.8, 8.6);
    // The front: the housing's face with its tall windows, the motor
    // room's.
    b.wall(
        Vec2::new(x0, z0 + t * 0.5),
        Vec2::new(hx, z0 + t * 0.5),
        f,
        top - f,
        t,
        Mat::Concrete,
        &[tall(1.4, 1.2), tall(3.6, 1.2)],
    );
    b.wall(
        Vec2::new(hx, z0 + t * 0.5),
        Vec2::new(x1, z0 + t * 0.5),
        f,
        eave - f,
        t,
        Mat::Concrete,
        &[window(2.1)],
    );
    // The -X side over the platform: the housing's tall window, then the
    // window over the control desk and the door at the +Z end.
    outer(
        b,
        Vec2::new(x0 + t * 0.5, iz1),
        Vec2::new(x0 + t * 0.5, hz),
        f,
        eave - f,
        t,
        Mat::Concrete,
        &[door(1.0), wide_window(5.5, 1.6)],
    );
    b.wall(
        Vec2::new(x0 + t * 0.5, hz),
        Vec2::new(x0 + t * 0.5, iz0),
        f,
        top - f,
        t,
        Mat::Concrete,
        &[tall(hz - iz0 - 2.85, 1.6)],
    );
    b.wall(
        Vec2::new(x1 - t * 0.5, iz0),
        Vec2::new(x1 - t * 0.5, iz1),
        f,
        eave - f,
        t,
        Mat::Concrete,
        &[window(3.0), window(7.5), window(11.0)],
    );
    outer(
        b,
        Vec2::new(x1, z1 - t * 0.5),
        Vec2::new(x0, z1 - t * 0.5),
        f,
        eave - f,
        t,
        Mat::Concrete,
        &[window(1.5), door(4.0), window(6.8)],
    );
    // The housing's inner walls over the motor room on their beams.
    for (a, c, beam) in [
        (Vec2::new(hx - t * 0.5, iz0), Vec2::new(hx - t * 0.5, hz - t), true),
        (Vec2::new(ix0, hz - t * 0.5), Vec2::new(hx, hz - t * 0.5), true),
    ] {
        b.wall(a, c, eave, top - eave, t, Mat::Concrete, &[]);
        if beam {
            let len = a.distance(c);
            let m = (a + c) * 0.5;
            let size = if (a.x - c.x).abs() < 0.01 {
                Vec3::new(t, 0.45, len)
            } else {
                Vec3::new(len, 0.45, t)
            };
            b.bx(size, Vec3::new(m.x, eave - 0.225, m.y), Mat::Concrete);
        }
    }
    // The roof over the motor room (round the housing), its coping; the
    // housing's own gable roof.
    b.bx(
        Vec3::new(x1 - x0, 0.25, z1 - hz),
        Vec3::new((x0 + x1) * 0.5, eave + 0.125, (hz + z1) * 0.5),
        Mat::Concrete,
    );
    b.bx(
        Vec3::new(x1 - hx, 0.25, hz - z0),
        Vec3::new((hx + x1) * 0.5, eave + 0.125, (z0 + hz) * 0.5),
        Mat::Concrete,
    );
    b.deco(
        Vec3::new(x1 - x0 + 0.1, 0.12, 0.35),
        Vec3::new((x0 + x1) * 0.5, eave + 0.31, z1 - 0.1),
        Mat::DarkMetal,
    );
    b.deco(
        Vec3::new(0.35, 0.12, z1 - z0 + 0.1),
        Vec3::new(x1 - 0.1, eave + 0.31, 0.0),
        Mat::DarkMetal,
    );
    let (gx, gz) = ((x0 + hx) * 0.5, (z0 + hz) * 0.5);
    place(b, Transform::from_xyz(gx, 0.0, gz), |s| {
        s.gable_roof_with(
            hx - x0 - t,
            hz - z0 - t,
            top,
            22.0,
            0.3,
            Mat::Corrugated,
            Mat::Concrete,
            false,
        );
    });
    // The bull wheel: its rim in segments, six spokes, the hub on its
    // shaft between two concrete pedestals; the shaft's chain case down
    // to the gearbox and the motor.
    let (wx, wy, wz, wr) = (gx, 5.0, gz, 2.4);
    let across = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    let n = 20;
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        let seg = std::f32::consts::TAU * wr / n as f32 * 1.04;
        b.piece(
            Shape::Box(Vec3::new(0.24, 0.22, seg)),
            Transform::from_xyz(wx, wy + wr * a.sin(), wz + wr * a.cos())
                .with_rotation(Quat::from_rotation_x(-a - std::f32::consts::FRAC_PI_2)),
            Mat::RustyMetal,
            true,
        );
    }
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU;
        let d = Vec3::new(0.0, a.sin(), a.cos());
        rod(
            b,
            Vec3::new(wx, wy, wz) + d * 0.3,
            Vec3::new(wx, wy, wz) + d * (wr - 0.1),
            0.12,
            Mat::PaintedMetal,
            false,
        );
    }
    b.piece(
        Shape::Cylinder(0.4, 0.5),
        Transform::from_xyz(wx, wy, wz).with_rotation(across),
        Mat::DarkMetal,
        true,
    );
    let (sa, sb) = (ix0 + 0.6, hx - t + 0.1);
    b.piece(
        Shape::Cylinder(0.12, sb - sa),
        Transform::from_xyz((sa + sb) * 0.5, wy, wz).with_rotation(across),
        Mat::DarkMetal,
        false,
    );
    for x in [wx - 0.9, wx + 0.9] {
        b.bx(
            Vec3::new(0.7, wy - 0.12 - f, 1.1),
            Vec3::new(x, (f + wy - 0.12) * 0.5, wz),
            Mat::Concrete,
        );
    }
    let cx = hx - t - 0.25;
    b.bx(
        Vec3::new(0.3, wy + 0.3 - (f + 0.8), 0.6),
        Vec3::new(cx, (wy + 0.3 + f + 0.8) * 0.5, wz),
        Mat::PaintedMetal,
    );
    b.furnish(
        "gearbox",
        Vec3::new(1.0, 0.75, 0.85),
        Vec3::new(cx + 0.55, f, wz),
        std::f32::consts::FRAC_PI_2,
        None,
    );
    b.furnish(
        "drive_motor",
        Vec3::new(1.7, 1.1, 0.8),
        Vec3::new(cx + 2.1, f, wz),
        0.0,
        None,
    );
    b.furnish(
        "brake_drum",
        Vec3::new(1.8, 1.4, 1.0),
        Vec3::new(4.4, f, -1.9),
        0.0,
        None,
    );
    // The rope going out: down from the wheel's foot and out under the
    // tall window to the line.
    rod(
        b,
        Vec3::new(wx, wy - wr - 0.05, wz - 0.3),
        Vec3::new(x0 - 0.3, 0.3, wz - 0.3),
        0.05,
        Mat::DarkMetal,
        false,
    );
    // The control desk at the window over the line; its stand.
    let half = std::f32::consts::FRAC_PI_2;
    b.table(Vec3::new(ix0 + 0.4, f, 2.9), half, None);
    b.furnish(
        "control_stand",
        Vec3::new(0.9, 1.4, 0.5),
        Vec3::new(ix0 + 0.3, f, 1.2),
        half,
        None,
    );
    b.chair(Vec3::new(ix0 + 1.2, f, 2.9), half);
    b.furnish(
        "switch_cabinet",
        Vec3::new(0.8, 1.4, 0.4),
        Vec3::new(ix1 - 0.2, f, 0.5),
        -half,
        None,
    );
    // The spare rope on its reel, the oil drums, the fitters' chest.
    let (rx, rz) = (3.6, 4.6);
    for dx in [-0.5, 0.5] {
        b.piece(
            Shape::Cylinder(0.9, 0.08),
            Transform::from_xyz(rx + dx, f + 0.9, rz).with_rotation(across),
            Mat::Timber,
            true,
        );
    }
    b.piece(
        Shape::Cylinder(0.7, 0.92),
        Transform::from_xyz(rx, f + 0.9, rz).with_rotation(across),
        Mat::DarkMetal,
        true,
    );
    for (x, z) in [(5.25, 6.25), (4.65, 6.3), (5.25, 5.65)] {
        b.barrel(Vec3::new(x, f, z));
    }
    b.furnish(
        "tool_chest",
        Vec3::new(0.9, 0.5, 0.5),
        Vec3::new(ix1 - 0.3, f, 3.2),
        half,
        Some("tools"),
    );
    b.crate_box(Vec3::new(ix1 - 0.45, f, iz0 + 0.45), 0.7, Some("workshop"));
    b.crate_box(Vec3::new(-0.8, f, iz1 - 0.65), 0.6, Some("medical"));
    b.crate_box(Vec3::new(ix0 + 0.4, f, iz0 + 0.4), 0.6, Some("ammo_common"));
    // The platform along the -X face.
    platform(b, -6.0, x0, z0, z1, f, 0.15, 2.0, eave - 0.4, eave - 0.8);
    board(b, Vec3::new(x0, f + 1.7, 2.75), Vec3::NEG_X, 1.2, 0.9);
    slat_bench_x(b, x0, f + 0.15, 4.0, 1.6);
}

/// The car's compartments: how many, and the body's length (z).
const CAR_ROOMS: usize = 3;
const CAR_LEN: f32 = 7.64;
/// The body's half width and its walls' height over each floor.
const CAR_HW: f32 = 1.3;
const CAR_WALL: f32 = 2.15;
/// The underframe's deck, over the rail heads (along the grade's normal).
const CAR_DECK: f32 = 0.9;

/// A pose on the grade: `y` over the plane (along its normal) at world
/// `z`, turned to lie along it.
fn on_grade(x: f32, y: f32, z: f32) -> Transform {
    let a = GRADE.atan();
    let (s, c) = a.sin_cos();
    let zp = z - y * s;
    Transform::from_xyz(x, -GRADE * zp + y * c, z).with_rotation(Quat::from_rotation_x(a))
}

/// Compartment `i`'s z extent (0 at the -Z end, the top of the car) and
/// floor height: level, a step down from the one before it.
fn car_room(i: usize) -> (f32, f32, f32) {
    let l = CAR_LEN / CAR_ROOMS as f32;
    let za = -CAR_LEN * 0.5 + l * i as f32;
    let deck = CAR_DECK / GRADE.atan().cos();
    (za, za + l, deck - GRADE * za)
}

/// A funicular car stopped on the line: an underframe pitched to the
/// grade on its four wheels, the body in three compartments stepped down
/// it so each floor is level, slatted benches facing across each, a door
/// on the +X side of each (one folded back, one gone, one hanging open),
/// most of the glass out. Its local y = 0 is the ground under its middle;
/// the ground falls toward +Z at [`GRADE`], and the wheels stand on rails
/// whose heads are 2 cm over it.
fn car(b: &mut Builder) {
    // The rails, and the underframe: two sole bars, the deck, headstocks
    // at each end, the wheels on their axles.
    for x in [-0.6, 0.6] {
        b.piece(
            Shape::Box(Vec3::new(0.07, 0.12, 8.0)),
            on_grade(x, -0.04, 0.0),
            Mat::RustyMetal,
            false,
        );
    }
    for x in [-0.8, 0.8] {
        b.piece(
            Shape::Box(Vec3::new(0.12, 0.3, CAR_LEN - 0.1)),
            on_grade(x, 0.65, 0.0),
            Mat::DarkMetal,
            true,
        );
    }
    b.piece(
        Shape::Box(Vec3::new(2.0 * CAR_HW, 0.1, CAR_LEN)),
        on_grade(0.0, CAR_DECK - 0.05, 0.0),
        Mat::DarkMetal,
        true,
    );
    for z in [-CAR_LEN * 0.5 + 0.08, CAR_LEN * 0.5 - 0.08] {
        b.piece(
            Shape::Box(Vec3::new(2.0 * CAR_HW, 0.3, 0.16)),
            on_grade(0.0, 0.65, z),
            Mat::PaintedMetal,
            true,
        );
    }
    let across = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    for z in [-2.6, 2.6] {
        for x in [-0.6, 0.6] {
            let mut t = on_grade(x, 0.32, z);
            t.rotation *= across;
            b.piece(Shape::Cylinder(0.3, 0.1), t, Mat::RustyMetal, true);
            b.piece(
                Shape::Box(Vec3::new(0.16, 0.24, 0.32)),
                on_grade(x.signum() * 0.8, 0.42, z),
                Mat::DarkMetal,
                false,
            );
        }
        let mut t = on_grade(0.0, 0.32, z);
        t.rotation *= across;
        b.piece(Shape::Cylinder(0.05, 1.5), t, Mat::DarkMetal, false);
    }
    // The haul rope's socket at the top end.
    b.piece(
        Shape::Box(Vec3::new(0.3, 0.25, 0.3)),
        on_grade(0.0, 0.55, -CAR_LEN * 0.5 - 0.05),
        Mat::DarkMetal,
        false,
    );
    // The skirt under the body each side, from the deck to above the
    // floors, broken at the doors.
    let l = CAR_LEN / CAR_ROOMS as f32;
    let door_w = 0.7;
    let cos = GRADE.atan().cos();
    for x in [-CAR_HW + 0.03, CAR_HW - 0.03] {
        let mut runs = Vec::new();
        let mut z = -CAR_LEN * 0.5;
        if x > 0.0 {
            for i in 0..CAR_ROOMS {
                let (za, _, _) = car_room(i);
                let m = za + l * 0.5;
                runs.push((z, m - door_w * 0.5));
                z = m + door_w * 0.5;
            }
        }
        runs.push((z, CAR_LEN * 0.5));
        for (a, c) in runs {
            b.piece(
                Shape::Box(Vec3::new(0.04, 0.6, (c - a) / cos)),
                on_grade(x, CAR_DECK + 0.3, (a + c) * 0.5),
                Mat::PaintedMetal,
                false,
            );
        }
    }
    // The compartments: floor, walls with their smashed windows and the
    // door gaps, a roof each, stepped down the car.
    b.ruin = Some(Ruin::Burnt);
    let t = 0.06;
    for i in 0..CAR_ROOMS {
        let (za, zb, y) = car_room(i);
        let zs = if i == 0 { za + t } else { za + t * 0.5 };
        let ze = if i == CAR_ROOMS - 1 {
            zb - t
        } else {
            zb - t * 0.5
        };
        let span = ze - zs;
        let ix = CAR_HW - t;
        b.bx(
            Vec3::new(2.0 * ix, 0.1, span),
            Vec3::new(0.0, y - 0.05, (zs + ze) * 0.5),
            Mat::Planks,
        );
        let mid = (za + zb) * 0.5 - zs;
        b.wall(
            Vec2::new(CAR_HW - t * 0.5, zs),
            Vec2::new(CAR_HW - t * 0.5, ze),
            y,
            CAR_WALL,
            t,
            Mat::PaintedMetal,
            &[
                pane(0.42, 0.55, 0.85, 1.75),
                gap(mid, door_w, 1.95),
                pane(span - 0.42, 0.55, 0.85, 1.75),
            ],
        );
        b.wall(
            Vec2::new(-CAR_HW + t * 0.5, ze),
            Vec2::new(-CAR_HW + t * 0.5, zs),
            y,
            CAR_WALL,
            t,
            Mat::PaintedMetal,
            &[
                pane(0.62, 0.85, 0.85, 1.75),
                pane(span - 0.62, 0.85, 0.85, 1.75),
            ],
        );
        // The wall at the compartment's -Z end: the car's end, or the
        // partition up from the compartment below to this one's roof.
        let end = &[pane(CAR_HW, 1.5, 0.9, 1.8)];
        if i == 0 {
            b.wall(
                Vec2::new(-CAR_HW, za + t * 0.5),
                Vec2::new(CAR_HW, za + t * 0.5),
                y,
                CAR_WALL,
                t,
                Mat::PaintedMetal,
                end,
            );
        }
        if i + 1 < CAR_ROOMS {
            let (_, _, below) = car_room(i + 1);
            b.wall(
                Vec2::new(-CAR_HW, zb),
                Vec2::new(CAR_HW, zb),
                below,
                y + CAR_WALL - below,
                t,
                Mat::PaintedMetal,
                &[],
            );
        } else {
            b.wall(
                Vec2::new(-CAR_HW, zb - t * 0.5),
                Vec2::new(CAR_HW, zb - t * 0.5),
                y,
                CAR_WALL,
                t,
                Mat::PaintedMetal,
                end,
            );
        }
        // The roof: over the partition behind it, out past the car's
        // ends.
        let ra = if i == 0 { za - 0.06 } else { za + t * 0.5 };
        let rb = if i + 1 == CAR_ROOMS { zb + 0.06 } else { zb + t * 0.5 };
        b.bx(
            Vec3::new(2.0 * CAR_HW + 0.1, 0.08, rb - ra),
            Vec3::new(0.0, y + CAR_WALL + 0.04, (ra + rb) * 0.5),
            Mat::RustyMetal,
        );
        // Benches facing each other across the door.
        slat_bench(b, 0.0, y, zs, 1.0, 2.0 * ix - 0.04);
        slat_bench(b, 0.0, y, ze, -1.0, 2.0 * ix - 0.04);
    }
    b.ruin = None;
    // The doors: the top one folded right back against the side, the
    // middle one gone, the bottom one hanging open off its top hinge.
    let (za, _, y) = car_room(0);
    let d0 = za + l * 0.5 - door_w * 0.5;
    b.deco(
        Vec3::new(0.04, 1.9, door_w - 0.04),
        Vec3::new(CAR_HW + 0.025, y + 0.98, d0 - door_w * 0.5 - 0.02),
        Mat::PaintedMetal,
    );
    let (za, _, y) = car_room(2);
    let hinge = Vec3::new(CAR_HW + 0.02, y + 0.03, za + l * 0.5 + door_w * 0.5);
    let swing = Quat::from_rotation_y(-0.45) * Quat::from_rotation_x(0.08);
    b.piece(
        Shape::Box(Vec3::new(0.04, 1.9, door_w - 0.04)),
        Transform::from_translation(hinge + swing * Vec3::new(0.0, 0.95, -(door_w - 0.04) * 0.5))
            .with_rotation(swing),
        Mat::PaintedMetal,
        false,
    );
    // What's left: a bag on the middle bench, a first-aid tin under the
    // bottom one.
    let (_, _, y1) = car_room(1);
    let (zs1, _, _) = car_room(1);
    b.loot(Vec3::new(0.75, y1 + 0.5, zs1 + 0.35), "tools");
    let (_, zb2, y2) = car_room(2);
    b.loot(Vec3::new(0.75, y2 + 0.08, zb2 - 0.4), "medical");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every piece is real (as `buildings::every_piece_is_sound`), and each
    /// prefab stays inside the footprint the map lint and nav grid use
    /// (roofs' eaves and a door's steps a little past it).
    #[test]
    fn fell_prefabs_are_sound_and_fit_their_footprints() {
        for id in PREFABS {
            let g = geo(id).unwrap_or_else(|| panic!("{id} builds"));
            let [w, h, d] = extraction_core::map::prefab_footprint(id);
            for (i, p) in g.pieces.iter().enumerate() {
                let ok = match p.shape {
                    Shape::Box(s) | Shape::Prism(s) => s.is_finite() && s.min_element() > 0.0,
                    Shape::Cylinder(r, h) => r > 0.0 && h > 0.0,
                } && p.at.translation.is_finite()
                    && p.at.rotation.is_finite();
                assert!(ok, "{id} piece {i}: {:?} at {:?}", p.shape, p.at);
                let at = p.at.translation;
                assert!(
                    at.x.abs() <= w * 0.5 + 0.6 && at.z.abs() <= d * 0.5 + 0.6,
                    "{id} piece {i} at {at:?} is outside its {w} x {d} footprint"
                );
                assert!(
                    at.y < h + 0.6,
                    "{id} piece {i} at {at:?} is over its {h} m height"
                );
            }
        }
    }

    /// Every model a Rime prefab is dressed with is built.
    #[test]
    fn fell_models_exist() {
        let root = crate::asset_root_path();
        for id in PREFABS {
            for d in geo(id).unwrap().props {
                for (name, _) in d.models {
                    let p = root.join(crate::render::world_props::path(name));
                    assert!(p.exists(), "{id}: no model {}", p.display());
                }
            }
        }
    }

    /// The map's copy of each prefab's doors is the recipes' own (as
    /// `buildings::entrances_match_the_map`).
    #[test]
    fn fell_entrances_match_the_map() {
        for id in PREFABS {
            let built = geo(id).unwrap().entrances;
            let listed = extraction_core::map::prefab_entrances(id);
            assert_eq!(
                built.len(),
                listed.len(),
                "{id}: doors built {built:?} vs listed"
            );
            for (b, l) in built.iter().zip(listed) {
                let close = |a: f32, b: f32| (a - b).abs() < 0.011;
                assert!(
                    close(b.at[0], l.at[0])
                        && close(b.at[1], l.at[1])
                        && close(b.out[0], l.out[0])
                        && close(b.out[1], l.out[1])
                        && close(b.width, l.width)
                        && close(b.clear, l.clear),
                    "{id}: built {b:?}, listed {l:?}"
                );
            }
        }
    }

    /// No two faces of different stuff fight in one plane, and nothing
    /// hangs in the air (`qa`'s gauges, which walk the main list only).
    #[test]
    fn fell_prefabs_dont_fight_or_float() {
        let mut bad = Vec::new();
        for id in PREFABS {
            let g = geo(id).unwrap();
            for f in crate::qa::fighting_faces_above(&g, -0.05) {
                bad.push(format!(
                    "{id}: pieces {} and {} fight ({:?}, {:.3} m2) at {:.2?}",
                    f.a, f.b, f.mats, f.area, f.at
                ));
            }
            for (i, at) in crate::qa::floating(&g) {
                bad.push(format!("{id}: piece {i} floats at {at:.2?}"));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Each of the mill's stairs climbs a storey in steps a body takes,
    /// and the plan's floor agrees with what's built: the step under each
    /// tread's middle is that tread's top.
    #[test]
    fn the_mill_stairs_climb_a_storey() {
        let g = geo("ore_mill").unwrap();
        for k in 0..3 {
            let (dir, foot, land, (za, zb)) = stair_plan(k);
            let zc = (za + zb) * 0.5;
            let mut last = MILL_FLOORS[k];
            let mut ax = foot + TREAD * 0.5;
            while ax < INNER {
                let y = stepped_floor("ore_mill", Vec2::new(dir * ax, zc)).unwrap();
                assert!(
                    y - last <= RISE + 1e-3 && y >= last,
                    "bay {k}: a step from {last:.2} to {y:.2} at {ax:.2}"
                );
                // Something built is underfoot there.
                let under = g.pieces.iter().any(|p| match p.shape {
                    Shape::Box(s) if p.collide => {
                        let l = p.at.rotation.inverse()
                            * (Vec3::new(dir * ax, y - 0.01, zc) - p.at.translation);
                        (l.abs() - s * 0.5).max_element() <= 0.0
                    }
                    _ => false,
                });
                assert!(under, "bay {k}: nothing under the step at {ax:.2}, {y:.2}");
                last = y;
                ax += TREAD * 0.5;
            }
            assert!(
                (last - MILL_FLOORS[k + 1]).abs() < 1e-3,
                "bay {k}: the stair tops out at {last:.2}"
            );
            let _ = land;
        }
    }

    /// The car's wheels stand on the grade: each wheel's lowest point is
    /// on its rail's head, 2 cm over the plane through the car's middle.
    #[test]
    fn the_car_stands_on_the_grade() {
        let g = geo("funicular_car").unwrap();
        let wheels: Vec<_> = g
            .pieces
            .iter()
            .filter(|p| matches!(p.shape, Shape::Cylinder(r, _) if (r - 0.3).abs() < 1e-3))
            .collect();
        assert_eq!(wheels.len(), 4);
        for w in wheels {
            let c = w.at.translation;
            let (s, co) = GRADE.atan().sin_cos();
            // The lowest point of a wheel on the grade: its centre less
            // its radius along the plane's normal.
            let low = c - Vec3::new(0.0, co, s) * 0.3;
            let plane = -GRADE * low.z;
            let over = (low.y - plane) * co;
            assert!(
                (over - 0.02).abs() < 0.01,
                "a wheel's foot is {over:.3} m off the grade at z {:.2}",
                low.z
            );
        }
        // Each compartment's floor is level and a step under the last.
        for i in 1..CAR_ROOMS {
            let (_, _, a) = car_room(i - 1);
            let (_, _, c) = car_room(i);
            assert!(a > c, "compartment {i} isn't below the one before it");
        }
    }
}
