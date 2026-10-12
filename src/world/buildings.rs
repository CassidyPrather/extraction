//! Buildings and structures, generated from short recipes.
//!
//! Each prefab is a recipe built with [`Builder`]: walls with doors and
//! windows cut out of them, floors, roofs, partitions and furniture. The
//! output is a list of [`Piece`]s (boxes, gable prisms, cylinders) with
//! materials and collision, plus loot spots on furniture and hinged doors.
//! Logic spawns the colliders, loot spots and doors (headless-safe); the
//! client attaches meshes. Building space: origin on the ground at the
//! footprint centre, `-Z` is the front.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Layer, Zone};
use crate::render::materials::Mat;

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Box(Vec3),
    /// A gable end: width (x), height (y), depth (z).
    Prism(Vec3),
    /// Radius, height (along local Y).
    Cylinder(f32, f32),
}

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub shape: Shape,
    pub at: Transform,
    pub mat: Mat,
    pub collide: bool,
}

#[derive(Clone, Debug)]
pub struct LootSpot {
    pub at: Vec3,
    pub table: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct DoorDef {
    /// Hinge position (bottom of the hinge side).
    pub hinge: Vec3,
    pub width: f32,
    pub height: f32,
    /// Yaw of the closed door; the leaf extends along local +X from the hinge.
    pub yaw: f32,
    pub mat: Mat,
}

#[derive(Default, Clone, Debug)]
pub struct Geo {
    pub pieces: Vec<Piece>,
    pub loot: Vec<LootSpot>,
    pub doors: Vec<DoorDef>,
    /// Warm interior lamps (rare; most of the Zone is dark).
    pub lights: Vec<Vec3>,
}

/// An opening in a wall, measured along it from its start.
#[derive(Clone, Copy, Debug)]
pub struct Opening {
    pub centre: f32,
    pub width: f32,
    pub bottom: f32,
    pub top: f32,
    pub kind: OpeningKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpeningKind {
    Door,
    Window,
    /// A doorway with no door in it.
    Gap,
}

pub fn door(centre: f32) -> Opening {
    Opening {
        centre,
        width: 0.95,
        bottom: 0.0,
        top: 2.05,
        kind: OpeningKind::Door,
    }
}
pub fn gap(centre: f32, width: f32, top: f32) -> Opening {
    Opening {
        centre,
        width,
        bottom: 0.0,
        top,
        kind: OpeningKind::Gap,
    }
}
pub fn window(centre: f32) -> Opening {
    Opening {
        centre,
        width: 1.0,
        bottom: 0.9,
        top: 2.0,
        kind: OpeningKind::Window,
    }
}
pub fn wide_window(centre: f32, width: f32) -> Opening {
    Opening {
        centre,
        width,
        bottom: 0.7,
        top: 2.2,
        kind: OpeningKind::Window,
    }
}

#[derive(Default)]
pub struct Builder {
    pub geo: Geo,
}

impl Builder {
    pub fn piece(&mut self, shape: Shape, at: Transform, mat: Mat, collide: bool) -> &mut Self {
        self.geo.pieces.push(Piece {
            shape,
            at,
            mat,
            collide,
        });
        self
    }

    pub fn bx(&mut self, size: Vec3, centre: Vec3, mat: Mat) -> &mut Self {
        self.piece(
            Shape::Box(size),
            Transform::from_translation(centre),
            mat,
            true,
        )
    }

    pub fn bx_rot(&mut self, size: Vec3, centre: Vec3, yaw: f32, mat: Mat) -> &mut Self {
        self.piece(
            Shape::Box(size),
            Transform::from_translation(centre).with_rotation(Quat::from_rotation_y(yaw)),
            mat,
            true,
        )
    }

    /// Visual-only box (trim, wire, glass handled separately).
    pub fn deco(&mut self, size: Vec3, centre: Vec3, mat: Mat) -> &mut Self {
        self.piece(
            Shape::Box(size),
            Transform::from_translation(centre),
            mat,
            false,
        )
    }

    pub fn loot(&mut self, at: Vec3, table: &'static str) -> &mut Self {
        self.geo.loot.push(LootSpot { at, table });
        self
    }

