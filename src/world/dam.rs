//! Sallow Reservoir's structures (`docs/FOURTH_VALLEY.md`): the dam's two
//! faces either side of its crest, the draw-off tower in front of it with
//! the stair inside and the bridge to the crest, and the Custodian's stock
//! laid out in rows on the drained bed with the gantry over them.
//!
//! The crest itself is terrain (a plateau in the map), so the walkers, the
//! sim's bots and the map lint all walk it; these pieces face it. Each face
//! is placed `absolute` at the foot of its wall, so its local y is metres
//! above that foot. Built from the same [`Builder`] as every other prefab;
//! the pieces are their own visuals until models are made for them.

use bevy::prelude::*;

use super::buildings::{Builder, Geo, Shape};
use crate::render::materials::Mat;

/// Every prefab built here.
pub const PREFABS: [&str; 9] = [
    "dam_face",
    "dam_face_end",
    "dam_back",
    "dam_back_end",
    "draw_off_tower",
    "stock_buses",
    "stock_bicycles",
    "stock_bins",
    "sorting_gantry",
];

/// The crest above the foot of the downstream face (the valley floor at
/// 30 m, the crest at 52).
const FACE_H: f32 = 22.0;
/// The crest above the foot of the upstream face (the bed at 31 m).
const BACK_H: f32 = 21.0;
/// A parapet's height over the crest and its thickness.
const PARAPET: (f32, f32) = (1.1, 0.5);
/// The draw-off tower: its plan (square), wall thickness, the bridge
/// floor over its foot (the crest), its height, and how far its bridge
/// runs to the crest's edge.
const TOWER_W: f32 = 8.0;
const TOWER_WALL: f32 = 0.6;
pub const TOWER_BRIDGE: f32 = 20.5;
const TOWER_H: f32 = 34.0;
const BRIDGE_RUN: f32 = 24.6;
/// Treads in the tower's stair: rise and run.
const RISE: f32 = 0.2;
const TREAD: f32 = 0.28;

pub fn geo(id: &str) -> Option<Geo> {
    let mut b = Builder::default();
    match id {
        "dam_face" => face(&mut b, 265.0, true),
        "dam_face_end" => face(&mut b, 25.0, false),
        "dam_back" => back(&mut b, 245.0, Some(-22.5)),
        "dam_back_end" => back(&mut b, 35.0, None),
        "draw_off_tower" => tower(&mut b),
        "stock_buses" => buses(&mut b),
        "stock_bicycles" => bicycles(&mut b),
        "stock_bins" => bins(&mut b),
        "sorting_gantry" => gantry(&mut b),
        _ => return None,
    }
    Some(b.finish())
}

/// The downstream face: masonry in three courses, each set back from the
/// one below (the batter), pilasters down it, and a parapet along the
/// crest's outer edge. The crest is on its -Z side; the long one has a
/// turret at each end, and a pier where the face stair comes up.
fn face(b: &mut Builder, len: f32, turrets: bool) {
    let hl = len * 0.5;
    // (bottom, top, how far the course's face stands out from the crest's
    // edge at -2).
    let courses = [(0.0, 8.0, 4.0), (8.0, 15.0, 3.4), (15.0, FACE_H, 2.8)];
    for (y0, y1, t) in courses {
        b.bx(
            Vec3::new(len, y1 - y0, t),
            Vec3::new(0.0, (y0 + y1) * 0.5, -2.0 + t * 0.5),
            Mat::DressedStone,
        );
        // Its coping: a string course a hand proud of the face.
        b.deco(
            Vec3::new(len + 0.1, 0.3, 0.12),
            Vec3::new(0.0, y1 - 0.15, -2.0 + t + 0.05),
            Mat::Boulder,
        );
    }
    // Pilasters every 16 m, stepping back with the courses.
    let n = (len / 16.0).floor() as i32;
    for k in 0..=n {
        let x = -hl + 2.0 + (len - 4.0) * k as f32 / n.max(1) as f32;
        for (y0, y1, t) in courses {
            b.bx(
                Vec3::new(1.4, y1 - y0 - 0.3, 0.6),
                Vec3::new(x, (y0 + y1 - 0.3) * 0.5, -2.0 + t + 0.3),
                Mat::Boulder,
            );
        }
    }
    // Every set-back weathered: the courses' steps and the pilasters'
    // tops sloped back, so nothing on the face is a ledge to stand on.
    for (i, &(_, y1, t)) in courses.iter().enumerate() {
        if let Some(&(_, _, above)) = courses.get(i + 1) {
            weathering(b, 0.0, len, -2.0 + t, -2.0 + above, y1, Mat::DressedStone);
        }
    }
    for k in 0..=n {
        let x = -hl + 2.0 + (len - 4.0) * k as f32 / n.max(1) as f32;
        for (_, y1, t) in courses {
            weathering(b, x, 1.4, -2.0 + t + 0.6, -2.0 + t, y1 - 0.3, Mat::Boulder);
        }
    }
    // The parapet on the outer edge, the walk behind it flush with the
    // crest.
    let (ph, pt) = PARAPET;
    let pz = -2.0 + 2.8 - pt * 0.5;
    b.bx(
        Vec3::new(len, ph, pt),
        Vec3::new(0.0, FACE_H + ph * 0.5, pz),
        Mat::DressedStone,
    );
    b.deco(
        Vec3::new(len + 0.1, 0.12, pt + 0.2),
        Vec3::new(0.0, FACE_H + ph + 0.06, pz),
        Mat::Boulder,
    );
    // A pier at each end: the face's corners, and the gate posts of the
    // face stair's landing between the two faces.
    for x in [-hl + 0.9, hl - 0.9] {
        b.bx(
            Vec3::new(1.8, FACE_H + ph + 0.6, 4.2),
            Vec3::new(x, (FACE_H + ph + 0.6) * 0.5, 0.1),
            Mat::Boulder,
        );
    }
    if turrets {
        stair_wall(b);
        // A turret at each end over the parapet: what you see of the dam
        // from the truck, either side of the valley.
        for x in [-hl + 3.0, hl - 3.0] {
            let (w, h) = (3.4, 5.0);
            b.bx(
                Vec3::new(w, h, w),
                Vec3::new(x, FACE_H + h * 0.5, pz - 0.9),
                Mat::DressedStone,
            );
            b.piece(
                Shape::Prism(Vec3::new(w + 0.4, 1.6, w + 0.4)),
                Transform::from_xyz(x, FACE_H + h, pz - 0.9),
                Mat::Shingles,
                true,
            );
        }
    }
}