    /// A wall from `a` to `b` (x, z), standing on `y0`, with openings cut out.
    pub fn wall(
        &mut self,
        a: Vec2,
        b: Vec2,
        y0: f32,
        height: f32,
        thick: f32,
        mat: Mat,
        openings: &[Opening],
    ) -> &mut Self {
        let len = a.distance(b);
        let dir = (b - a) / len.max(1e-4);
        let yaw = -dir.y.atan2(dir.x);
        let rot = Quat::from_rotation_y(yaw);
        let at = |s: f32, y: f32| Vec3::new(a.x + dir.x * s, y, a.y + dir.y * s);
        let mut ops: Vec<Opening> = openings.to_vec();
        ops.sort_by(|p, q| p.centre.total_cmp(&q.centre));
        let mut cursor = 0.0;
        let seg = |b: &mut Self, s0: f32, s1: f32, y_lo: f32, y_hi: f32| {
            if s1 - s0 < 0.01 || y_hi - y_lo < 0.01 {
                return;
            }
            let c = at((s0 + s1) * 0.5, y0 + (y_lo + y_hi) * 0.5);
            b.piece(
                Shape::Box(Vec3::new(s1 - s0, y_hi - y_lo, thick)),
                Transform::from_translation(c).with_rotation(rot),
                mat,
                true,
            );
        };
        for o in &ops {
            let s0 = (o.centre - o.width * 0.5).max(0.0);
            let s1 = (o.centre + o.width * 0.5).min(len);
            seg(self, cursor, s0, 0.0, height);
            seg(self, s0, s1, 0.0, o.bottom);
            seg(self, s0, s1, o.top, height);
            // Frame trim and contents.
            let frame = Mat::Timber;
            let mid = at(o.centre, y0);
            match o.kind {
                OpeningKind::Window => {
                    self.piece(
                        Shape::Box(Vec3::new(o.width, 0.012, 0.008)),
                        Transform::from_translation(mid + Vec3::Y * ((o.bottom + o.top) * 0.5))
                            .with_rotation(rot),
                        frame,
                        false,
                    );
                    self.piece(
                        Shape::Box(Vec3::new(o.width - 0.02, o.top - o.bottom - 0.02, 0.01)),
                        Transform::from_translation(mid + Vec3::Y * ((o.bottom + o.top) * 0.5))
                            .with_rotation(rot),
                        Mat::Glass,
                        false,
                    );
                    self.piece(
                        Shape::Box(Vec3::new(o.width + 0.1, 0.05, thick + 0.08)),
                        Transform::from_translation(mid + Vec3::Y * o.bottom).with_rotation(rot),
                        frame,
                        false,
                    );
                }
                OpeningKind::Door => {
                    let hinge = at(s0 + 0.02, y0);
                    self.geo.doors.push(DoorDef {
                        hinge,
                        width: o.width - 0.04,
                        height: o.top - o.bottom - 0.02,
                        yaw,
                        mat: Mat::Planks,
                    });
                }
                OpeningKind::Gap => {}
            }
            if o.kind != OpeningKind::Gap {
                self.piece(
                    Shape::Box(Vec3::new(o.width + 0.12, 0.08, thick + 0.04)),
                    Transform::from_translation(mid + Vec3::Y * (o.top + 0.04)).with_rotation(rot),
                    frame,
                    false,
                );
            }
            cursor = s1;
        }
        seg(self, cursor, len, 0.0, height);
        self
    }

    /// Four walls round a rectangle (centred), openings per side:
    /// front (-Z), right (+X), back (+Z), left (-X); each measured from the
    /// side's start going clockwise seen from above.
    pub fn shell(
        &mut self,
        w: f32,
        d: f32,
        y0: f32,
        h: f32,
        thick: f32,
        mat: Mat,
        sides: [&[Opening]; 4],
    ) -> &mut Self {
        let (x, z) = (w * 0.5, d * 0.5);
        let c = [
            Vec2::new(-x, -z),
            Vec2::new(x, -z),
            Vec2::new(x, z),
            Vec2::new(-x, z),
        ];
        for i in 0..4 {
            self.wall(c[i], c[(i + 1) % 4], y0, h, thick, mat, sides[i]);
        }
        self
    }

    /// A foundation that buries itself into sloping ground, and a floor.
    pub fn plinth(&mut self, w: f32, d: f32, floor_y: f32, mat: Mat, floor: Mat) -> &mut Self {
        let depth = floor_y + 1.5;
        self.bx(
            Vec3::new(w + 0.2, depth, d + 0.2),
            Vec3::new(0.0, floor_y - depth * 0.5, 0.0),
            mat,
        );
        self.piece(
            Shape::Box(Vec3::new(w - 0.1, 0.03, d - 0.1)),
            Transform::from_xyz(0.0, floor_y + 0.015, 0.0),
            floor,
            false,
        );
        self
    }

    /// Steps up to a door on the front (-Z) at x.
    pub fn front_steps(&mut self, x: f32, front_z: f32, floor_y: f32, mat: Mat) -> &mut Self {
        let n = (floor_y / 0.18).ceil().max(1.0) as usize;
        let rise = floor_y / n as f32;
        for k in 0..n {
            let top = rise * (k + 1) as f32;
            let out = (n - k) as f32 * 0.3;
            self.bx(
                Vec3::new(1.4, top, 0.3),
                Vec3::new(x, top * 0.5, front_z - out + 0.15),
                mat,
            );
        }
        self
    }

    /// A pitched roof along Z (ridge runs front to back).
    pub fn gable_roof(
        &mut self,
        w: f32,
        d: f32,
        eave_y: f32,
        pitch_deg: f32,
        overhang: f32,
        roof: Mat,
        gable: Mat,
    ) -> &mut Self {
        let pitch = pitch_deg.to_radians();
        let rise = (w * 0.5) * pitch.tan();
        let slope_len = (w * 0.5 + overhang) / pitch.cos();
        let thick = 0.12;
        for side in [-1.0f32, 1.0] {
            let cx = side * (w * 0.5 + overhang) * 0.5 - side * overhang * 0.0;
            let cy = eave_y + rise * 0.5 - overhang * pitch.tan() * 0.5 + 0.06;
            let rot = Quat::from_rotation_z(-side * pitch);
            self.piece(
                Shape::Box(Vec3::new(slope_len, thick, d + 2.0 * overhang)),
                Transform::from_xyz(cx, cy, 0.0).with_rotation(rot),
                roof,
                true,
            );
        }
        for z in [-d * 0.5, d * 0.5] {
            self.piece(
                Shape::Prism(Vec3::new(w, rise, 0.14)),
                Transform::from_xyz(0.0, eave_y, z),
                gable,
                false,
            );
        }
        // Ceiling, so the attic doesn't glare.
        self.piece(
            Shape::Box(Vec3::new(w, 0.04, d)),
            Transform::from_xyz(0.0, eave_y + 0.02, 0.0),
            Mat::Planks,
            false,
        );
        self
    }

    pub fn flat_roof(&mut self, w: f32, d: f32, y: f32, mat: Mat) -> &mut Self {
        self.bx(
            Vec3::new(w + 0.4, 0.25, d + 0.4),
            Vec3::new(0.0, y + 0.125, 0.0),
            mat,
        )
    }

    // --- furniture ---------------------------------------------------

    pub fn table(&mut self, at: Vec3, yaw: f32, loot: Option<&'static str>) -> &mut Self {
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(1.2, 0.05, 0.7)),
            Transform::from_translation(at + Vec3::Y * 0.76).with_rotation(rot),
            Mat::Timber,
            true,
        );
        for (x, z) in [(-0.55, -0.3), (0.55, -0.3), (-0.55, 0.3), (0.55, 0.3)] {
            self.piece(
                Shape::Box(Vec3::new(0.05, 0.74, 0.05)),
                Transform::from_translation(at + rot * Vec3::new(x, 0.37, z)),
                Mat::Timber,
                false,
            );
        }
        if let Some(t) = loot {
            self.loot(at + Vec3::Y * 0.82, t);
        }
        self
    }

    pub fn shelf(&mut self, at: Vec3, yaw: f32, loot: Option<&'static str>) -> &mut Self {
        let rot = Quat::from_rotation_y(yaw);
        for k in 0..4 {
            let y = 0.3 + k as f32 * 0.45;
            self.piece(
                Shape::Box(Vec3::new(1.0, 0.03, 0.35)),
                Transform::from_translation(at + Vec3::Y * y).with_rotation(rot),
                Mat::Timber,
                true,
            );
            if let Some(t) = loot {
                if k == 1 || k == 2 {
                    self.loot(at + Vec3::Y * (y + 0.05), t);
                }
            }
        }
        for x in [-0.5, 0.5] {
            self.piece(
                Shape::Box(Vec3::new(0.03, 1.8, 0.35)),
                Transform::from_translation(at + rot * Vec3::new(x, 0.9, 0.0)).with_rotation(rot),
                Mat::Timber,
                true,
            );
        }
        self
    }

    pub fn bed(&mut self, at: Vec3, yaw: f32) -> &mut Self {
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(0.9, 0.45, 1.95)),
            Transform::from_translation(at + Vec3::Y * 0.225).with_rotation(rot),
            Mat::Timber,
            true,
        );
        self.piece(
            Shape::Box(Vec3::new(0.85, 0.12, 1.85)),
            Transform::from_translation(at + Vec3::Y * 0.5).with_rotation(rot),
            Mat::Canvas,
            false,
        );
        self
    }

    pub fn counter(
        &mut self,
        at: Vec3,
        yaw: f32,
        len: f32,
        loot: Option<&'static str>,
    ) -> &mut Self {
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(len, 0.9, 0.6)),
            Transform::from_translation(at + Vec3::Y * 0.45).with_rotation(rot),
            Mat::Timber,
            true,
        );
        if let Some(t) = loot {
            self.loot(at + Vec3::Y * 0.95, t);
        }
        self
    }

    pub fn stove(&mut self, at: Vec3, yaw: f32) -> &mut Self {
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(0.7, 0.85, 0.6)),
            Transform::from_translation(at + Vec3::Y * 0.425).with_rotation(rot),
            Mat::DarkMetal,
            true,
        );
        self.piece(
            Shape::Cylinder(0.07, 1.6),
            Transform::from_translation(at + rot * Vec3::new(0.2, 1.6, 0.15)),
            Mat::DarkMetal,
            false,
        );
        self
    }

    pub fn crate_box(&mut self, at: Vec3, size: f32, loot: Option<&'static str>) -> &mut Self {
        self.bx(Vec3::splat(size), at + Vec3::Y * size * 0.5, Mat::Planks);
        if let Some(t) = loot {
            self.loot(at + Vec3::Y * (size + 0.05), t);
        }
        self
    }

    pub fn workbench(&mut self, at: Vec3, yaw: f32, loot: Option<&'static str>) -> &mut Self {
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(2.0, 0.08, 0.8)),
            Transform::from_translation(at + Vec3::Y * 0.9).with_rotation(rot),
            Mat::Timber,
            true,
        );
        self.piece(
            Shape::Box(Vec3::new(1.9, 0.86, 0.7)),
            Transform::from_translation(at + Vec3::Y * 0.43).with_rotation(rot),
            Mat::RustyMetal,
            true,
        );
        if let Some(t) = loot {
            self.loot(at + Vec3::Y * 0.98, t);
        }
        self
    }

    pub fn finish(self) -> Geo {
        self.geo
    }
}

/// Builds a prefab's geometry. `None` for prefabs drawn elsewhere.
pub fn prefab(id: &str) -> Option<Geo> {
    let mut b = Builder::default();
    match id {
        "house_plank" => house(&mut b, 8.0, 7.0, Mat::Planks, Mat::Shingles),
        "farmhouse" => house(&mut b, 10.0, 8.0, Mat::Planks, Mat::Corrugated),
        "house_brick" => {
            house(&mut b, 10.0, 8.0, Mat::Brick, Mat::Shingles);
            b.loot(Vec3::new(3.5, 0.6, 2.8), "medical");
        }
        "cabin_log" => cabin(&mut b),
        "shop" => shop(&mut b),
        "shed" => shed(&mut b),
        "barn" => barn(&mut b),
        "sawmill" => sawmill(&mut b),
        "pump_station" => pump_station(&mut b),
        "bus_depot" => hall(
            &mut b,
            30.0,
            18.0,
            7.0,
            Mat::Concrete,
            Mat::Corrugated,
            true,
        ),
        "rail_shed" => hall(
            &mut b,
            20.0,
            12.0,
            7.5,
            Mat::Corrugated,
            Mat::Corrugated,
            false,
        ),
        "mast_hut" => mast_hut(&mut b),
        "radio_mast" => radio_mast(&mut b),
        "ranger_tower" => ranger_tower(&mut b),
        "quarantine_gate" => quarantine_gate(&mut b),
        "guard_booth" => guard_booth(&mut b),
        "robot_gate" => robot_gate(&mut b),
        "boxcar" => boxcar(&mut b),
        "log_pile" => log_pile(&mut b),
        "anomaly_core" => anomaly(&mut b),
        _ => return None,
    }
    Some(b.finish())
}