/// How steep a set-back's weathering is, degrees: past what a body stands
/// on (`player`'s ground is a normal over 0.65 up, under 49 degrees), so
/// anything landing on one slides off it.
const WEATHERING: f32 = 60.0;

/// Weathering on a set-back: its top sloped back from its outer edge
/// (local z `out`, at height `y`) up to the face behind (`inn`), `w` wide
/// about `x`.
fn weathering(b: &mut Builder, x: f32, w: f32, out: f32, inn: f32, y: f32, mat: Mat) {
    let a = WEATHERING.to_radians();
    let run = out - inn;
    // As thick as the set-back is deep, so it fills the wedge down to the
    // top it covers (its thickness goes into the masonry, out of sight).
    let thick = run;
    // Tipped about x: its +z end (the outer edge) down.
    let rot = Quat::from_rotation_x(a);
    let mid = Vec3::new(x, y + run * a.tan() * 0.5, (out + inn) * 0.5);
    b.piece(
        Shape::Box(Vec3::new(w, thick, run / a.cos())),
        Transform::from_translation(mid - rot * Vec3::Y * (thick * 0.5)).with_rotation(rot),
        mat,
        true,
    );
}

/// The face stair's run, as the map lays its terraces (from the west end
/// at the valley floor to the landing at crest height), its walls' foot,
/// and the long downstream face's placement (x, z, y): the walls are built
/// as part of that face, so they're no placement of their own for the lint
/// to count.
const STAIR_RUN: (f32, f32) = (-30.0, 90.0);
const STAIR_RISE: (f32, f32) = (30.0, 52.0);
const STAIR_WALL_FOOT: f32 = 29.0;
pub const DAM_FACE_AT: (f32, f32, f32) = (-42.5, 148.0, 30.0);
/// The stair's walls, map x0..x1, z0..z1: the outer wall along the
/// embankment's south edge (3 m thick, from the stair path's shoulder to
/// past the foot of the bank), on past the landing's south side to its far
/// corner, and the landing's east side back to the face end. Nowhere off
/// the stair or the landing but over a wall.
const STAIR_WALLS: [(f32, f32, f32, f32); 2] =
    [(-30.0, 103.0, 157.1, 160.1), (100.0, 103.0, 150.0, 157.1)];

/// The face stair's height over the valley floor at map x, as the road on
/// its terraces grades it (the landing's beyond its east end).
fn stair_height(x: f32) -> f32 {
    let (x0, x1) = STAIR_RUN;
    let (h0, h1) = STAIR_RISE;
    h0 + (h1 - h0) * ((x - x0) / (x1 - x0)).clamp(0.0, 1.0)
}

/// The face stair's walls, in the long downstream face's space: masonry
/// thick enough to hold the bank's whole fall to the grass, its top a
/// metre over the steps or the landing (a parapet to lean a rifle on, and
/// what stops you stepping off 20 m of embankment), in panels of about 4 m
/// stepping up with the stair.
fn stair_wall(b: &mut Builder) {
    let (fx, fz, fy) = DAM_FACE_AT;
    let ly = STAIR_WALL_FOOT - fy;
    for (x0, x1, z0, z1) in STAIR_WALLS {
        let along_x = x1 - x0 > z1 - z0;
        let len = if along_x { x1 - x0 } else { z1 - z0 };
        let n = (len / 4.0).ceil();
        let panel = len / n;
        for k in 0..n as i32 {
            let a = panel * k as f32;
            // Centre and size in plan, and the stair's highest x along it.
            let (cx, cz, sx, sz, high) = if along_x {
                (
                    x0 + a + panel * 0.5,
                    (z0 + z1) * 0.5,
                    panel + 0.02,
                    z1 - z0,
                    x0 + a + panel,
                )
            } else {
                (
                    (x0 + x1) * 0.5,
                    z0 + a + panel * 0.5,
                    x1 - x0,
                    panel + 0.02,
                    x1,
                )
            };
            let top = stair_height(high) + 1.0 - STAIR_WALL_FOOT;
            let (lx, lz) = (cx - fx, cz - fz);
            b.bx(
                Vec3::new(sx, top, sz),
                Vec3::new(lx, ly + top * 0.5, lz),
                Mat::DressedStone,
            );
            b.deco(
                Vec3::new(sx + 0.06, 0.1, sz + 0.1),
                Vec3::new(lx, ly + top + 0.05, lz),
                Mat::Boulder,
            );
        }
    }
}

/// The upstream face: plain masonry down to the bed, the stone below the
/// old top water line stained where the water stood (`Boulder`), a
/// parapet along the crest's edge, open where the tower's bridge comes
/// in at `gap` (x).
fn back(b: &mut Builder, len: f32, gap: Option<f32>) {
    let hl = len * 0.5;
    let line = BACK_H - 2.2;
    b.bx(
        Vec3::new(len, BACK_H, 3.0),
        Vec3::new(0.0, BACK_H * 0.5, 0.0),
        Mat::DressedStone,
    );
    // Where the water stood: the stained skin, a tide line along its top.
    b.deco(
        Vec3::new(len, line, 0.02),
        Vec3::new(0.0, line * 0.5, -1.51),
        Mat::Boulder,
    );
    b.deco(
        Vec3::new(len, 0.25, 0.04),
        Vec3::new(0.0, line, -1.52),
        Mat::DarkMetal,
    );
    let (ph, pt) = PARAPET;
    let pz = -1.5 + pt * 0.5;
    let mut runs = vec![(-hl, hl)];
    if let Some(g) = gap {
        runs = vec![(-hl, g - 1.2), (g + 1.2, hl)];
    }
    for (a, c) in runs {
        b.bx(
            Vec3::new(c - a, ph, pt),
            Vec3::new((a + c) * 0.5, BACK_H + ph * 0.5, pz),
            Mat::DressedStone,
        );
        b.deco(
            Vec3::new(c - a + 0.1, 0.12, pt + 0.2),
            Vec3::new((a + c) * 0.5, BACK_H + ph + 0.06, pz),
            Mat::Boulder,
        );
    }
}