fn house(b: &mut Builder, w: f32, d: f32, wall: Mat, roof: Mat) {
    let f = 0.5;
    let h = 2.7;
    let (x, z) = (w * 0.5, d * 0.5);
    b.plinth(w, d, f, Mat::Concrete, Mat::WoodFloor);
    b.shell(
        w,
        d,
        f,
        h,
        0.18,
        wall,
        [
            &[window(w * 0.2), door(w * 0.5), window(w * 0.8)],
            &[window(d * 0.3), window(d * 0.75)],
            &[window(w * 0.25), window(w * 0.75)],
            &[window(d * 0.5)],
        ],
    );
    b.front_steps(0.0, -z, f, Mat::Concrete);
    // Partition: front room / back rooms, and back rooms split left/right.
    let pz = z - d * 0.42;
    b.wall(
        Vec2::new(-x, pz),
        Vec2::new(x, pz),
        f,
        h,
        0.1,
        Mat::Plaster,
        &[gap(w * 0.25, 0.9, 2.05), gap(w * 0.75, 0.9, 2.05)],
    );
    b.wall(
        Vec2::new(0.0, pz),
        Vec2::new(0.0, z),
        f,
        h,
        0.1,
        Mat::Plaster,
        &[],
    );
    b.gable_roof(w + 0.36, d + 0.36, f + h, 32.0, 0.5, roof, wall);
    // Front room.
    b.table(Vec3::new(-x * 0.4, f, -z * 0.2), 0.0, Some("house"));
    b.shelf(
        Vec3::new(x - 0.3, f, -z * 0.4),
        std::f32::consts::FRAC_PI_2,
        Some("house"),
    );
    // Kitchen (back left).
    b.counter(
        Vec3::new(-x + 0.4, f, z - 1.4),
        std::f32::consts::FRAC_PI_2,
        1.8,
        Some("kitchen"),
    );
    b.stove(Vec3::new(-x * 0.35, f, z - 0.45), 0.0);
    b.shelf(Vec3::new(-x * 0.6, f, z - 0.3), 0.0, Some("kitchen"));
    // Bedroom (back right).
    b.bed(Vec3::new(x - 0.6, f, z - 1.2), 0.0);
    b.crate_box(Vec3::new(x * 0.35, f, z - 0.5), 0.5, Some("house"));
}

fn cabin(b: &mut Builder) {
    let (w, d, f, h) = (7.0, 6.0, 0.4, 2.5);
    b.plinth(w, d, f, Mat::Concrete, Mat::WoodFloor);
    b.shell(
        w,
        d,
        f,
        h,
        0.28,
        Mat::LogWall,
        [
            &[door(2.0), window(5.0)],
            &[window(3.0)],
            &[window(3.5)],
            &[],
        ],
    );
    b.front_steps(-1.5, -d * 0.5, f, Mat::Timber);
    b.gable_roof(
        w + 0.5,
        d + 0.5,
        f + h,
        30.0,
        0.5,
        Mat::Corrugated,
        Mat::Planks,
    );
    b.stove(Vec3::new(2.6, f, 2.3), 0.0);
    b.bed(Vec3::new(-2.8, f, 1.5), 0.0);
    b.table(Vec3::new(0.0, f, 0.5), 0.3, Some("kitchen"));
    b.shelf(Vec3::new(-1.0, f, 2.7), 0.0, Some("house"));
    b.crate_box(Vec3::new(2.8, f, -2.2), 0.6, Some("tools"));
}

fn shop(b: &mut Builder) {
    let (w, d, f, h) = (9.0, 7.0, 0.3, 3.1);
    b.plinth(w, d, f, Mat::Concrete, Mat::WoodFloor);
    b.shell(
        w,
        d,
        f,
        h,
        0.25,
        Mat::Plaster,
        [
            &[wide_window(2.0, 2.4), door(4.5), wide_window(7.0, 2.4)],
            &[],
            &[door(7.5)],
            &[window(3.5)],
        ],
    );
    b.front_steps(0.0, -d * 0.5, f, Mat::Concrete);
    b.wall(
        Vec2::new(-4.5, 1.6),
        Vec2::new(4.5, 1.6),
        f,
        h,
        0.12,
        Mat::Plaster,
        &[door(6.5)],
    );
    b.flat_roof(w, d, f + h, Mat::Concrete);
    b.counter(Vec3::new(-1.0, f, -0.4), 0.0, 3.0, Some("food_common"));
    for x in [-3.6, -2.2, 2.4, 3.8] {
        b.shelf(Vec3::new(x, f, 1.2), 0.0, Some("food_common"));
    }
    b.crate_box(Vec3::new(-3.0, f, 2.6), 0.7, Some("food_common"));
    b.crate_box(Vec3::new(-2.0, f, 2.7), 0.6, Some("tools"));
    b.shelf(Vec3::new(3.0, f, 3.0), 0.0, Some("medical"));
}

fn shed(b: &mut Builder) {
    let (w, d, f, h) = (4.0, 3.0, 0.15, 2.2);
    b.plinth(w, d, f, Mat::Timber, Mat::Planks);
    b.shell(w, d, f, h, 0.1, Mat::Planks, [&[door(1.0)], &[], &[], &[]]);
    b.gable_roof(
        w + 0.2,
        d + 0.2,
        f + h,
        20.0,
        0.2,
        Mat::Corrugated,
        Mat::Planks,
    );
    b.shelf(Vec3::new(0.8, f, 1.1), 0.0, Some("tools"));
    b.crate_box(Vec3::new(-1.3, f, 0.8), 0.5, Some("workshop"));
}