/// The draw-off tower on the bed: a square stone tower with a doorway at
/// its foot (on the bed side, -Z) and another at the bridge floor, where a
/// bridge runs 25 m to the crest (+Z). Inside, a stair climbs the four
/// walls round an open well from the foot to the bridge floor; above it
/// the valve room under the roof.
fn tower(b: &mut Builder) {
    let (w, t) = (TOWER_W, TOWER_WALL);
    let h = TOWER_H;
    let hw = w * 0.5;
    // Footing and the ground floor.
    b.bx(
        Vec3::new(w + 1.0, 0.6, w + 1.0),
        Vec3::new(0.0, -0.1, 0.0),
        Mat::Boulder,
    );
    // Walls, as four slabs each side of the doorways (a doorway is 1.2 wide
    // and 2.4 high): the foot's in the -Z wall, the bridge's in the +Z.
    let door = |b: &mut Builder, z: f32, y0: f32| {
        let side = (w - 1.2) * 0.5;
        for x in [-(1.2 + side) * 0.5, (1.2 + side) * 0.5] {
            b.bx(
                Vec3::new(side, 2.4, t),
                Vec3::new(x, y0 + 1.2, z),
                Mat::DressedStone,
            );
        }
    };
    for z in [-hw + t * 0.5, hw - t * 0.5] {
        let doorway = if z < 0.0 { 0.0 } else { TOWER_BRIDGE };
        door(b, z, doorway);
        // The wall below and above the doorway.
        if doorway > 0.0 {
            b.bx(
                Vec3::new(w, doorway, t),
                Vec3::new(0.0, doorway * 0.5, z),
                Mat::DressedStone,
            );
        }
        let top = doorway + 2.4;
        b.bx(
            Vec3::new(w, h - top, t),
            Vec3::new(0.0, (top + h) * 0.5, z),
            Mat::DressedStone,
        );
    }
    for x in [-hw + t * 0.5, hw - t * 0.5] {
        b.bx(
            Vec3::new(t, h, w - 2.0 * t),
            Vec3::new(x, h * 0.5, 0.0),
            Mat::DressedStone,
        );
    }
    // Slits of light up the walls (dark glass, visual only), and a band
    // of stone at the bridge floor and under the eaves.
    for y in [5.0, 10.0, 15.0, 26.0] {
        for (x, z, yaw) in [(hw + 0.01, 0.0, 0.0), (-hw - 0.01, 0.0, 0.0)] {
            b.piece(
                Shape::Box(Vec3::new(0.02, 1.4, 0.35)),
                Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(yaw)),
                Mat::Glass,
                false,
            );
        }
    }
    for y in [TOWER_BRIDGE - 0.4, h - 0.6] {
        b.deco(
            Vec3::new(w + 0.3, 0.4, w + 0.3),
            Vec3::new(0.0, y, 0.0),
            Mat::Boulder,
        );
    }
    // The roof: a gable, its ridge across the bridge.
    b.piece(
        Shape::Prism(Vec3::new(w + 0.8, 3.2, w + 0.8)),
        Transform::from_xyz(0.0, h, 0.0),
        Mat::Shingles,
        true,
    );
    stair(b);
    // The valve room: a floor over the well at the bridge floor (all but
    // the last flight, which comes up beside it), the winch stands for the
    // draw-off valves, a crate the keeper left.
    let inner = w - 2.0 * t;
    let open = 1.2;
    b.bx(
        Vec3::new(inner - open, 0.3, inner),
        Vec3::new(-open * 0.5, TOWER_BRIDGE - 0.15, 0.0),
        Mat::Concrete,
    );
    for x in [-1.5, 1.5] {
        b.bx(
            Vec3::new(0.6, 1.0, 0.6),
            Vec3::new(x, TOWER_BRIDGE + 0.5, -1.6),
            Mat::PaintedMetal,
        );
        b.piece(
            Shape::Cylinder(0.45, 0.08),
            Transform::from_xyz(x, TOWER_BRIDGE + 1.2, -1.6)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Mat::RustyMetal,
            false,
        );
    }
    b.crate_box(Vec3::new(-2.2, TOWER_BRIDGE, -2.6), 0.8, Some("tools"));
    // The chain the keeper cut off the big wheel, lying where he left it:
    // links along the floor from the east winch.
    for k in 0..9 {
        let t = k as f32 * 0.11;
        let yaw = if k % 2 == 0 { 0.4 } else { -1.17 };
        b.piece(
            Shape::Box(Vec3::new(0.09, 0.025, 0.05)),
            Transform::from_xyz(1.2 - t, TOWER_BRIDGE + 0.013, -1.0 + t * 0.6)
                .with_rotation(Quat::from_rotation_y(yaw)),
            Mat::RustyMetal,
            false,
        );
    }
    // The keeper's table by the bridge door (Story's note lies on it).
    b.table(Vec3::new(-1.0, TOWER_BRIDGE, 1.9), 0.0, None);
    // The bridge to the crest: its deck, rails, and two piers down to the
    // bed.
    let (z0, z1) = (hw, hw + BRIDGE_RUN);
    let deck = Vec3::new(2.0, 0.3, z1 - z0);
    b.bx(
        deck,
        Vec3::new(0.0, TOWER_BRIDGE - 0.15, (z0 + z1) * 0.5),
        Mat::Concrete,
    );
    for x in [-1.0, 1.0] {
        b.bx(
            Vec3::new(0.12, 1.05, z1 - z0),
            Vec3::new(x * 0.94, TOWER_BRIDGE + 0.525, (z0 + z1) * 0.5),
            Mat::PaintedMetal,
        );
    }
    for z in [z0 + 9.5, z0 + 19.0] {
        b.bx(
            Vec3::new(1.6, TOWER_BRIDGE + 2.7, 1.6),
            Vec3::new(0.0, (TOWER_BRIDGE - 0.3 - 3.0) * 0.5, z),
            Mat::Concrete,
        );
    }
}

/// The stair inside the tower: flights up the four walls in turn,
/// clockwise from the foot's doorway, with a landing in each corner, to
/// the bridge doorway. Treads and landings are solid; a rail runs along
/// each flight's inner edge, over the well.
fn stair(b: &mut Builder) {
    let inner = TOWER_W - 2.0 * TOWER_WALL;
    let hi = inner * 0.5;
    let (width, land) = (1.2, 1.3);
    let run = inner - 2.0 * land;
    let per_flight = (run / TREAD).floor() as usize;
    let flights = (TOWER_BRIDGE / (per_flight as f32 * RISE)).ceil() as usize;
    let rise = TOWER_BRIDGE / (flights * per_flight) as f32;
    // Walking clockwise seen from above: along +X by the -Z wall, along +Z
    // by the +X wall, along -X by the +Z wall, along -Z by the -X wall.
    let legs: [(Vec2, Vec2); 4] = [
        (Vec2::new(-hi + land, -hi), Vec2::X),
        (Vec2::new(hi, -hi + land), Vec2::Y),
        (Vec2::new(hi - land, hi), -Vec2::X),
        (Vec2::new(-hi, hi - land), -Vec2::Y),
    ];
    let mut y = 0.0;
    // The foot's doorway is in the -Z wall's middle: start on the far leg
    // so the first flight leaves the door clear.
    for f in 0..flights {
        let (start, dir) = legs[(f + 3) % 4];
        // The wall is on the flight's outer side; treads stand off it by
        // half their width, inward.
        let off = Vec2::new(-dir.y, dir.x) * (width * 0.5);
        let off = if (start + off).length() < (start - off).length() {
            off
        } else {
            -off
        };
        for s in 0..per_flight {
            let p = start + dir * (TREAD * (s as f32 + 0.5)) + off;
            let top = y + rise * (s + 1) as f32;
            let size = if dir.x != 0.0 {
                Vec3::new(TREAD, rise, width)
            } else {
                Vec3::new(width, rise, TREAD)
            };
            b.bx(size, Vec3::new(p.x, top - rise * 0.5, p.y), Mat::Concrete);
        }
        y += rise * per_flight as f32;
        // The rail along the flight's inner edge, sloping with it.
        let a = start + off * 2.0;
        let c = a + dir * run;
        let (ya, yc) = (y - rise * per_flight as f32 + 1.0, y + 1.0);
        let along = Vec3::new(c.x - a.x, yc - ya, c.y - a.y);
        b.piece(
            Shape::Box(Vec3::new(0.06, 0.06, along.length())),
            Transform::from_translation(Vec3::new(
                (a.x + c.x) * 0.5,
                (ya + yc) * 0.5,
                (a.y + c.y) * 0.5,
            ))
            .looking_to(along, Vec3::Y),
            Mat::PaintedMetal,
            true,
        );
        // The landing in the corner at the flight's top.
        let corner = start + dir * (run + land * 0.5) + off;
        b.bx(
            Vec3::new(land.max(width), 0.2, land.max(width)),
            Vec3::new(corner.x, y - 0.1, corner.y),
            Mat::Concrete,
        );
    }
}

/// Four buses side by side, nose to the -Z end, as the Custodian parks
/// its stock: kept, not driven. Each is a 2.5 x 10 m bus with a ticket in
/// the windscreen.
fn buses(b: &mut Builder) {
    for (i, x) in [-4.95f32, -1.65, 1.65, 4.95].into_iter().enumerate() {
        let from = b.geo.pieces.len();
        let breakables = b.geo.breakables.len();
        super::buildings::wrecked_bus(b, Vec3::new(x, 0.0, 0.0));
        // Kept stock isn't prised apart: the wreck's panels and door stay
        // solid, under the model that draws the bus whole.
        for k in b.geo.breakables.split_off(breakables) {
            b.piece(Shape::Box(k.size), k.at, k.mat, true);
        }
        // Drawn by `kept_bus` (art/props/dam.py); its boxes stay the
        // colliders.
        b.dress_structure(from, vec![("kept_bus", Transform::from_xyz(x, 0.0, 0.0))]);
        // The tag: a white card behind the windscreen, its number on it.
        b.deco(
            Vec3::new(0.5, 0.35, 0.02),
            Vec3::new(x + 0.6 - 0.3 * (i % 2) as f32, 2.25, -5.0),
            Mat::Plaster,
        );
    }
}