fn barn(b: &mut Builder) {
    let (w, d, f, h) = (12.0, 16.0, 0.05, 4.2);
    b.piece(
        Shape::Box(Vec3::new(w, 0.03, d)),
        Transform::from_xyz(0.0, f, 0.0),
        Mat::WoodFloor,
        false,
    );
    b.shell(
        w,
        d,
        f,
        h,
        0.15,
        Mat::Planks,
        [
            &[gap(6.0, 4.0, 3.8)],
            &[door(4.0), window(11.0)],
            &[gap(6.0, 3.0, 3.0)],
            &[window(5.0)],
        ],
    );
    b.gable_roof(
        w + 0.3,
        d + 0.3,
        f + h,
        42.0,
        0.6,
        Mat::Corrugated,
        Mat::Planks,
    );
    // Hay bales, a cart, a workbench.
    for (x, z) in [(-4.0, 4.0), (-4.0, 5.3), (-2.9, 4.6), (-4.0, 4.6)] {
        b.bx(
            Vec3::new(1.1, 0.6, 0.5),
            Vec3::new(x, f + 0.3, z),
            Mat::Canvas,
        );
    }
    b.workbench(
        Vec3::new(5.0, f, -3.0),
        std::f32::consts::FRAC_PI_2,
        Some("workshop"),
    );
    b.crate_box(Vec3::new(4.5, f, 5.5), 0.8, Some("food_common"));
    b.crate_box(Vec3::new(-4.5, f, -5.5), 0.6, Some("tools"));
}