/// A row of bicycles in a rack, 24 m of them along z, each tagged on its
/// saddle: two rails to lean them on, and the bicycles (visual only: the
/// rails are what stops you).
fn bicycles(b: &mut Builder) {
    let len = 24.0;
    for (y, h) in [(0.45, 0.06), (0.85, 0.06)] {
        b.bx(
            Vec3::new(0.06, h, len),
            Vec3::new(0.0, y, 0.0),
            Mat::RustyMetal,
        );
    }
    for z in [-len * 0.5 + 0.2, 0.0, len * 0.5 - 0.2] {
        b.bx(
            Vec3::new(0.08, 0.9, 0.08),
            Vec3::new(0.0, 0.45, z),
            Mat::RustyMetal,
        );
    }
    let n = 38;
    let from = b.geo.pieces.len();
    let mut models = Vec::new();
    for k in 0..n {
        let z = -len * 0.5 + 0.4 + (len - 0.8) * k as f32 / (n - 1) as f32;
        // Alternate sides of the rack, front wheel in.
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let (front, rear) = (side * 0.15, side * 1.15);
        for x in [front, rear] {
            b.piece(
                Shape::Cylinder(0.34, 0.04),
                Transform::from_xyz(x, 0.34, z)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                Mat::DarkMetal,
                false,
            );
        }
        // The frame: top tube, down tube, saddle, bars.
        let mid = (front + rear) * 0.5;
        b.deco(
            Vec3::new(0.62, 0.04, 0.04),
            Vec3::new(mid, 0.72, z),
            Mat::PaintedMetal,
        );
        b.piece(
            Shape::Box(Vec3::new(0.72, 0.04, 0.04)),
            Transform::from_xyz(mid - side * 0.12, 0.52, z)
                .with_rotation(Quat::from_rotation_z(side * 0.6)),
            Mat::PaintedMetal,
            false,
        );
        b.deco(
            Vec3::new(0.24, 0.06, 0.1),
            Vec3::new(rear - side * 0.25, 0.82, z),
            Mat::Polymer,
        );
        b.deco(
            Vec3::new(0.06, 0.06, 0.5),
            Vec3::new(front + side * 0.05, 0.92, z),
            Mat::DarkMetal,
        );
        // `kept_bicycle` runs along +X, front wheel first.
        let yaw = if side > 0.0 {
            std::f32::consts::PI
        } else {
            0.0
        };
        models.push((
            "kept_bicycle",
            Transform::from_xyz(mid, 0.0, z).with_rotation(Quat::from_rotation_y(yaw)),
        ));
    }
    // Drawn near, as furniture is.
    b.dress(from, models);
}

/// Wheelie bins in a rank along z, lids shut, 20 m of them.
fn bins(b: &mut Builder) {
    let len = 20.0;
    let n = 27;
    let from = b.geo.pieces.len();
    let mut models = Vec::new();
    for k in 0..n {
        let z = -len * 0.5 + 0.36 + (len - 0.72) * k as f32 / (n - 1) as f32;
        let mat = if k % 3 == 1 {
            Mat::PaintedMetal
        } else {
            Mat::Polymer
        };
        b.bx(Vec3::new(0.58, 0.95, 0.66), Vec3::new(0.0, 0.5, z), mat);
        b.deco(Vec3::new(0.62, 0.06, 0.7), Vec3::new(-0.02, 1.0, z), mat);
        b.deco(
            Vec3::new(0.06, 0.08, 0.5),
            Vec3::new(-0.33, 0.95, z),
            Mat::DarkMetal,
        );
        models.push(("wheelie_bin", Transform::from_xyz(0.0, 0.0, z)));
    }
    b.dress(from, models);
}

/// The Custodian's gantry over its rows: two braced legs 19 m apart, a
/// box girder along the top at 13 m, the trolley on it and a grab
/// hanging from its cables.
fn gantry(b: &mut Builder) {
    let (span, h, legs) = (19.0, 13.0, 5.0);
    for x in [-span * 0.5, span * 0.5] {
        for z in [-legs * 0.5, legs * 0.5] {
            b.bx(
                Vec3::new(0.5, h, 0.5),
                Vec3::new(x, h * 0.5, z),
                Mat::PaintedMetal,
            );
        }
        // Cross-bracing between the leg's posts, and its foot beam.
        for y in [4.0, 8.5] {
            b.deco(
                Vec3::new(0.25, 0.25, legs),
                Vec3::new(x, y, 0.0),
                Mat::PaintedMetal,
            );
        }
        b.bx(
            Vec3::new(1.2, 0.6, legs + 1.0),
            Vec3::new(x, 0.3, 0.0),
            Mat::DarkMetal,
        );
    }
    b.bx(
        Vec3::new(span + 1.0, 1.4, 1.2),
        Vec3::new(0.0, h + 0.7, 0.0),
        Mat::PaintedMetal,
    );
    for z in [-legs * 0.5, legs * 0.5] {
        b.deco(
            Vec3::new(span, 0.3, 0.3),
            Vec3::new(0.0, h - 0.15, z),
            Mat::PaintedMetal,
        );
    }
    // The tagging desk between the legs, under the girder (the stock
    // ground's terminal stands on it).
    b.table(Vec3::new(-6.0, 0.0, 0.0), 0.0, None);
    // The trolley over the third bay, its cables and the grab.
    let tx = 3.0;
    b.bx(
        Vec3::new(2.0, 1.0, 2.0),
        Vec3::new(tx, h + 1.9, 0.0),
        Mat::RobotShell,
    );
    for dz in [-0.4, 0.4] {
        b.deco(
            Vec3::new(0.04, 6.0, 0.04),
            Vec3::new(tx, h - 3.0, dz),
            Mat::DarkMetal,
        );
    }
    b.bx(
        Vec3::new(1.6, 0.5, 1.2),
        Vec3::new(tx, h - 6.25, 0.0),
        Mat::RobotShell,
    );
    for sx in [-0.7, 0.7] {
        b.piece(
            Shape::Box(Vec3::new(0.15, 1.3, 1.0)),
            Transform::from_xyz(tx + sx, h - 7.0, 0.0)
                .with_rotation(Quat::from_rotation_z(sx * 0.4)),
            Mat::RobotShell,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every piece is real (as `buildings::every_piece_is_sound`), and each
    /// prefab stays inside the footprint the map lint and nav grid use,
    /// bar the tower's bridge, which is in the air.
    #[test]
    fn sallow_prefabs_are_sound_and_fit_their_footprints() {
        for id in PREFABS {
            let g = geo(id).unwrap_or_else(|| panic!("{id} builds"));
            let [w, _, d] = extraction_core::map::prefab_footprint(id);
            for (i, p) in g.pieces.iter().enumerate() {
                let ok = match p.shape {
                    Shape::Box(s) | Shape::Prism(s) => s.is_finite() && s.min_element() > 0.0,
                    Shape::Cylinder(r, h) => r > 0.0 && h > 0.0,
                } && p.at.translation.is_finite()
                    && p.at.rotation.is_finite();
                assert!(ok, "{id} piece {i}: {:?} at {:?}", p.shape, p.at);
                let at = p.at.translation;
                let bridge = id == "draw_off_tower" && at.y > 3.0 && at.z > TOWER_W * 0.5;
                // Walls built out past the footprint (the face stair's)
                // are listed for the nav grid, and stand wholly inside the
                // listing.
                let half = match p.shape {
                    Shape::Box(s) | Shape::Prism(s) => s * 0.5,
                    Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
                };
                let stair = extraction_core::map::prefab_walls(id).iter().any(|(o, h)| {
                    (at.x - o[0]).abs() + half.x <= h[0] + 0.1
                        && (at.z - o[1]).abs() + half.z <= h[1] + 0.1
                });
                // Pilasters and turrets stand a little proud of the faces.
                let slack = 1.5;
                assert!(
                    bridge
                        || stair
                        || (at.x.abs() <= w * 0.5 + slack && at.z.abs() <= d * 0.5 + slack),
                    "{id} piece {i} at {at:?} is outside its {w} x {d} footprint"
                );
            }
        }
    }

    /// The face stair's walls stand where the map lays the stair: built
    /// with the long downstream face, so that face's placement is where
    /// they reckon from; the road on the terraces is at the walls' stair
    /// height; their feet are buried all the way; the nav grid's listing
    /// of them is them; and the stair runs right up against the face, no
    /// drop between them for a body to fall down onto the courses.
    #[test]
    fn the_stair_walls_follow_the_stair() {
        let map =
            extraction_core::map::MapDef::from_ron(extraction_core::map::SALLOW_RESERVOIR).unwrap();
        let hm = extraction_core::terrain::Heightmap::build(&map);
        let face = map
            .placements
            .iter()
            .find(|p| p.prefab == "dam_face")
            .expect("the downstream face is placed");
        assert_eq!((face.at[0], face.at[1], face.y), DAM_FACE_AT);
        let walls = extraction_core::map::prefab_walls("dam_face");
        for (x0, x1, z0, z1) in STAIR_WALLS {
            let want = (
                [
                    (x0 + x1) * 0.5 - DAM_FACE_AT.0,
                    (z0 + z1) * 0.5 - DAM_FACE_AT.1,
                ],
                [(x1 - x0) * 0.5, (z1 - z0) * 0.5],
            );
            assert!(
                walls.iter().any(|(o, h)| (0..2)
                    .all(|k| (o[k] - want.0[k]).abs() < 0.05 && (h[k] - want.1[k]).abs() < 0.05)),
                "map::prefab_walls(\"dam_face\") is {walls:?}, a wall {want:?}"
            );
            for fx in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                for z in [z0, z1] {
                    let x = x0 + (x1 - x0) * fx;
                    let ground = hm.height([x, z]);
                    assert!(
                        ground > STAIR_WALL_FOOT,
                        "a stair wall's foot shows at ({x}, {z}): ground {ground:.1}"
                    );
                }
            }
        }
        for x in [-25.0f32, 0.0, 14.0, 22.0, 30.0, 60.0, 85.0] {
            let road = hm.height([x, 154.0]);
            let want = stair_height(x);
            assert!(
                (road - want).abs() < 1.2,
                "the stair at x {x}: {road:.1} vs {want:.1}"
            );
            // Both sides: the face behind, the wall in front (a building's
            // door apron below once cut the south strip down 7 m).
            for z in [149.2f32, 150.2, 151.2, 155.6, 156.6] {
                let ground = hm.height([x, z]);
                assert!(
                    ground > want - 0.5,
                    "a drop beside the stair at ({x}, {z}): {ground:.1} under a stair at {want:.1}"
                );
            }
        }
    }

    /// Nothing on the dam's downstream faces is a ledge: every top a body
    /// could land on below the crest is under something (the next course,
    /// the embankment's stone) or weathered past standing, but for the
    /// stair walls' copings (a metre over the steps, stepped down off).
    /// Sampled over each solid box's top, inset from its edges.
    #[test]
    fn the_faces_have_no_ledges() {
        let solid = |g: &Geo, at: Vec3, skip: usize| {
            g.pieces.iter().enumerate().any(|(i, p)| {
                if i == skip || !p.collide {
                    return false;
                }
                let Shape::Box(s) = p.shape else {
                    return false;
                };
                let l = p.at.compute_affine().inverse().transform_point3(at);
                (l.abs() - s * 0.5).max_element() <= 0.0
            })
        };
        for id in ["dam_face", "dam_face_end"] {
            let g = geo(id).unwrap();
            let walls = extraction_core::map::prefab_walls(id);
            for (i, p) in g.pieces.iter().enumerate() {
                let Shape::Box(s) = p.shape else { continue };
                if !p.collide || p.at.rotation.angle_between(Quat::IDENTITY) > 0.01 {
                    continue;
                }
                let c = p.at.translation;
                let top = c.y + s.y * 0.5;
                if top >= FACE_H - 0.01 {
                    continue;
                }
                let on_wall = walls.iter().any(|(o, h)| {
                    (c.x - o[0]).abs() <= h[0] + 0.1 && (c.z - o[1]).abs() <= h[1] + 0.1
                });
                if on_wall {
                    continue;
                }
                let inset = |half: f32| (half - 0.15).max(0.0);
                for fx in [-1.0f32, 0.0, 1.0] {
                    for fz in [-1.0f32, 0.0, 1.0] {
                        let at = Vec3::new(
                            c.x + fx * inset(s.x * 0.5),
                            top + 0.02,
                            c.z + fz * inset(s.z * 0.5),
                        );
                        assert!(
                            solid(&g, at, i),
                            "{id} piece {i}: its top at {at:?} is a ledge ({s:?} at {c:?})"
                        );
                    }
                }
            }
        }
    }

    /// The Custodian's stock is kept whole: nothing on it prises off (the
    /// models draw it entire, so a gap would show nowhere).
    #[test]
    fn kept_stock_has_nothing_to_prise_off() {
        for id in PREFABS {
            let g = geo(id).unwrap();
            assert!(
                g.breakables.is_empty(),
                "{id}: {} breakables",
                g.breakables.len()
            );
        }
    }

    /// Every model a Sallow prefab is dressed with is built
    /// (`art/props/dam.py`, `python3 art/build.py dam`).
    #[test]
    fn sallow_models_exist() {
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

    /// The tower's bridge comes in level with the crest: the deck's top at
    /// the bridge floor, and as long as the bed's gap from the tower to the
    /// crest's edge in the map.
    #[test]
    fn the_tower_bridge_meets_the_crest() {
        let map =
            extraction_core::map::MapDef::from_ron(extraction_core::map::SALLOW_RESERVOIR).unwrap();
        let tower = map
            .placements
            .iter()
            .find(|p| p.prefab == "draw_off_tower")
            .expect("the tower is placed");
        let back = map
            .placements
            .iter()
            .find(|p| p.prefab == "dam_back")
            .expect("the upstream face is placed");
        let crest = back.y + BACK_H;
        assert!(
            (tower.y + TOWER_BRIDGE - crest).abs() < 0.05,
            "the bridge floor is at {:.2}, the crest at {crest:.2}",
            tower.y + TOWER_BRIDGE
        );
        let reach = tower.at[1] + TOWER_W * 0.5 + BRIDGE_RUN;
        let edge = back.at[1] - 1.5;
        assert!(
            reach > edge && reach < back.at[1] + 1.5,
            "the bridge ends at z {reach:.1}, the face's bed side at {edge:.1}"
        );
    }

    /// The stair climbs from the foot to the bridge floor in steps a body
    /// takes (`player::STEP_HEIGHT` is 0.4).
    #[test]
    fn the_tower_stair_reaches_the_bridge() {
        let g = geo("draw_off_tower").unwrap();
        let mut treads: Vec<f32> = g
            .pieces
            .iter()
            .filter_map(|p| match p.shape {
                Shape::Box(s)
                    if s.y < 0.25 && (s.x - TREAD).abs() < 0.01 || (s.z - TREAD).abs() < 0.01 =>
                {
                    Some(p.at.translation.y + s.y * 0.5)
                }
                _ => None,
            })
            .collect();
        treads.sort_by(f32::total_cmp);
        assert!(!treads.is_empty());
        assert!(treads[0] < 0.4, "first tread at {:.2}", treads[0]);
        let top = *treads.last().unwrap();
        assert!(
            (top - TOWER_BRIDGE).abs() < 0.05,
            "the stair tops out at {top:.2}"
        );
        for w in treads.windows(2) {
            assert!(w[1] - w[0] < 0.3, "a step of {:.2}", w[1] - w[0]);
        }
    }
}