fn sawmill(b: &mut Builder) {
    let (w, d, h) = (24.0, 12.0, 4.5);
    for x in [-11.5, -5.75, 0.0, 5.75, 11.5] {
        for z in [-5.5, 5.5] {
            b.bx(
                Vec3::new(0.25, h, 0.25),
                Vec3::new(x, h * 0.5, z),
                Mat::Timber,
            );
        }
    }
    b.bx(
        Vec3::new(w + 1.0, 0.2, d + 1.0),
        Vec3::new(0.0, h + 0.1, 0.0),
        Mat::Corrugated,
    );
    b.bx(
        Vec3::new(10.0, 0.9, 1.4),
        Vec3::new(-2.0, 0.45, 0.0),
        Mat::Timber,
    );
    b.piece(
        Shape::Cylinder(0.7, 0.04),
        Transform::from_xyz(-2.0, 1.1, 0.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Mat::DarkMetal,
        false,
    );
    b.workbench(Vec3::new(8.0, 0.0, 3.5), 0.0, Some("workshop"));
    b.workbench(Vec3::new(8.0, 0.0, -3.5), 0.0, Some("tools"));
    for k in 0..6 {
        b.piece(
            Shape::Cylinder(0.25, 5.0),
            Transform::from_xyz(
                -9.0,
                0.25 + (k / 3) as f32 * 0.45,
                -2.0 + (k % 3) as f32 * 0.52 + (k / 3) as f32 * 0.26,
            )
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            Mat::BarkSpruce,
            true,
        );
    }
}

fn pump_station(b: &mut Builder) {
    let (w, d, f, h) = (14.0, 10.0, 0.3, 4.5);
    b.plinth(w, d, f, Mat::Concrete, Mat::Concrete);
    b.shell(
        w,
        d,
        f,
        h,
        0.3,
        Mat::Brick,
        [
            &[window(3.0), door(7.0), window(11.0)],
            &[window(5.0)],
            &[window(4.0), window(10.0)],
            &[door(5.0)],
        ],
    );
    b.front_steps(0.0, -d * 0.5, f, Mat::Concrete);
    b.flat_roof(w, d, f + h, Mat::Concrete);
    // Two big pumps with pipes, and an office corner.
    for x in [-3.5, 1.5] {
        b.piece(
            Shape::Cylinder(0.9, 1.8),
            Transform::from_xyz(x, f + 0.9, 1.5),
            Mat::PaintedMetal,
            true,
        );
        b.piece(
            Shape::Cylinder(0.18, 6.0),
            Transform::from_xyz(x, f + 0.6, 1.5)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Mat::RustyMetal,
            false,
        );
    }
    b.wall(
        Vec2::new(3.5, -5.0),
        Vec2::new(3.5, 0.0),
        f,
        h,
        0.12,
        Mat::Plaster,
        &[door(3.5)],
    );
    b.table(Vec3::new(5.5, f, -3.0), 0.0, Some("tools"));
    b.shelf(
        Vec3::new(6.6, f, -1.0),
        std::f32::consts::FRAC_PI_2,
        Some("medical"),
    );
    b.workbench(Vec3::new(-5.5, f, -3.5), 0.0, Some("workshop"));
}

fn hall(b: &mut Builder, w: f32, d: f32, h: f32, wall: Mat, roof: Mat, office: bool) {
    let f = 0.1;
    b.piece(
        Shape::Box(Vec3::new(w, 0.04, d)),
        Transform::from_xyz(0.0, f, 0.0),
        Mat::Concrete,
        false,
    );
    b.bx(
        Vec3::new(w + 0.4, 1.0, d + 0.4),
        Vec3::new(0.0, f - 0.52, 0.0),
        Mat::Concrete,
    );
    b.shell(
        w,
        d,
        f,
        h,
        0.25,
        wall,
        [
            &[gap(w * 0.3, 5.0, 4.8), gap(w * 0.7, 5.0, 4.8)],
            &[wide_window(d * 0.5, 3.0)],
            &[door(w * 0.15), wide_window(w * 0.5, 4.0)],
            &[wide_window(d * 0.5, 3.0)],
        ],
    );
    b.gable_roof(w + 0.5, d + 0.5, f + h, 14.0, 0.4, roof, wall);
    b.workbench(Vec3::new(-w * 0.35, f, d * 0.35), 0.0, Some("workshop"));
    b.workbench(Vec3::new(w * 0.35, f, d * 0.35), 0.0, Some("workshop"));
    b.crate_box(Vec3::new(w * 0.4, f, -d * 0.3), 0.9, Some("tools"));
    b.crate_box(
        Vec3::new(w * 0.4 - 1.0, f, -d * 0.3),
        0.7,
        Some("ammo_common"),
    );
    if office {
        // A wrecked bus in bay one, an office in the corner.
        b.bx(
            Vec3::new(2.5, 3.0, 10.0),
            Vec3::new(-w * 0.2, f + 1.6, 0.0),
            Mat::PaintedMetal,
        );
        b.wall(
            Vec2::new(w * 0.5 - 6.0, d * 0.5),
            Vec2::new(w * 0.5 - 6.0, d * 0.5 - 5.0),
            f,
            2.8,
            0.12,
            Mat::Plaster,
            &[door(3.5)],
        );
        b.wall(
            Vec2::new(w * 0.5 - 6.0, d * 0.5 - 5.0),
            Vec2::new(w * 0.5, d * 0.5 - 5.0),
            f,
            2.8,
            0.12,
            Mat::Plaster,
            &[window(3.0)],
        );
        b.table(
            Vec3::new(w * 0.5 - 3.0, f, d * 0.5 - 2.5),
            0.0,
            Some("house"),
        );
        b.shelf(
            Vec3::new(w * 0.5 - 1.0, f, d * 0.5 - 0.6),
            0.0,
            Some("gadgets"),
        );
    }
}

fn mast_hut(b: &mut Builder) {
    let (w, d, f, h) = (5.0, 4.0, 0.2, 2.6);
    b.plinth(w, d, f, Mat::Concrete, Mat::Concrete);
    b.shell(
        w,
        d,
        f,
        h,
        0.25,
        Mat::Concrete,
        [&[door(1.2)], &[], &[window(2.5)], &[]],
    );
    b.flat_roof(w, d, f + h, Mat::Concrete);
    b.counter(Vec3::new(1.2, f, 1.4), 0.0, 2.2, Some("gadgets"));
    b.shelf(Vec3::new(-1.8, f, 1.5), 0.0, Some("tools"));
}

fn radio_mast(b: &mut Builder) {
    let h = 45.0;
    let r = 0.9;
    for (x, z) in [(-r, -r), (r, -r), (r, r), (-r, r)] {
        b.bx(
            Vec3::new(0.12, h, 0.12),
            Vec3::new(x, h * 0.5, z),
            Mat::RustyMetal,
        );
    }
    let mut y = 1.5;
    while y < h {
        for (a, bb) in [
            ((-r, -r), (r, -r)),
            ((r, -r), (r, r)),
            ((r, r), (-r, r)),
            ((-r, r), (-r, -r)),
        ] {
            let mid = Vec3::new((a.0 + bb.0) * 0.5, y, (a.1 + bb.1) * 0.5);
            let len = 2.0 * r;
            let yaw = if a.1 == bb.1 {
                0.0
            } else {
                std::f32::consts::FRAC_PI_2
            };
            b.piece(
                Shape::Box(Vec3::new(len, 0.06, 0.06)),
                Transform::from_translation(mid).with_rotation(Quat::from_rotation_y(yaw)),
                Mat::RustyMetal,
                false,
            );
        }
        y += 3.0;
    }
    b.geo.lights.push(Vec3::new(0.0, h + 0.3, 0.0));
}

fn ranger_tower(b: &mut Builder) {
    let top = 9.0;
    for (x, z) in [(-1.8, -1.8), (1.8, -1.8), (1.8, 1.8), (-1.8, 1.8)] {
        b.bx(
            Vec3::new(0.25, top, 0.25),
            Vec3::new(x, top * 0.5, z),
            Mat::Timber,
        );
    }
    // Switchback stairs on the outside: three flights.
    let flights = 3;
    let per = top / flights as f32;
    for k in 0..flights {
        let y0 = per * k as f32;
        let steps = (per / 0.2).ceil() as usize;
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        for s in 0..steps {
            let y = y0 + (s + 1) as f32 * per / steps as f32;
            let z = side * (-1.6 + s as f32 * 3.2 / steps as f32);
            b.bx(
                Vec3::new(0.9, 0.06, 0.32),
                Vec3::new(2.5, y - 0.03, z),
                Mat::Planks,
            );
        }
        b.bx(
            Vec3::new(0.9, 0.08, 1.0),
            Vec3::new(2.5, y0 + per - 0.04, side * 1.9),
            Mat::Planks,
        );
    }
    b.bx(
        Vec3::new(4.4, 0.15, 4.4),
        Vec3::new(0.0, top, 0.0),
        Mat::Planks,
    );
    b.shell(
        4.0,
        4.0,
        top + 0.08,
        2.4,
        0.08,
        Mat::Planks,
        [
            &[wide_window(2.0, 3.0)],
            &[door(3.2), wide_window(1.2, 1.4)],
            &[wide_window(2.0, 3.0)],
            &[wide_window(2.0, 3.0)],
        ],
    );
    b.gable_roof(4.3, 4.3, top + 2.5, 25.0, 0.3, Mat::Corrugated, Mat::Planks);
    b.table(Vec3::new(0.0, top + 0.08, 0.6), 0.0, Some("gadgets"));
}

fn quarantine_gate(b: &mut Builder) {
    // Concrete blocks either side of the road and the barrier arm, raised.
    for x in [-5.2, 5.2] {
        b.bx(
            Vec3::new(1.4, 1.2, 1.4),
            Vec3::new(x, 0.6, 0.0),
            Mat::Concrete,
        );
        b.bx(
            Vec3::new(0.25, 3.5, 0.25),
            Vec3::new(x.signum() * 4.3, 1.75, 0.0),
            Mat::PaintedMetal,
        );
    }
    b.piece(
        Shape::Box(Vec3::new(0.12, 6.0, 0.12)),
        Transform::from_xyz(-4.0, 3.5, 0.0).with_rotation(Quat::from_rotation_z(0.15)),
        Mat::PaintedMetal,
        false,
    );
}

/// The guard booth by the quarantine gate (its own placement, so it sits
/// on its own level pad rather than half buried in the gate's slope).
fn guard_booth(b: &mut Builder) {
    // On a plinth with steps up to the door (front, -Z): the road's
    // grading leaves the ground here sloping.
    let f = 0.6;
    b.plinth(2.2, 2.2, f, Mat::Concrete, Mat::WoodFloor);
    b.shell(
        2.2,
        2.2,
        f,
        2.4,
        0.08,
        Mat::PaintedMetal,
        [&[door(1.1)], &[window(1.1)], &[window(1.1)], &[window(1.1)]],
    );
    b.front_steps(0.0, -1.1, f, Mat::Concrete);
    b.bx(
        Vec3::new(2.6, 0.15, 2.6),
        Vec3::new(0.0, f + 2.5, 0.0),
        Mat::Corrugated,
    );
    b.counter(Vec3::new(0.0, f, 0.6), 0.0, 1.6, Some("ammo_common"));
}

fn robot_gate(b: &mut Builder) {
    // A sleek white arch: two piers and a lintel, glowing seams.
    for x in [-8.0, 8.0] {
        b.bx(
            Vec3::new(1.6, 12.0, 4.0),
            Vec3::new(x, 6.0, 0.0),
            Mat::RobotShell,
        );
    }
    b.bx(
        Vec3::new(17.6, 1.6, 4.0),
        Vec3::new(0.0, 12.8, 0.0),
        Mat::RobotShell,
    );
    b.geo.lights.push(Vec3::new(0.0, 11.5, -2.2));
}

fn boxcar(b: &mut Builder) {
    let (w, d, f, h) = (3.0, 14.0, 1.2, 2.6);
    b.bx(Vec3::new(w, 0.15, d), Vec3::new(0.0, f, 0.0), Mat::Planks);
    b.shell(
        w,
        d,
        f,
        h,
        0.06,
        Mat::RustyMetal,
        [&[], &[gap(d * 0.5, 2.0, 2.3)], &[], &[]],
    );
    b.bx(
        Vec3::new(w + 0.1, 0.1, d + 0.1),
        Vec3::new(0.0, f + h + 0.05, 0.0),
        Mat::RustyMetal,
    );
    for z in [-5.0, -4.0, 4.0, 5.0] {
        for x in [-0.75, 0.75] {
            b.piece(
                Shape::Cylinder(0.45, 0.12),
                Transform::from_xyz(x, 0.45, z)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                Mat::DarkMetal,
                false,
            );
        }
    }
    b.crate_box(Vec3::new(0.6, f + 0.08, 2.0), 0.8, Some("workshop"));
    b.crate_box(Vec3::new(-0.6, f + 0.08, -3.0), 0.7, Some("food_common"));
}

fn log_pile(b: &mut Builder) {
    for k in 0..10 {
        let row = k / 4;
        let col = k % 4;
        b.piece(
            Shape::Cylinder(0.22, 7.0),
            Transform::from_xyz(
                -0.7 + col as f32 * 0.45 + row as f32 * 0.22,
                0.22 + row as f32 * 0.4,
                0.0,
            )
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Mat::BarkSpruce,
            true,
        );
    }
}

fn anomaly(b: &mut Builder) {
    // Stones that hang in the air around a hollow, slowly.
    for k in 0..9 {
        let a = k as f32 / 9.0 * std::f32::consts::TAU;
        let r = 3.0 + (k % 3) as f32;
        b.piece(
            Shape::Box(Vec3::new(0.6 + (k % 2) as f32 * 0.4, 0.5, 0.7)),
            Transform::from_xyz(a.cos() * r, 1.5 + (k as f32 * 0.7).sin() * 1.2, a.sin() * r)
                .with_rotation(Quat::from_euler(EulerRot::XYZ, a, a * 0.5, 0.3)),
            Mat::Concrete,
            false,
        );
    }
    b.loot(Vec3::new(0.0, 0.3, 0.0), "curio");
}

/// A placed building's logic root.
#[derive(Component)]
pub struct Building {
    pub prefab: String,
}

/// A door: swings between closed and open.
#[derive(Component, Debug)]
pub struct Door {
    pub open: bool,
    pub angle: f32,
}

/// Loot spots from every building, filled when the raid starts.
#[derive(Resource, Default, Debug)]
pub struct BuildingLoot(pub Vec<LootSpot>);

/// Building pose in the world.
pub fn placement_transform(zone: &Zone, p: &extraction_core::map::Placement) -> Transform {
    // Sit on the lowest corner of a rough footprint, so nothing floats.
    let size = super::structures::footprint(&p.prefab);
    let rot = Quat::from_rotation_y(-p.rot.to_radians());
    let mut y = f32::INFINITY;
    for (sx, sz) in [
        (-0.5, -0.5),
        (0.5, -0.5),
        (0.5, 0.5),
        (-0.5, 0.5),
        (0.0, 0.0),
    ] {
        let c = rot * Vec3::new(sx * size.x, 0.0, sz * size.z);
        y = y.min(zone.ground(p.at[0] + c.x, p.at[1] + c.z));
    }
    let y = if p.absolute { p.y } else { y + p.y };
    Transform::from_xyz(p.at[0], y, p.at[1]).with_rotation(rot)
}

/// Logic: colliders, doors and loot spots for every generated prefab.
pub fn spawn_buildings(mut commands: Commands, zone: Res<Zone>) {
    let mut loot = BuildingLoot::default();
    for p in &zone.map.placements {
        let Some(geo) = prefab(&p.prefab) else {
            continue;
        };
        let t = placement_transform(&zone, p);
        let root = commands
            .spawn((
                Name::new(format!("Building {}", p.prefab)),
                Building {
                    prefab: p.prefab.clone(),
                },
                t,
                Visibility::default(),
            ))
            .id();
        let mut colliders = Vec::new();
        for piece in geo.pieces.iter().filter(|q| q.collide) {
            let c = match piece.shape {
                Shape::Box(s) => Collider::cuboid(s.x, s.y, s.z),
                Shape::Prism(s) => Collider::cuboid(s.x, s.y * 0.5, s.z),
                Shape::Cylinder(r, h) => Collider::cylinder(r, h),
            };
            colliders.push((piece.at.translation, piece.at.rotation, c));
        }
        if !colliders.is_empty() {
            commands.entity(root).insert((
                RigidBody::Static,
                Collider::compound(colliders),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
            ));
        }
        for d in &geo.doors {
            commands
                .spawn((
                    Name::new("Door"),
                    Door {
                        open: false,
                        angle: 0.0,
                    },
                    crate::raid::truck::Pressable::Door,
                    crate::interact::Grabbable {
                        points: vec![crate::interact::GrabPoint::free()],
                    },
                    DoorLeaf(*d),
                    Transform::from_translation(d.hinge)
                        .with_rotation(Quat::from_rotation_y(d.yaw)),
                    Visibility::default(),
                    RigidBody::Kinematic,
                    ChildOf(root),
                ))
                .with_child((
                    Collider::cuboid(d.width, d.height, 0.05),
                    CollisionLayers::new(Layer::World, LayerMask::ALL),
                    Transform::from_xyz(d.width * 0.5, d.height * 0.5 + 0.01, 0.0),
                ));
        }
        for s in &geo.loot {
            loot.0.push(LootSpot {
                at: t.transform_point(s.at),
                table: s.table,
            });
        }
    }
    info!("{} building loot spots", loot.0.len());
    commands.insert_resource(loot);
}

/// The door's definition, kept for visuals.
#[derive(Component, Clone, Copy, Debug)]
pub struct DoorLeaf(pub DoorDef);

/// Pressing a door toggles it; it swings over a second.
pub fn swing_doors(
    mut events: MessageReader<crate::interact::InteractEvent>,
    mut doors: Query<(&mut Door, &mut Transform, &DoorLeaf)>,
    time: Res<Time>,
) {
    for ev in events.read() {
        if let crate::interact::InteractEvent::Pressed { item, .. } = ev {
            if let Ok((mut d, _, _)) = doors.get_mut(*item) {
                d.open = !d.open;
            }
        }
    }
    for (mut d, mut t, leaf) in &mut doors {
        let target = if d.open { 1.0 } else { 0.0 };
        d.angle += (target - d.angle).clamp(-time.delta_secs() * 1.6, time.delta_secs() * 1.6);
        t.rotation = Quat::from_rotation_y(leaf.0.yaw + d.angle * 1.75);
    }
}

/// Client: meshes for buildings and doors.
pub fn building_visuals(
    mut commands: Commands,
    buildings: Query<(Entity, &Building), Added<Building>>,
    doors: Query<(Entity, &DoorLeaf), Added<DoorLeaf>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<crate::render::materials::MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for (e, b) in &buildings {
        let Some(geo) = prefab(&b.prefab) else {
            continue;
        };
        // One mesh per material per building: a house is a handful of
        // draws, not fifty.
        let mut merged: Vec<(Mat, Mesh)> = Vec::new();
        for piece in &geo.pieces {
            let mut mesh = match piece.shape {
                Shape::Box(s) => crate::render::materials::metric_box(s),
                Shape::Prism(s) => crate::render::materials::gable_prism(s.x, s.y, s.z),
                Shape::Cylinder(r, h) => Cylinder::new(r, h).mesh().resolution(16).build(),
            };
            let _ = mesh.generate_tangents();
            let mesh = mesh.transformed_by(piece.at);
            let fits = match merged.iter_mut().find(|(m, _)| *m == piece.mat) {
                Some((_, into)) => into.merge(&mesh).is_ok(),
                None => {
                    merged.push((piece.mat, mesh.clone()));
                    true
                }
            };
            if !fits {
                let mat = library.get(piece.mat, &server, &mut materials);
                commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat), ChildOf(e)));
            }
        }
        for (m, mesh) in merged {
            let mat = library.get(m, &server, &mut materials);
            commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat), ChildOf(e)));
        }
        for l in &geo.lights {
            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.3, 0.25),
                    intensity: 40_000.0,
                    range: 25.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(*l),
                ChildOf(e),
            ));
        }
    }
    for (e, d) in &doors {
        let leaf = d.0;
        let mat = library.get(leaf.mat, &server, &mut materials);
        commands.spawn((
            Mesh3d(meshes.add(crate::render::materials::metric_box(Vec3::new(
                leaf.width,
                leaf.height,
                0.05,
            )))),
            MeshMaterial3d(mat),
            Transform::from_xyz(leaf.width * 0.5, leaf.height * 0.5 + 0.01, 0.0),
            ChildOf(e),
        ));
        let knob = library.get(Mat::DarkMetal, &server, &mut materials);
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.035))),
            MeshMaterial3d(knob),
            Transform::from_xyz(leaf.width - 0.1, 1.0, -0.05),
            ChildOf(e),
        ));
    }
}
