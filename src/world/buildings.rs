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

/// A modelled prop (`art/props`, see `render::world_props`) drawn in place
/// of some of a recipe's pieces. Their colliders, surfaces and loot spots
/// stay as they are; only their boxes' meshes give way, and only once every
/// model here has loaded. An empty `pieces` range adds the models as
/// dressing (a window's carved surround) with nothing to replace.
#[derive(Clone, Debug)]
pub struct PropDef {
    /// Model names under `models/props/`, posed in building space.
    pub models: Vec<(&'static str, Transform)>,
    pub pieces: std::ops::Range<usize>,
    /// Furniture: drawn without shadows (the sun doesn't reach it) and
    /// only near, its boxes standing in further off.
    pub inside: bool,
    /// Outside, whether it casts a shadow (a gate's blocks do; windows,
    /// in their wall's shadow already, don't).
    pub shadow: bool,
    /// A hero landmark's lighter models: one drawn past [`LANDMARK_MID`]
    /// in place of `models`, one in the building's far stand-in.
    pub lods: Option<[&'static str; 2]>,
}

/// Where a hero landmark's full model gives way to its middle one.
pub const LANDMARK_MID: f32 = 60.0;

#[derive(Default, Clone, Debug)]
pub struct Geo {
    pub pieces: Vec<Piece>,
    pub loot: Vec<LootSpot>,
    pub doors: Vec<DoorDef>,
    /// Warm interior lamps (rare; most of the Zone is dark).
    pub lights: Vec<Vec3>,
    pub props: Vec<PropDef>,
    /// Outside doors and doorways: what must be kept clear in front of
    /// them (mirrored in `map::prefab_entrances` for the map lint).
    pub entrances: Vec<extraction_core::map::Entrance>,
    /// Ladders climbed with the hands (`interact::hand_climb`).
    pub ladders: Vec<crate::interact::hand_climb::Ladder>,
    /// Where the sky comes in through what's left of a roof (a ruin's
    /// hole): each opening's centre at the eaves, building-local, and its
    /// half extents along x and z. For the sun's shafts (`shafts`).
    pub skylights: Vec<(Vec3, Vec2)>,
    /// Things a hand or a round can take off, each its own entity (not in
    /// the building's collider, nor drawn by it): Destruction spawns,
    /// draws and breaks them.
    pub breakables: Vec<Breakable>,
    /// The floor walked on inside, building-local y, where it isn't a
    /// door's sill (a drowned house's silt: it has no doors left).
    pub floor: Option<f32>,
}

/// What a breakable thing is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakKind {
    /// A plank window shutter, hinged open against the wall.
    Shutter,
    /// A garden fence's plank panel between its posts.
    FencePanel,
    /// A wrecked bus's folding door.
    BusDoor,
    /// A wrecked bus's side skirt panel.
    BusPanel,
    /// A car wreck's door, bonnet, boot lid or wheel (`destruction::strip`).
    /// Their models are authored in the car's own space, so each slots
    /// into the body where it came from.
    CarDoor,
    CarBonnet,
    CarBoot,
    CarWheel,
}

impl BreakKind {
    /// A car's part: its model is in the car's space, not its own.
    pub fn in_car_space(self) -> bool {
        matches!(
            self,
            BreakKind::CarDoor | BreakKind::CarBonnet | BreakKind::CarBoot | BreakKind::CarWheel
        )
    }
}

/// A breakable thing on a building (`Geo::breakables`).
#[derive(Clone, Debug)]
pub struct Breakable {
    pub kind: BreakKind,
    /// Building-local: its middle, its outer face to local -Z.
    pub at: Transform,
    /// Its box, for its collider.
    pub size: Vec3,
    /// Its model, or none: a box of `size` in `mat`.
    pub model: Option<&'static str>,
    pub mat: Mat,
    /// What it drops when it's prised off.
    pub yields: Option<&'static str>,
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
    /// A hole knocked through a ruin's wall: no frame, no steps.
    Breach,
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
pub fn breach(centre: f32, width: f32, bottom: f32, top: f32) -> Opening {
    Opening {
        centre,
        width,
        bottom,
        top,
        kind: OpeningKind::Breach,
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
    /// Building a ruin (`house_ruin`): windows broken, doors gone, the
    /// roof holed or burnt to its rafters.
    pub ruin: Option<Ruin>,
    /// A damaged ruin's windows all broken out, not every other one (a
    /// looted shop's front).
    pub glass_out: bool,
    /// Set while `shell` builds outside walls: their local -Z faces out.
    exterior: bool,
    /// How far above the ground in front of a door the floor can stand
    /// (the building is lifted that much on a slope: `map::prefab_lift`).
    pub drop: f32,
}

/// A ruined house's state (its prefab id's suffix, `map::RUIN_STATES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ruin {
    /// Shelled or fallen in: a hole in the roof, a breach in a wall, a
    /// beam down across a room, most of the glass out.
    Damaged,
    /// Burnt out: charred to its rafters, no doors or glass, ash.
    Burnt,
    /// Under a reservoir for sixty years, the water now down: the roof
    /// and floors gone, the walls standing, silted to the sills, a tide
    /// line round them.
    Drowned,
}

/// One of `n` variants for a prop at `at`, so a room's two crates differ
/// but a rebuild picks the same.
fn variant(at: Vec3, n: usize) -> usize {
    ((at.x * 7.31 + at.z * 3.17 + at.y * 1.3).sin().abs() * 997.0) as usize % n
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

    /// The pieces pushed since `from` are drawn by these models once they
    /// load (see [`PropDef`]).
    pub fn dress(&mut self, from: usize, models: Vec<(&'static str, Transform)>) -> &mut Self {
        let to = self.geo.pieces.len();
        self.geo.props.push(PropDef {
            models,
            pieces: from..to,
            inside: true,
            shadow: false,
            lods: None,
        });
        self
    }

    /// As `dress`, for the building's own fabric (windows): drawn as far
    /// as the building is, casting shadows.
    pub fn dress_outside(
        &mut self,
        from: usize,
        models: Vec<(&'static str, Transform)>,
    ) -> &mut Self {
        self.dress(from, models);
        if let Some(d) = self.geo.props.last_mut() {
            d.inside = false;
        }
        self
    }

    /// As `dress`, for structures standing in the open (a gate's blocks):
    /// drawn as far as the building is, casting shadows.
    pub fn dress_structure(
        &mut self,
        from: usize,
        models: Vec<(&'static str, Transform)>,
    ) -> &mut Self {
        self.dress_outside(from, models);
        if let Some(d) = self.geo.props.last_mut() {
            d.shadow = true;
        }
        self
    }

    /// As `dress_structure`, for a hero landmark: `name`'s model near,
    /// `name_lod1` in the middle distance, `name_lod2` far off.
    pub fn dress_landmark(
        &mut self,
        from: usize,
        name: &'static str,
        at: Transform,
        lods: [&'static str; 2],
    ) {
        self.dress_structure(from, vec![(name, at)]);
        if let Some(d) = self.geo.props.last_mut() {
            d.lods = Some(lods);
        }
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
                // A ruin's windows: burnt out, or (damaged) every other
                // one's glass broken out of its bar.
                OpeningKind::Window
                    if matches!(self.ruin, Some(Ruin::Burnt | Ruin::Drowned))
                        || (self.ruin.is_some() && (self.glass_out || variant(mid, 2) == 1)) =>
                {
                    if self.ruin == Some(Ruin::Damaged) {
                        self.piece(
                            Shape::Box(Vec3::new(o.width - 0.02, 0.03, 0.04)),
                            Transform::from_translation(mid + Vec3::Y * ((o.bottom + o.top) * 0.5))
                                .with_rotation(rot),
                            frame,
                            false,
                        );
                    }
                    self.piece(
                        Shape::Box(Vec3::new(o.width + 0.1, 0.05, thick + 0.08)),
                        Transform::from_translation(mid + Vec3::Y * o.bottom).with_rotation(rot),
                        frame,
                        false,
                    );
                }
                OpeningKind::Window => {
                    let from = self.geo.pieces.len();
                    // The glazing bar stands proud of the glass both sides.
                    self.piece(
                        Shape::Box(Vec3::new(o.width - 0.02, 0.03, 0.04)),
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
                    // Modelled: a row of sashes stretched to fill the
                    // opening, and on a timber house's outside a carved
                    // surround.
                    let n = (o.width / 1.0).round().max(1.0);
                    let (uw, h) = (o.width / n, o.top - o.bottom);
                    let centre_y = y0 + (o.bottom + o.top) * 0.5;
                    let sashes = (0..n as usize)
                        .map(|i| {
                            let s = o.centre - o.width * 0.5 + uw * (i as f32 + 0.5);
                            (
                                "window_sash",
                                Transform::from_translation(at(s, centre_y))
                                    .with_rotation(rot)
                                    .with_scale(Vec3::new(uw, h / 1.1, 1.0)),
                            )
                        })
                        .collect();
                    self.dress_outside(from, sashes);
                    if self.exterior && matches!(mat, Mat::Planks | Mat::LogWall) {
                        let face = at(o.centre, centre_y) + rot * Vec3::new(0.0, 0.0, -thick * 0.5);
                        let casing = Transform::from_translation(face)
                            .with_rotation(rot)
                            .with_scale(Vec3::new(o.width, h / 1.1, 1.0));
                        let here = self.geo.pieces.len();
                        self.dress_outside(here, vec![("window_casing", casing)]);
                        // Its shutters, hinged open flat against the wall
                        // either side, clear of the casing (`shutter`:
                        // hinged on its +X edge; the right one turned
                        // over to hinge on its left).
                        let sh = h;
                        for side in [-1.0f32, 1.0] {
                            let s = o.centre + side * (o.width * 0.5 + 0.13 + 0.25);
                            if s < 0.4 || s > len - 0.4 {
                                continue;
                            }
                            let c =
                                at(s, centre_y) + rot * Vec3::new(0.0, 0.0, -thick * 0.5 - 0.02);
                            let turn = if side > 0.0 {
                                Quat::from_rotation_z(std::f32::consts::PI)
                            } else {
                                Quat::IDENTITY
                            };
                            self.geo.breakables.push(Breakable {
                                kind: BreakKind::Shutter,
                                at: Transform::from_translation(c)
                                    .with_rotation(rot * turn)
                                    .with_scale(Vec3::new(1.0, sh / 1.1, 1.0)),
                                size: Vec3::new(0.5, sh, 0.03),
                                model: Some("shutter"),
                                mat: Mat::Timber,
                                yields: None,
                            });
                        }
                    }
                    self.piece(
                        Shape::Box(Vec3::new(o.width + 0.1, 0.05, thick + 0.08)),
                        Transform::from_translation(mid + Vec3::Y * o.bottom).with_rotation(rot),
                        frame,
                        false,
                    );
                }
                OpeningKind::Door => {
                    // Jambs standing a little proud of the wall either
                    // side, so the doorway reads from inside and out (and
                    // half a centimetre into it: flush with the wall's end
                    // they'd fight it).
                    for s in [s0 - 0.025, s1 + 0.025] {
                        self.piece(
                            Shape::Box(Vec3::new(0.06, o.top - o.bottom - 0.005, thick + 0.04)),
                            Transform::from_translation(at(
                                s,
                                y0 + (o.bottom + o.top + 0.005) * 0.5,
                            ))
                            .with_rotation(rot),
                            frame,
                            false,
                        );
                    }
                    let hinge = at(s0 + 0.02, y0);
                    // Burnt or drowned, the door's gone with the rest.
                    if !matches!(self.ruin, Some(Ruin::Burnt | Ruin::Drowned)) {
                        self.geo.doors.push(DoorDef {
                            hinge,
                            width: o.width - 0.04,
                            height: o.top - o.bottom - 0.02,
                            yaw,
                            // Plank doors in timber walls, steel elsewhere.
                            mat: if matches!(mat, Mat::Planks | Mat::LogWall) {
                                Mat::Planks
                            } else {
                                Mat::PaintedMetal
                            },
                        });
                    }
                }
                OpeningKind::Gap | OpeningKind::Breach => {}
            }
            if !matches!(o.kind, OpeningKind::Gap | OpeningKind::Breach) {
                // The lintel's underside a centimetre below the wall's over
                // the opening (in the same plane they fight). Under a
                // carved casing it's slimmer, behind the casing's boards
                // and below its head (not in their planes).
                let cased = o.kind == OpeningKind::Window
                    && self.exterior
                    && matches!(mat, Mat::Planks | Mat::LogWall);
                let (lh, ld) = if cased { (0.065, 0.03) } else { (0.08, 0.04) };
                self.piece(
                    Shape::Box(Vec3::new(o.width + 0.12, lh, thick + ld)),
                    Transform::from_translation(mid + Vec3::Y * (o.top - 0.01 + lh * 0.5))
                        .with_rotation(rot),
                    frame,
                    false,
                );
            }
            cursor = s1;
        }
        seg(self, cursor, len, 0.0, height);
        if self.exterior {
            self.exterior_trim(a, dir, rot, len, y0, height, thick, mat, &ops);
            let steps = if mat == Mat::LogWall {
                Mat::Timber
            } else {
                Mat::Concrete
            };
            for o in ops
                .iter()
                .filter(|o| matches!(o.kind, OpeningKind::Door | OpeningKind::Gap))
            {
                let reach = self.door_steps(a, dir, rot, o.centre, o.width, y0, steps);
                // From the wall's face: its steps, then a metre to stand in.
                let out = rot * Vec3::NEG_Z;
                let c = a + dir * o.centre;
                let face = Vec2::new(c.x + out.x * thick * 0.5, c.y + out.z * thick * 0.5);
                let r = |v: f32| (v * 100.0).round() / 100.0 + 0.0;
                self.geo.entrances.push(extraction_core::map::Entrance {
                    at: [r(face.x), r(face.y)],
                    out: [r(out.x), r(out.z)],
                    width: r(o.width),
                    clear: r((reach - thick * 0.5).max(0.0) + 1.0),
                });
            }
        }
        self
    }

    /// Visual trim on an outside wall (see `shell`), so a house reads as
    /// built rather than cut from boxes: a filler for the corner the wall
    /// ends at (walls meet on their centre lines, which leaves a notch
    /// outside), then by material corner boards, a drip board and door
    /// surrounds (timber), or a cornice band and a rendered base
    /// (masonry). A strip runs past the corner it ends at by its own depth
    /// and starts at the corner's face where it begins, so the two that
    /// meet at a corner never share a face.
    #[allow(clippy::too_many_arguments)]
    fn exterior_trim(
        &mut self,
        a: Vec2,
        dir: Vec2,
        rot: Quat,
        len: f32,
        y0: f32,
        h: f32,
        thick: f32,
        mat: Mat,
        ops: &[Opening],
    ) {
        let face = thick * 0.5;
        // A box from s0 to s1 along the wall, ya to yb up, standing `out`
        // from the centre line to `out + depth`.
        let strip = |b: &mut Self, s0: f32, s1: f32, ya: f32, yb: f32, out: f32, depth: f32, m| {
            if s1 - s0 < 0.01 {
                return;
            }
            let c = a + dir * ((s0 + s1) * 0.5);
            let centre = Vec3::new(c.x, y0 + (ya + yb) * 0.5, c.y)
                + rot * Vec3::new(0.0, 0.0, -(out + depth * 0.5));
            b.piece(
                Shape::Box(Vec3::new(s1 - s0, yb - ya, depth)),
                Transform::from_translation(centre).with_rotation(rot),
                m,
                false,
            );
        };
        // A ruin's collapsed stretches: breaches to the wall's top. Trim
        // there would hang in the air; a corner's cap stands only as high
        // as the stub left.
        let gone: Vec<(f32, f32, f32)> = ops
            .iter()
            .filter(|o| o.kind == OpeningKind::Breach && o.top >= h - 0.01)
            .map(|o| (o.centre - o.width * 0.5, o.centre + o.width * 0.5, o.bottom))
            .collect();
        let stub = |at: f32| {
            gone.iter()
                .find(|(s0, s1, _)| at >= s0 - 0.02 && at <= s1 + 0.02)
                .map(|g| g.2)
        };
        strip(
            self,
            len,
            len + face,
            0.0,
            stub(len).unwrap_or(h),
            0.0,
            face,
            mat,
        );
        // Runs of a strip along the wall's top, broken where it's gone.
        let top_runs = |s0: f32, s1: f32| {
            let mut out = Vec::new();
            let mut s = s0;
            for &(g0, g1, _) in &gone {
                out.push((s, g0.max(s0)));
                s = g1.min(s1);
            }
            out.push((s, s1));
            out
        };
        let doors: Vec<(f32, f32, OpeningKind, f32)> = ops
            .iter()
            .filter(|o| o.kind != OpeningKind::Window)
            .map(|o| {
                (
                    (o.centre - o.width * 0.5).max(0.0),
                    (o.centre + o.width * 0.5).min(len),
                    o.kind,
                    o.top,
                )
            })
            .collect();
        // Runs of a strip along the wall's foot, broken at doorways by
        // `clear` either side.
        let runs = |end: f32, clear: f32| {
            let mut out = Vec::new();
            let mut s = -face;
            for &(s0, s1, _, _) in &doors {
                out.push((s, s0 - clear));
                s = s1 + clear;
            }
            out.push((s, end));
            out
        };
        match mat {
            Mat::Planks | Mat::LogWall => {
                let timber = Mat::Timber;
                if mat == Mat::Planks {
                    let e = len + face + 0.03;
                    if stub(0.0).is_none() {
                        strip(self, -face, -face + 0.14, -0.05, h, face, 0.03, timber);
                    }
                    if stub(len).is_none() {
                        strip(self, e - 0.14, e, -0.05, h, face, 0.03, timber);
                    }
                    for (s0, s1) in runs(len + face + 0.045, 0.13) {
                        strip(self, s0, s1, -0.04, 0.12, face, 0.045, timber);
                    }
                }
                for &(s0, s1, kind, top) in &doors {
                    if kind != OpeningKind::Door {
                        continue;
                    }
                    strip(
                        self,
                        s0 - 0.13,
                        s0 - 0.03,
                        0.0,
                        top + 0.065,
                        face,
                        0.035,
                        timber,
                    );
                    strip(
                        self,
                        s1 + 0.03,
                        s1 + 0.13,
                        0.0,
                        top + 0.065,
                        face,
                        0.035,
                        timber,
                    );
                    strip(
                        self,
                        s0 - 0.16,
                        s1 + 0.16,
                        top + 0.065,
                        top + 0.2,
                        face,
                        0.045,
                        timber,
                    );
                }
            }
            Mat::Brick | Mat::Plaster => {
                for (s0, s1) in top_runs(-face, len + face + 0.04) {
                    strip(self, s0, s1, h - 0.18, h - 0.02, face, 0.04, mat);
                }
                // Its foot just off the steeple's base and the like,
                // modelled to the footing's 5 cm.
                for (s0, s1) in runs(len + face + 0.025, 0.06) {
                    strip(self, s0, s1, -0.056, 0.3, face, 0.025, Mat::Concrete);
                }
            }
            _ => {}
        }
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
        self.exterior = true;
        for i in 0..4 {
            self.wall(c[i], c[(i + 1) % 4], y0, h, thick, mat, sides[i]);
        }
        self.exterior = false;
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
    /// Steps up to an outside doorway, `width` wide, centred `s` along the
    /// wall from `a` (direction `dir`, turned `rot`, outside its local -Z),
    /// to a floor `y0` above the footing. The flight carries on down past
    /// the footing as far as the building can stand above the ground
    /// (`drop`), the steps below the ground buried where it's level, so
    /// the first riser is never more than one step. Each step is wider
    /// than the one above by a tread either side: it's climbed one riser
    /// at a time from in front or from either side (from beside a flight
    /// of equal steps, the ground is a whole flight lower).
    /// Returns how far the flight reaches out from the wall's centre line.
    #[allow(clippy::too_many_arguments)]
    fn door_steps(
        &mut self,
        a: Vec2,
        dir: Vec2,
        rot: Quat,
        s: f32,
        width: f32,
        y0: f32,
        mat: Mat,
    ) -> f32 {
        // Not up a tower (a cabin's door opens onto its own landing), nor
        // where the floor is as good as the ground.
        let low = y0 + self.drop;
        if y0 > 1.0 || low < 0.12 {
            return 0.0;
        }
        // Risers of 0.17 down from the floor, until one is at or below
        // the lowest the ground can be.
        let (rise, tread) = (0.17, 0.3);
        let n = (low / rise - 1e-3).ceil().max(1.0) as usize;
        let sunk = 1.0 + self.drop;
        let base = a + dir * s;
        let out = rot * Vec3::NEG_Z;
        for k in 0..n {
            // k = 0 is the top step, a few millimetres under the floor
            // (out of the plinth's plane where it laps the plinth's edge).
            let top = -rise * k as f32 - 0.004;
            let reach = (k + 1) as f32 * tread;
            let w = width + 0.4 + 2.0 * tread * k as f32;
            let c = Vec3::new(base.x, y0 + (top - sunk) * 0.5, base.y) + out * (reach * 0.5);
            self.piece(
                Shape::Box(Vec3::new(w, top + sunk, reach)),
                Transform::from_translation(c).with_rotation(rot),
                mat,
                true,
            );
        }
        n as f32 * tread
    }

    /// A drowned building's silt washed out of its doorway: a bank sloping
    /// from the threshold at `y0` down to the ground and on under it,
    /// `width` wide, out to -Z from the wall whose centre line is at `z`
    /// (`thick` thick). A body walks up a slope at any slant; up steps at
    /// a slant its step up met the doorway's jamb, and bots stuck there.
    /// Returns how far out from the wall's centre line it meets the ground.
    fn silt_bank(&mut self, z: f32, thick: f32, width: f32, y0: f32) -> f32 {
        let run = 2.6;
        let face = z - thick * 0.5;
        // Its foot under the lowest the ground can be.
        let low = -(self.drop + 0.3);
        let (a, c) = (
            Vec2::new(face - run * (y0 - low) / y0, low),
            Vec2::new(face, y0),
        );
        let along = c - a;
        let normal = Vec3::new(0.0, along.x, -along.y).normalize();
        let t = 0.5;
        let mid = Vec3::new(0.0, (a.y + c.y) * 0.5, (a.x + c.x) * 0.5);
        self.piece(
            Shape::Box(Vec3::new(width + 0.6, t, along.length())),
            Transform::from_translation(mid - normal * t * 0.5)
                .with_rotation(Quat::from_rotation_x(-along.y.atan2(along.x))),
            Mat::Silt,
            true,
        );
        run + thick * 0.5
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
        self.gable_roof_with(w, d, eave_y, pitch_deg, overhang, roof, gable, true)
    }

    /// A pitched roof, with a ceiling at the eaves or open to the rafters
    /// (halls show their trusses).
    #[allow(clippy::too_many_arguments)]
    pub fn gable_roof_with(
        &mut self,
        w: f32,
        d: f32,
        eave_y: f32,
        pitch_deg: f32,
        overhang: f32,
        roof: Mat,
        gable: Mat,
        ceiling: bool,
    ) -> &mut Self {
        let pitch = pitch_deg.to_radians();
        let rise = (w * 0.5) * pitch.tan();
        let slope_len = (w * 0.5 + overhang) / pitch.cos();
        let thick = 0.12;
        let depth = d + 2.0 * overhang;
        for side in [-1.0f32, 1.0] {
            let cx = side * (w * 0.5 + overhang) * 0.5 - side * overhang * 0.0;
            let cy = eave_y + rise * 0.5 - overhang * pitch.tan() * 0.5 + 0.06;
            let rot = Quat::from_rotation_z(-side * pitch);
            // A ruin's roof: the front third of the +X slope gone over the
            // fallen corner (damaged), or only the front's two slopes left
            // (burnt).
            let spans: &[(f32, f32)] = match (self.ruin, side > 0.0) {
                (None, _) => &[(-0.5, 0.5)],
                (Some(Ruin::Damaged), true) => &[(-1.0 / 6.0, 0.5)],
                (Some(Ruin::Damaged), false) => &[(-0.5, 0.5)],
                (Some(Ruin::Burnt), _) => &[(-0.5, -1.0 / 6.0)],
                // (A drowned house has no roof to build.)
                (Some(Ruin::Drowned), _) => &[],
            };
            for &(z0, z1) in spans {
                self.piece(
                    Shape::Box(Vec3::new(slope_len, thick, depth * (z1 - z0))),
                    Transform::from_xyz(cx, cy, depth * (z0 + z1) * 0.5).with_rotation(rot),
                    roof,
                    true,
                );
            }
            if self.ruin.is_some() {
                self.ruin_rafters(side, eave_y, rise, w, d);
            }
        }
        for z in [-d * 0.5, d * 0.5] {
            self.piece(
                Shape::Prism(Vec3::new(w, rise, 0.14)),
                Transform::from_xyz(0.0, eave_y, z),
                gable,
                false,
            );
        }
        // Ruins: no trim left, and a ceiling holed or gone.
        if let Some(ruin) = self.ruin {
            if ceiling {
                self.ruin_ceiling(ruin, w, d, eave_y);
            }
            return self;
        }
        // Fascias along the eaves, barge boards up the gables, a cap over
        // the ridge (see the slabs above: each is turned about Z by
        // -side * pitch, so its +X runs down the slope on side +1).
        let (sp, cp) = pitch.sin_cos();
        let cx = (w * 0.5 + overhang) * 0.5;
        let cy = eave_y + rise * 0.5 - overhang * pitch.tan() * 0.5 + 0.06;
        // Where the ridge is on a slab's top face, along its slope.
        let u_ridge = -(cx + thick * 0.5 * sp) / cp;
        for side in [-1.0f32, 1.0] {
            let rot = Quat::from_rotation_z(-side * pitch);
            let local = |u: f32, v: f32| {
                Vec3::new(side * (cx + u * cp + v * sp), cy - u * sp + v * cp, 0.0)
            };
            let edge = local(slope_len * 0.5, 0.0);
            self.piece(
                Shape::Box(Vec3::new(0.03, 0.2, depth)),
                Transform::from_translation(edge + Vec3::new(side * 0.01, -0.06, 0.0)),
                Mat::Timber,
                false,
            );
            for z in [-1.0f32, 1.0] {
                self.piece(
                    Shape::Box(Vec3::new(slope_len, 0.2, 0.03)),
                    Transform::from_translation(
                        local(0.0, -0.07) + Vec3::Z * z * (depth * 0.5 + 0.015),
                    )
                    .with_rotation(rot),
                    Mat::Timber,
                    false,
                );
            }
            self.piece(
                Shape::Box(Vec3::new(0.15, 0.03, depth + 0.08)),
                Transform::from_translation(local(u_ridge + 0.07, thick * 0.5 + 0.017))
                    .with_rotation(rot),
                Mat::DarkMetal,
                false,
            );
        }
        // Ceiling, so the attic doesn't glare.
        if ceiling {
            self.piece(
                Shape::Box(Vec3::new(w, 0.04, d)),
                Transform::from_xyz(0.0, eave_y + 0.02, 0.0),
                Mat::Planks,
                false,
            );
        }
        self
    }

    /// A ruin's rafters under one slope (`side`, as `gable_roof_with`'s
    /// slabs), from the ridge down onto the wall's top: across its hole
    /// (damaged, one snapped), or all of them, charred, some fallen and
    /// some burnt short, with the ridge beam (burnt).
    fn ruin_rafters(&mut self, side: f32, eave_y: f32, rise: f32, w: f32, d: f32) {
        let burnt = self.ruin == Some(Ruin::Burnt);
        // Over the walls (`d` is the roof's: the walls' and 0.18 either
        // end), not the overhangs.
        let span = d - 0.6;
        let (mat, zs): (Mat, Vec<f32>) = if burnt {
            let n = (span / 0.6).floor() as i32;
            (
                Mat::Charred,
                (0..=n)
                    .map(|k| -span * 0.5 + span * k as f32 / n as f32)
                    .collect(),
            )
        } else {
            // Damaged: they came down with the corner (they're in its
            // rubble).
            return;
        };
        // `w` is the roof's (the walls' and 0.18 either side): the wall's
        // middle is 0.18 in from its edge.
        let ridge = |z: f32| Vec3::new(0.0, eave_y + rise - 0.12, z);
        let wall = |z: f32| Vec3::new(side * (w * 0.5 - 0.18), eave_y + 0.05, z);
        for (k, &z) in zs.iter().enumerate() {
            // Burnt: one in three gone, one in four burnt down to its
            // lower half; damaged, the middle one snapped.
            let (gone, short) = if burnt {
                ((k + (side > 0.0) as usize) % 3 == 1, k % 4 == 2)
            } else {
                (false, k == 1)
            };
            if gone {
                continue;
            }
            let (low, high) = (wall(z), ridge(z));
            let high = if short { low.lerp(high, 0.5) } else { high };
            strut(self, low, high, 0.07, 0.14, mat);
        }
        if burnt && side > 0.0 {
            // What's left of the ridge beam, under what's left of the roof
            // (and stopping short of its torn edge).
            let (z0, z1) = (-span * 0.5, -(d + 1.0) / 6.0 - 0.03);
            self.deco(
                Vec3::new(0.14, 0.2, z1 - z0),
                ridge((z0 + z1) * 0.5) + Vec3::Y * 0.02,
                Mat::Charred,
            );
        }
    }

    /// A ruin's ceiling at the eaves: holed under the roof's hole
    /// (damaged; two joists left across it), or burnt away to its joists.
    fn ruin_ceiling(&mut self, ruin: Ruin, w: f32, d: f32, eave_y: f32) {
        let y = eave_y + 0.02;
        match ruin {
            Ruin::Damaged => {
                // The +X half's front open under the roof's hole (`w` the
                // roof's: the walls' insides 0.27 in).
                let (x0, x1, z0, z1) = (0.0, w * 0.5 - 0.27, -d * 0.5 + 0.27, -d / 6.0);
                self.geo.skylights.push((
                    Vec3::new((x0 + x1) * 0.5, y, (z0 + z1) * 0.5),
                    Vec2::new((x1 - x0) * 0.5, (z1 - z0) * 0.5),
                ));
                self.deco(
                    Vec3::new(w * 0.5, 0.04, d),
                    Vec3::new(-w * 0.25, y, 0.0),
                    Mat::Planks,
                );
                let z0 = -d / 6.0;
                self.deco(
                    Vec3::new(w * 0.5, 0.04, d * 0.5 - z0),
                    Vec3::new(w * 0.25, y, (z0 + d * 0.5) * 0.5),
                    Mat::Planks,
                );
            }
            // No ceiling to build.
            Ruin::Drowned => {}
            Ruin::Burnt => {
                // Open from where the front's slopes stop to the back.
                let (z0, z1) = (-(d + 1.0) / 6.0, d * 0.5 - 0.27);
                self.geo.skylights.push((
                    Vec3::new(0.0, y, (z0 + z1) * 0.5),
                    Vec2::new(w * 0.5 - 0.27, (z1 - z0) * 0.5),
                ));
                let n = (d / 1.2).floor() as i32;
                for k in 0..=n {
                    if k % 3 == 1 {
                        continue;
                    }
                    let z = -d * 0.5 + 0.2 + (d - 0.4) * k as f32 / n as f32;
                    // Their ends in the walls (`w` is the roof's, 0.18
                    // past the walls' middles either side).
                    self.deco(
                        Vec3::new(w - 0.56, 0.12, 0.08),
                        Vec3::new(0.0, y - 0.09, z),
                        Mat::Charred,
                    );
                }
            }
        }
    }

    pub fn flat_roof(&mut self, w: f32, d: f32, y: f32, mat: Mat) -> &mut Self {
        self.bx(
            Vec3::new(w + 0.4, 0.25, d + 0.4),
            Vec3::new(0.0, y + 0.125, 0.0),
            mat,
        );
        // Sheet-metal coping round the edge, standing a little above the
        // roof: the front and back pieces cover the sides' ends.
        let (x, z) = (w * 0.5 + 0.2, d * 0.5 + 0.2);
        for s in [-1.0f32, 1.0] {
            self.piece(
                Shape::Box(Vec3::new(2.0 * x + 0.08, 0.1, 0.04)),
                Transform::from_xyz(0.0, y + 0.22, s * (z + 0.02)),
                Mat::DarkMetal,
                false,
            );
            self.piece(
                Shape::Box(Vec3::new(0.04, 0.1, 2.0 * z)),
                Transform::from_xyz(s * (x + 0.02), y + 0.22, 0.0),
                Mat::DarkMetal,
                false,
            );
        }
        self
    }

    // --- furniture ---------------------------------------------------
    //
    // Each piece of furniture is boxes for its colliders and loot surfaces,
    // drawn by its model from `art/props/furniture.py` and `crates.py` when
    // that has loaded (the boxes otherwise).

    pub fn table(&mut self, at: Vec3, yaw: f32, loot: Option<&'static str>) -> &mut Self {
        let from = self.geo.pieces.len();
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
        self.dress(
            from,
            vec![("table", Transform::from_translation(at).with_rotation(rot))],
        );
        if let Some(t) = loot {
            self.loot(at + Vec3::Y * 0.82, t);
        }
        self
    }

    pub fn shelf(&mut self, at: Vec3, yaw: f32, loot: Option<&'static str>) -> &mut Self {
        let from = self.geo.pieces.len();
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
        let model = ["shelf", "shelf_metal"][variant(at, 2)];
        self.dress(
            from,
            vec![(model, Transform::from_translation(at).with_rotation(rot))],
        )
    }

    pub fn bed(&mut self, at: Vec3, yaw: f32) -> &mut Self {
        let from = self.geo.pieces.len();
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
        self.dress(
            from,
            vec![("bed", Transform::from_translation(at).with_rotation(rot))],
        )
    }

    pub fn counter(
        &mut self,
        at: Vec3,
        yaw: f32,
        len: f32,
        loot: Option<&'static str>,
    ) -> &mut Self {
        let from = self.geo.pieces.len();
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(len, 0.9, 0.6)),
            Transform::from_translation(at + Vec3::Y * 0.45).with_rotation(rot),
            Mat::Timber,
            true,
        );
        // The model's doors face the room: towards the building's middle
        // (behind a shop counter, that's the keeper's side).
        let front = if (rot * Vec3::NEG_Z).dot(-at.with_y(0.0)) < 0.0 {
            rot * Quat::from_rotation_y(std::f32::consts::PI)
        } else {
            rot
        };
        let n = (len / 0.6).round().max(1.0);
        let w = len / n;
        let pick = variant(at, 2);
        let mut models: Vec<(&'static str, Transform)> = (0..n as usize)
            .map(|i| {
                let x = -len * 0.5 + w * (i as f32 + 0.5);
                let m = ["counter_cabinet", "counter_drawers"][(i + pick) % 2];
                (
                    m,
                    Transform::from_translation(at + front * Vec3::X * x)
                        .with_rotation(front)
                        .with_scale(Vec3::new(w / 0.6, 1.0, 1.0)),
                )
            })
            .collect();
        models.push((
            "counter_top",
            Transform::from_translation(at)
                .with_rotation(front)
                .with_scale(Vec3::new(len, 1.0, 1.0)),
        ));
        self.dress(from, models);
        if let Some(t) = loot {
            self.loot(at + Vec3::Y * 0.95, t);
        }
        self
    }

    pub fn stove(&mut self, at: Vec3, yaw: f32) -> &mut Self {
        let from = self.geo.pieces.len();
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
        self.dress(
            from,
            vec![("stove", Transform::from_translation(at).with_rotation(rot))],
        )
    }

    pub fn crate_box(&mut self, at: Vec3, size: f32, loot: Option<&'static str>) -> &mut Self {
        let from = self.geo.pieces.len();
        self.bx(Vec3::splat(size), at + Vec3::Y * size * 0.5, Mat::Planks);
        // A cube either way round: turned a quarter or so for variety.
        let k = variant(at, 8);
        let model = ["crate_slat", "crate_army"][k % 2];
        let turn = Quat::from_rotation_y((k / 2) as f32 * std::f32::consts::FRAC_PI_2);
        self.dress(
            from,
            vec![(
                model,
                Transform::from_translation(at)
                    .with_rotation(turn)
                    .with_scale(Vec3::splat(size)),
            )],
        );
        if let Some(t) = loot {
            self.loot(at + Vec3::Y * (size + 0.05), t);
        }
        self
    }

    /// A piece of equipment: a box of `size` standing at `at` (turned
    /// `yaw`) for its collider, drawn by `model`, loot on its top if any.
    pub fn furnish(
        &mut self,
        model: &'static str,
        size: Vec3,
        at: Vec3,
        yaw: f32,
        loot: Option<&'static str>,
    ) -> &mut Self {
        let from = self.geo.pieces.len();
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(size),
            Transform::from_translation(at + Vec3::Y * size.y * 0.5).with_rotation(rot),
            Mat::DarkMetal,
            true,
        );
        self.dress(
            from,
            vec![(model, Transform::from_translation(at).with_rotation(rot))],
        );
        if let Some(t) = loot {
            self.loot(at + Vec3::Y * (size.y + 0.05), t);
        }
        self
    }

    /// A small thing drawn by `model` with no collider (a coil of rope on
    /// the floor), standing in as a low box until it loads.
    pub fn strew(&mut self, model: &'static str, size: Vec3, at: Vec3, yaw: f32) -> &mut Self {
        let from = self.geo.pieces.len();
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(size),
            Transform::from_translation(at + Vec3::Y * size.y * 0.5).with_rotation(rot),
            Mat::Timber,
            false,
        );
        self.dress(
            from,
            vec![(model, Transform::from_translation(at).with_rotation(rot))],
        )
    }

    pub fn workbench(&mut self, at: Vec3, yaw: f32, loot: Option<&'static str>) -> &mut Self {
        let from = self.geo.pieces.len();
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
        self.dress(
            from,
            vec![(
                "workbench",
                Transform::from_translation(at).with_rotation(rot),
            )],
        );
        if let Some(t) = loot {
            self.loot(at + Vec3::Y * 0.98, t);
        }
        self
    }

    /// A plank chair, facing -Z (its back at +Z): solid seat and back.
    pub fn chair(&mut self, at: Vec3, yaw: f32) -> &mut Self {
        let from = self.geo.pieces.len();
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(0.42, 0.45, 0.42)),
            Transform::from_translation(at + Vec3::Y * 0.225).with_rotation(rot),
            Mat::Timber,
            true,
        );
        self.piece(
            Shape::Box(Vec3::new(0.38, 0.45, 0.04)),
            Transform::from_translation(at + rot * Vec3::new(0.0, 0.675, 0.18)).with_rotation(rot),
            Mat::Timber,
            true,
        );
        self.dress(
            from,
            vec![("chair", Transform::from_translation(at).with_rotation(rot))],
        )
    }

    /// A chapel bench 2.4 long, facing -Z (its back at +Z).
    pub fn pew(&mut self, at: Vec3, yaw: f32) -> &mut Self {
        let from = self.geo.pieces.len();
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(2.4, 0.45, 0.4)),
            Transform::from_translation(at + Vec3::Y * 0.225).with_rotation(rot),
            Mat::Timber,
            true,
        );
        self.piece(
            Shape::Box(Vec3::new(2.4, 0.5, 0.05)),
            Transform::from_translation(at + rot * Vec3::new(0.0, 0.72, 0.19)).with_rotation(rot),
            Mat::Timber,
            true,
        );
        self.dress(
            from,
            vec![("pew", Transform::from_translation(at).with_rotation(rot))],
        )
    }

    /// A steel locker, its door on -Z.
    pub fn locker(&mut self, at: Vec3, yaw: f32) -> &mut Self {
        let from = self.geo.pieces.len();
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(0.5, 1.8, 0.45)),
            Transform::from_translation(at + Vec3::Y * 0.9).with_rotation(rot),
            Mat::PaintedMetal,
            true,
        );
        self.dress(
            from,
            vec![("locker", Transform::from_translation(at).with_rotation(rot))],
        )
    }

    /// A 200 l drum.
    pub fn barrel(&mut self, at: Vec3) -> &mut Self {
        let from = self.geo.pieces.len();
        self.piece(
            Shape::Cylinder(0.29, 0.88),
            Transform::from_translation(at + Vec3::Y * 0.44),
            Mat::PaintedMetal,
            true,
        );
        let turn = Quat::from_rotation_y(variant(at, 12) as f32 * 0.5);
        self.dress(
            from,
            vec![(
                "barrel",
                Transform::from_translation(at).with_rotation(turn),
            )],
        )
    }

    /// A pallet on the floor (things can stand on it).
    pub fn pallet(&mut self, at: Vec3, yaw: f32) -> &mut Self {
        let from = self.geo.pieces.len();
        let rot = Quat::from_rotation_y(yaw);
        self.piece(
            Shape::Box(Vec3::new(1.2, 0.144, 0.8)),
            Transform::from_translation(at + Vec3::Y * 0.072).with_rotation(rot),
            Mat::Planks,
            true,
        );
        self.dress(
            from,
            vec![("pallet", Transform::from_translation(at).with_rotation(rot))],
        )
    }

    /// A ladder you climb with your hands: its rails' middle from `bottom`
    /// to `top`, `width` apart, standing out toward `out`; `over` is where
    /// the top puts you (prefab space).
    pub fn ladder(
        &mut self,
        bottom: Vec3,
        top: Vec3,
        width: f32,
        out: Vec3,
        over: Option<Vec3>,
    ) -> &mut Self {
        self.geo.ladders.push(crate::interact::hand_climb::Ladder {
            bottom,
            top,
            width,
            out,
            over,
        });
        self
    }

    pub fn finish(self) -> Geo {
        self.geo
    }
}

/// Builds a prefab's geometry. `None` for prefabs drawn elsewhere.
/// Every generated prefab's id.
pub const PREFABS: [&str; 52] = [
    "house_plank",
    "farmhouse",
    "house_brick",
    "cabin_log",
    "shop",
    "shed",
    "barn",
    "sawmill",
    "pump_station",
    "bus_depot",
    "rail_shed",
    "mast_hut",
    "radio_mast",
    "ranger_tower",
    "survey_tower",
    "wreck_lada",
    "wreck_uaz",
    "wreck_lorry",
    "quarantine_gate",
    "guard_booth",
    "robot_gate",
    "boxcar",
    "log_pile",
    "anomaly_core",
    "factory_hall",
    "chimney",
    "target",
    "water_tower",
    "headframe",
    "signal_box",
    "steeple",
    "chapel",
    "mill_race",
    "ropeway_station",
    "ropeway_pylon",
    "ropeway_pylon_down",
    "blast_furnace",
    "house_plank_damaged",
    "house_plank_burnt",
    "farmhouse_damaged",
    "farmhouse_burnt",
    "house_brick_damaged",
    "house_brick_burnt",
    "house_plank_drowned",
    "farmhouse_drowned",
    "house_brick_drowned",
    "chapel_drowned",
    "fence_garden",
    "cabin_log_damaged",
    "cabin_log_burnt",
    "shop_damaged",
    "shop_burnt",
];

pub fn prefab(id: &str) -> Option<Geo> {
    if let Some(g) = super::underground::geo(id) {
        return Some(g);
    }
    if let Some(g) = super::dam::geo(id) {
        return Some(g);
    }
    if let Some(g) = super::fell::geo(id) {
        return Some(g);
    }
    let mut b = Builder {
        drop: extraction_core::map::prefab_lift(id),
        ..Default::default()
    };
    let base = extraction_core::map::prefab_base(id);
    if base != id {
        let ruin = if id.ends_with("_burnt") {
            Ruin::Burnt
        } else if id.ends_with("_drowned") {
            Ruin::Drowned
        } else {
            Ruin::Damaged
        };
        b.ruin = Some(ruin);
        if ruin == Ruin::Drowned {
            match base {
                "house_plank" => house_drowned(&mut b, 8.0, 7.0, Mat::Planks),
                "farmhouse" => house_drowned(&mut b, 10.0, 8.0, Mat::Planks),
                "house_brick" => house_drowned(&mut b, 10.0, 8.0, Mat::Brick),
                "chapel" => chapel_drowned(&mut b),
                _ => return None,
            }
            return Some(b.finish());
        }
        match base {
            "house_plank" => house_ruin(&mut b, 8.0, 7.0, Mat::Planks, Mat::Shingles, ruin),
            "farmhouse" => house_ruin(&mut b, 10.0, 8.0, Mat::Planks, Mat::Corrugated, ruin),
            "house_brick" => house_ruin(&mut b, 10.0, 8.0, Mat::Brick, Mat::Shingles, ruin),
            "cabin_log" => cabin_ruin(&mut b, ruin),
            "shop" => shop_ruin(&mut b, ruin),
            _ => return None,
        }
        return Some(b.finish());
    }
    match id {
        "house_plank" => house(&mut b, 8.0, 7.0, Mat::Planks, Mat::Shingles),
        "farmhouse" => house(&mut b, 10.0, 8.0, Mat::Planks, Mat::Corrugated),
        "house_brick" => {
            house(&mut b, 10.0, 8.0, Mat::Brick, Mat::Shingles);
            b.loot(Vec3::new(3.5, 0.6, 2.8), "medical");
        }
        "cabin_log" => cabin(&mut b),
        "fence_garden" => fence_garden(&mut b),
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
        "survey_tower" => survey_tower(&mut b),
        "wreck_lada" => car_wreck(&mut b, "wreck_lada"),
        "wreck_uaz" => car_wreck(&mut b, "wreck_uaz"),
        "wreck_lorry" => car_wreck(&mut b, "wreck_lorry"),
        "quarantine_gate" => quarantine_gate(&mut b),
        "guard_booth" => guard_booth(&mut b),
        "robot_gate" => robot_gate(&mut b),
        "boxcar" => boxcar(&mut b),
        "log_pile" => log_pile(&mut b),
        "anomaly_core" => anomaly(&mut b),
        "factory_hall" => {
            hall(&mut b, 36.0, 20.0, 9.0, Mat::Brick, Mat::Corrugated, false);
            factory_fitout(&mut b, 36.0, 20.0, 9.0, 0.1);
            // On a pallet: the crate (and its loot) stand on the pallet's top.
            b.pallet(Vec3::new(-14.0, 0.1, -7.0), 0.0);
            b.crate_box(Vec3::new(-14.0, 0.244, -7.0), 0.9, Some("workshop"));
            b.crate_box(Vec3::new(-12.8, 0.1, -7.0), 0.7, Some("gadgets"));
        }
        "chimney" => chimney(&mut b),
        "water_tower" => water_tower(&mut b),
        "headframe" => headframe(&mut b),
        "signal_box" => signal_box(&mut b),
        "steeple" => steeple(&mut b),
        "chapel" => chapel(&mut b),
        "mill_race" => mill_race(&mut b),
        "ropeway_station" => ropeway_station(&mut b),
        "ropeway_pylon" => ropeway_pylon(&mut b),
        "ropeway_pylon_down" => ropeway_pylon_down(&mut b),
        "blast_furnace" => blast_furnace(&mut b),
        "target" => target(&mut b),
        id if id.starts_with(super::scenes::PREFIX) => return super::scenes::geo(id),
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
    b.chair(
        Vec3::new(-x * 0.4 - 0.3, f, -z * 0.2 - 0.6),
        std::f32::consts::PI,
    );
    b.chair(Vec3::new(-x * 0.4 + 0.25, f, -z * 0.2 + 0.6), 0.1);
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

/// A house in ruins, from `house`'s plan (same footprint, same doors).
/// Damaged: a third of the roof's +X slope fallen in, the ceiling under
/// it holed and the boards and shingles in a heap below; a breach knocked
/// through the +X wall, its bricks or boards spilled either side; a roof
/// beam down across the front room from the -X wall to the floor (duck
/// under its high end); every other window's glass out. Burnt: charred
/// through, the roof gone to a few rafters and one corner of slope, no
/// doors, no glass, the furniture burnt to its iron and ash, two charred
/// beams down (one wedged across the back passage at head height).
fn house_ruin(b: &mut Builder, w: f32, d: f32, wall: Mat, roof: Mat, ruin: Ruin) {
    let start = b.geo.pieces.len();
    let f = 0.5;
    let h = 2.7;
    let (x, z) = (w * 0.5, d * 0.5);
    let damaged = ruin == Ruin::Damaged;
    b.plinth(w, d, f, Mat::Concrete, Mat::WoodFloor);
    // Damaged: the front +X corner fallen in to a stub (both its walls'
    // ends), and a breach knocked through the +X wall.
    let stub = 0.5;
    let side: &[Opening] = if damaged {
        &[
            breach(0.6, 1.2, stub, h),
            window(d * 0.3),
            breach(d * 0.52, 1.4, 0.3, 1.9),
            window(d * 0.8),
        ]
    } else {
        &[window(d * 0.3), window(d * 0.75)]
    };
    let front_breach = [breach(w - 0.5, 1.0, stub, h)];
    let front_open = [window(w * 0.2), door(w * 0.5), window(w * 0.8)];
    let front: Vec<Opening> = if damaged {
        front_open.iter().chain(&front_breach).copied().collect()
    } else {
        front_open.to_vec()
    };
    b.shell(
        w,
        d,
        f,
        h,
        0.18,
        wall,
        [
            &front,
            side,
            &[window(w * 0.25), window(w * 0.75)],
            &[window(d * 0.5)],
        ],
    );
    // The partitions 2 cm under the walls' tops (with the ceiling gone
    // the two would show in one plane).
    let pz = z - d * 0.42;
    b.wall(
        Vec2::new(-x, pz),
        Vec2::new(x, pz),
        f,
        h - 0.02,
        0.1,
        Mat::Plaster,
        &[gap(w * 0.25, 0.9, 2.05), gap(w * 0.75, 0.9, 2.05)],
    );
    b.wall(
        Vec2::new(0.0, pz),
        Vec2::new(0.0, z),
        f,
        h - 0.02,
        0.1,
        Mat::Plaster,
        &[],
    );
    b.gable_roof(w + 0.36, d + 0.36, f + h, 32.0, 0.5, roof, wall);
    let heap = if wall == Mat::Brick {
        "rubble_brick"
    } else {
        "rubble_planks"
    };
    if damaged {
        // The fallen corner's rubble, outside and in, in steps: up from the
        // ground over the stub and down to the floor.
        let corner = Vec3::new(x, 0.0, -z);
        let from = b.geo.pieces.len();
        for (size, at) in [
            (
                Vec3::new(1.8, 0.35, 1.8),
                corner + Vec3::new(0.55, 0.175, -0.55),
            ),
            (
                Vec3::new(1.1, 0.35, 1.1),
                corner + Vec3::new(0.25, 0.525, -0.25),
            ),
            (
                Vec3::new(1.2, 0.3, 1.2),
                corner + Vec3::new(-0.75, f + 0.15, 0.75),
            ),
        ] {
            b.bx(size, at, Mat::Planks);
        }
        b.dress(
            from,
            vec![
                (
                    heap,
                    Transform::from_translation(corner + Vec3::new(0.5, 0.0, -0.5))
                        .with_rotation(Quat::from_rotation_y(0.8))
                        .with_scale(Vec3::new(1.1, 1.8, 1.2)),
                ),
                (
                    heap,
                    Transform::from_translation(corner + Vec3::new(-0.75, f + 0.03, 0.75))
                        .with_rotation(Quat::from_rotation_y(-0.7)),
                ),
            ],
        );
        // Under the roof's hole; the breach's spill outside and in.
        rubble(
            b,
            Vec3::new(x * 0.4, f + 0.03, -z * 0.5),
            0.0,
            "rubble_planks",
            Mat::Planks,
        );
        rubble(b, Vec3::new(x + 0.75, 0.0, 0.14), 1.57, heap, Mat::Planks);
        rubble(
            b,
            Vec3::new(x - 0.75, f + 0.03, 0.14),
            -1.57,
            heap,
            Mat::Planks,
        );
        fallen_beam(
            b,
            Vec3::new(-x + 0.25, f + 1.7, -z * 0.15),
            Vec3::new(x * 0.05, f + 0.12, -z * 0.55),
            "beam_fallen",
        );
        // The rooms as they were, a little less.
        b.table(Vec3::new(-x * 0.4, f, -z * 0.2), 0.2, Some("house"));
        b.chair(
            Vec3::new(-x * 0.4 - 0.3, f, -z * 0.2 - 0.6),
            std::f32::consts::PI,
        );
        b.shelf(
            Vec3::new(x - 0.3, f, -z * 0.75),
            std::f32::consts::FRAC_PI_2,
            None,
        );
        b.counter(
            Vec3::new(-x + 0.4, f, z - 1.4),
            std::f32::consts::FRAC_PI_2,
            1.8,
            Some("kitchen"),
        );
        b.stove(Vec3::new(-x * 0.35, f, z - 0.45), 0.0);
        b.bed(Vec3::new(x - 0.6, f, z - 1.2), 0.0);
        b.crate_box(Vec3::new(x * 0.35, f, z - 0.5), 0.5, Some("house"));
    } else {
        // Charred beams: one wedged across the back passage from the
        // partition's door to the -X wall (duck under it), one on the
        // floor; the roof's fall in a heap; ash.
        fallen_beam(
            b,
            Vec3::new(-x + 0.08, f + 1.45, pz + 0.35),
            Vec3::new(-0.02, f + 1.25, pz + 0.65),
            "beam_charred",
        );
        fallen_beam(
            b,
            Vec3::new(x * 0.7, f + 0.12, -z * 0.7),
            Vec3::new(-x * 0.1, f + 0.12, -z * 0.1),
            "beam_charred",
        );
        rubble(
            b,
            Vec3::new(x * 0.45, f + 0.03, -z * 0.15),
            1.0,
            "rubble_planks",
            Mat::Planks,
        );
        for (ax, az) in [(-0.45, -0.5), (0.6, 0.75), (-0.75, 0.55)] {
            ash(b, Vec3::new(x * ax, f, z * az));
        }
        // What didn't burn: the stove, the bedstead, a table's legs, a
        // steel locker.
        b.stove(Vec3::new(-x * 0.35, f, z - 0.45), 0.0);
        b.geo.loot.push(LootSpot {
            at: Vec3::new(-x * 0.35, f + 0.9, z - 0.45),
            table: "kitchen",
        });
        let at = Vec3::new(x - 0.6, f, z - 1.2);
        let from = b.geo.pieces.len();
        b.bx(
            Vec3::new(0.9, 0.42, 1.95),
            at + Vec3::Y * 0.21,
            Mat::DarkMetal,
        );
        b.dress(from, vec![("bed_burnt", Transform::from_translation(at))]);
        let at = Vec3::new(-x * 0.4, f, -z * 0.2);
        let from = b.geo.pieces.len();
        b.bx(Vec3::new(1.2, 0.3, 0.7), at + Vec3::Y * 0.15, Mat::Timber);
        b.dress(from, vec![("table_burnt", Transform::from_translation(at))]);
        b.locker(
            Vec3::new(x - 0.35, f, -z * 0.5),
            -std::f32::consts::FRAC_PI_2,
        );
        b.loot(Vec3::new(x * 0.35, f + 0.05, z - 0.5), "house");
        burn(b, start);
    }
}

/// A rubble heap (`at` on the floor or the ground): two steps you can
/// walk up, drawn by `model`, their boxes `mat`.
fn rubble(b: &mut Builder, at: Vec3, yaw: f32, model: &'static str, mat: Mat) {
    let from = b.geo.pieces.len();
    let rot = Quat::from_rotation_y(yaw);
    for (size, y) in [
        (Vec3::new(1.4, 0.2, 1.1), 0.1),
        (Vec3::new(0.8, 0.2, 0.6), 0.3),
    ] {
        b.piece(
            Shape::Box(size),
            Transform::from_translation(at + Vec3::Y * y).with_rotation(rot),
            mat,
            true,
        );
    }
    b.dress(
        from,
        vec![(model, Transform::from_translation(at).with_rotation(rot))],
    );
}

/// A fallen beam from `a` to `c`, drawn by `model` (4.4 m along X,
/// stretched to fit).
fn fallen_beam(b: &mut Builder, a: Vec3, c: Vec3, model: &'static str) {
    let from = b.geo.pieces.len();
    let v = c - a;
    let rot = Quat::from_rotation_arc(Vec3::X, v.normalize());
    b.piece(
        Shape::Box(Vec3::new(v.length(), 0.24, 0.2)),
        Transform::from_translation((a + c) * 0.5).with_rotation(rot),
        Mat::Timber,
        true,
    );
    b.dress(
        from,
        vec![(
            model,
            Transform::from_translation((a + c) * 0.5)
                .with_rotation(rot)
                .with_scale(Vec3::new(v.length() / 4.4, 1.0, 1.0)),
        )],
    );
}

/// A drift of ash on the floor at `at`.
fn ash(b: &mut Builder, at: Vec3) {
    let from = b.geo.pieces.len();
    b.deco(Vec3::new(1.0, 0.08, 0.8), at + Vec3::Y * 0.04, Mat::Charred);
    b.dress(from, vec![("ash_heap", Transform::from_translation(at))]);
}

/// Burnt: the pieces since `start` charred (timber), sooted (brick and
/// plaster), the paint burnt off the iron.
fn burn(b: &mut Builder, start: usize) {
    for p in &mut b.geo.pieces[start..] {
        p.mat = match p.mat {
            Mat::Planks | Mat::Shingles | Mat::WoodFloor | Mat::Timber | Mat::LogWall => {
                Mat::Charred
            }
            Mat::Brick => Mat::SootBrick,
            Mat::Plaster => Mat::SootPlaster,
            Mat::Corrugated | Mat::PaintedMetal => Mat::RustyMetal,
            m => m,
        };
    }
}

/// How deep a drowned house's silt lies over its footing: four of a
/// stair's risers (`underground::STEP_RISE`), a hand under the sills.
pub const SILT: f32 = 0.76;

/// A house that was under a reservoir (`Ruin::Drowned`): its walls, no
/// roof, floors or partitions, no doors or glass, the timber sodden; silt
/// inside to just under the sills, banked down from the doorway (the
/// door frame rotted out, the wall over it fallen: a doorway open to the
/// sky); a stove standing out of the silt. The tide line is
/// `decay::stains`'s, found by the silt.
fn house_drowned(b: &mut Builder, w: f32, d: f32, wall: Mat) {
    let start = b.geo.pieces.len();
    // At grade: the valley's mud has come up round it.
    let f = 0.05;
    let h = 2.7;
    let thick = 0.18;
    let (x, z) = (w * 0.5, d * 0.5);
    // Its footing banked in silt too (where the bed fell away from it).
    b.plinth(w, d, f, Mat::Silt, Mat::Silt);
    let doorway = 1.6;
    b.shell(
        w,
        d,
        f,
        h,
        thick,
        wall,
        [
            &[
                window(w * 0.2),
                breach(w * 0.5, doorway, 0.0, h),
                window(w * 0.8),
            ],
            &[window(d * 0.3), window(d * 0.75)],
            &[window(w * 0.25), window(w * 0.75)],
            &[window(d * 0.5)],
        ],
    );
    // Open to the sky wall to wall.
    b.geo.skylights.push((
        Vec3::new(0.0, f + h, 0.0),
        Vec2::new(x - thick * 0.5, z - thick * 0.5),
    ));
    // The silt: wall to wall, and through the doorway to the bank.
    let top = f + SILT;
    b.geo.floor = Some(top);
    b.bx(
        Vec3::new(w - thick, SILT, d - thick),
        Vec3::new(0.0, f + SILT * 0.5, 0.0),
        Mat::Silt,
    );
    b.bx(
        Vec3::new(doorway, SILT, thick + 0.02),
        Vec3::new(0.0, f + SILT * 0.5, -z),
        Mat::Silt,
    );
    let reach = b.silt_bank(-z, thick, doorway, top);
    let r = |v: f32| (v * 100.0).round() / 100.0 + 0.0;
    b.geo.entrances.push(extraction_core::map::Entrance {
        at: [0.0, r(-z - thick * 0.5)],
        out: [0.0, -1.0],
        width: doorway,
        clear: r((reach - thick * 0.5).max(0.0) + 1.0),
    });
    // Lumps of it against the walls, and what stands out of it: the
    // stove, a fallen joist.
    for (ax, az, sx, sz) in [
        (-0.7, 0.75, 1.6, 0.9),
        (0.65, 0.2, 1.0, 1.4),
        (-0.2, -0.55, 1.2, 0.8),
    ] {
        b.deco(
            Vec3::new(sx, 0.12, sz),
            Vec3::new(x * ax, top + 0.06, z * az),
            Mat::Silt,
        );
    }
    b.stove(Vec3::new(-x * 0.35, top - 0.45, z - 0.45), 0.0);
    b.geo.loot.push(LootSpot {
        at: Vec3::new(-x * 0.35, top + 0.45, z - 0.45),
        table: "kitchen",
    });
    let (a, c) = (
        Vec3::new(-x * 0.2, top + 0.05, -z * 0.1),
        Vec3::new(x * 0.55, top + 0.9, z * 0.6),
    );
    let v = c - a;
    b.piece(
        Shape::Box(Vec3::new(0.14, 0.2, v.length())),
        Transform::from_translation((a + c) * 0.5).looking_to(v.normalize(), Vec3::Y),
        Mat::Sodden,
        true,
    );
    // What someone left on a crate since the water went down (loot off
    // the floor).
    b.crate_box(Vec3::new(x * 0.4, top, -z * 0.45), 0.55, Some("house"));
    // Sixty years under water: the timber sodden, the paint and plaster
    // gone to the brick and the iron rusted.
    for p in &mut b.geo.pieces[start..] {
        p.mat = match p.mat {
            Mat::Planks | Mat::Shingles | Mat::WoodFloor | Mat::Timber => Mat::Sodden,
            Mat::Plaster => Mat::Concrete,
            Mat::Corrugated | Mat::PaintedMetal => Mat::RustyMetal,
            m => m,
        };
    }
}

/// The chapel under the reservoir (`chapel_drowned`): the nave's walls
/// standing with their plaster gone to the stone, no roof, its pews up to
/// their seats in silt, the altar table and a shelf standing out of it;
/// the doorway's frame gone, silt banked up to it. The bell tower stood
/// above the water, as it was.
fn chapel_drowned(b: &mut Builder) {
    let start = b.geo.pieces.len();
    let (w, d, f, h) = (10.0, 14.0, 0.05, 4.5);
    let thick = 0.35;
    let (x, z) = (w * 0.5, d * 0.5);
    let tall = |c: f32| Opening {
        centre: c,
        width: 1.0,
        bottom: 1.5,
        top: 3.6,
        kind: OpeningKind::Window,
    };
    // Its footing banked in silt too (where the bed fell away from it).
    b.plinth(w, d, f, Mat::Silt, Mat::Silt);
    let doorway = 2.0;
    b.shell(
        w,
        d,
        f,
        h,
        thick,
        Mat::Plaster,
        [
            &[breach(w * 0.5, doorway, 0.0, 3.2)],
            &[tall(3.0), tall(7.0), tall(11.0)],
            &[],
            &[tall(3.0), tall(7.0), tall(11.0)],
        ],
    );
    b.geo.skylights.push((
        Vec3::new(0.0, f + h, 0.0),
        Vec2::new(x - thick * 0.5, z - thick * 0.5),
    ));
    let top = f + SILT;
    b.geo.floor = Some(top);
    b.bx(
        Vec3::new(w - thick, SILT, d - thick),
        Vec3::new(0.0, f + SILT * 0.5, 0.0),
        Mat::Silt,
    );
    b.bx(
        Vec3::new(doorway, SILT, thick + 0.02),
        Vec3::new(0.0, f + SILT * 0.5, -z),
        Mat::Silt,
    );
    let reach = b.silt_bank(-z, thick, doorway, top);
    let r = |v: f32| (v * 100.0).round() / 100.0 + 0.0;
    b.geo.entrances.push(extraction_core::map::Entrance {
        at: [0.0, r(-z - thick * 0.5)],
        out: [0.0, -1.0],
        width: doorway,
        clear: r((reach - thick * 0.5).max(0.0) + 1.0),
    });
    // The pews to their seats in it, a few gone, the rest knocked askew.
    for k in 0..5 {
        let pz = -z + 3.0 + k as f32 * 1.1;
        for (i, px) in [-2.6f32, 2.6].into_iter().enumerate() {
            if (k + i) % 4 == 3 {
                continue;
            }
            let skew = [0.05, -0.12, 0.0, 0.18, -0.06][(k + 2 * i) % 5];
            b.pew(Vec3::new(px, top - 0.3, pz), std::f32::consts::PI + skew);
        }
    }
    // Loot off the floor: on the altar table and the shelf.
    b.table(Vec3::new(0.0, top - 0.3, z - 1.6), 0.08, Some("house"));
    b.shelf(
        Vec3::new(-x + 0.45, top - 0.25, z - 1.0),
        std::f32::consts::FRAC_PI_2,
        Some("medical"),
    );
    for p in &mut b.geo.pieces[start..] {
        p.mat = match p.mat {
            Mat::Planks | Mat::Shingles | Mat::WoodFloor | Mat::Timber => Mat::Sodden,
            Mat::Plaster => Mat::DressedStone,
            Mat::Corrugated | Mat::PaintedMetal => Mat::RustyMetal,
            m => m,
        };
    }
    // The bell tower, as the chapel's.
    let at = Vec3::new(0.0, 0.0, z + 0.175 + 2.0);
    b.bx(Vec3::new(4.2, 1.5, 4.2), at - Vec3::Y * 0.75, Mat::Concrete);
    let from = b.geo.pieces.len();
    let mut tower = Builder::default();
    steeple(&mut tower);
    let turn =
        Transform::from_translation(at).with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
    for mut p in tower.geo.pieces {
        p.at = turn * p.at;
        b.geo.pieces.push(p);
    }
    b.dress_structure(from, vec![("steeple", turn)]);
}

/// A garden fence: four plank panels between five posts along x, 8.4 m,
/// the boards' face to -Z. The posts are the building's; the panels are
/// breakable things (`fence_panel`), each its own.
fn fence_garden(b: &mut Builder) {
    let (n, span, post) = (4, 2.1, 0.1);
    let x0 = -(n as f32) * span * 0.5;
    for k in 0..=n {
        let x = x0 + k as f32 * span;
        b.bx(
            Vec3::new(post, 1.55, post),
            Vec3::new(x, 1.3 - 0.775, 0.0),
            Mat::Timber,
        );
    }
    for k in 0..n {
        let x = x0 + (k as f32 + 0.5) * span;
        b.geo.breakables.push(Breakable {
            kind: BreakKind::FencePanel,
            at: Transform::from_xyz(x, 0.15 + 0.55, 0.0),
            size: Vec3::new(2.0, 1.1, 0.08),
            model: Some("fence_panel"),
            mat: Mat::Timber,
            yields: None,
        });
    }
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
    b.chair(Vec3::new(0.45, f, -0.5), std::f32::consts::PI + 0.3);
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
    b.wall(
        Vec2::new(-4.5, 1.6),
        Vec2::new(4.5, 1.6),
        f,
        h,
        0.12,
        Mat::Plaster,
        // Between the shelves either side, not behind one.
        &[door(5.5)],
    );
    b.flat_roof(w, d, f + h, Mat::Concrete);
    b.counter(Vec3::new(-1.0, f, -0.4), 0.0, 3.0, Some("food_common"));
    for x in [-3.6, -2.2, 2.4, 3.8] {
        b.shelf(Vec3::new(x, f, 1.2), 0.0, Some("food_common"));
    }
    // In the corner, clear of the back door (at x -3).
    b.crate_box(Vec3::new(-3.9, f, 2.6), 0.7, Some("food_common"));
    b.crate_box(Vec3::new(-2.0, f, 2.7), 0.6, Some("tools"));
    b.shelf(Vec3::new(3.0, f, 3.0), 0.0, Some("medical"));
}

/// A log cabin in ruins, from `cabin`'s plan (same footprint, same door).
/// Damaged: the front +X corner fallen in to a stub, its logs down in a
/// heap over it outside and in; the roof's iron gone from over it, the
/// ceiling holed, a rafter down from the +X wall to the floor; every
/// other window's glass out. Burnt: the logs charred, the roof gone to a
/// few rafters and the front's iron, no door or glass, the bed and stove
/// left in the ash, two charred beams down.
fn cabin_ruin(b: &mut Builder, ruin: Ruin) {
    let start = b.geo.pieces.len();
    let (w, d, f, h) = (7.0, 6.0, 0.4, 2.5);
    let (x, z) = (w * 0.5, d * 0.5);
    let damaged = ruin == Ruin::Damaged;
    b.plinth(w, d, f, Mat::Concrete, Mat::WoodFloor);
    // Damaged: the corner's two walls fallen to a stub (the front's clear
    // of its window's shutter).
    let stub = 0.5;
    let (front, side) = if damaged {
        (
            vec![door(2.0), window(5.0), breach(w - 0.4, 0.8, stub, h)],
            vec![breach(0.6, 1.2, stub, h), window(3.0)],
        )
    } else {
        (vec![door(2.0), window(5.0)], vec![window(3.0)])
    };
    b.shell(
        w,
        d,
        f,
        h,
        0.28,
        Mat::LogWall,
        [&front, &side, &[window(3.5)], &[]],
    );
    b.gable_roof(
        w + 0.5,
        d + 0.5,
        f + h,
        30.0,
        0.5,
        Mat::Corrugated,
        Mat::Planks,
    );
    if damaged {
        // The corner's logs, outside and in, in steps: up from the ground
        // over the stub and down to the floor.
        let corner = Vec3::new(x, 0.0, -z);
        let from = b.geo.pieces.len();
        for (size, at) in [
            (
                Vec3::new(1.8, 0.35, 1.8),
                corner + Vec3::new(0.55, 0.175, -0.55),
            ),
            (
                Vec3::new(1.1, 0.35, 1.1),
                corner + Vec3::new(0.25, 0.525, -0.25),
            ),
            (
                Vec3::new(1.2, 0.3, 1.2),
                corner + Vec3::new(-0.75, f + 0.15, 0.75),
            ),
        ] {
            b.bx(size, at, Mat::Planks);
        }
        b.dress(
            from,
            vec![
                (
                    "rubble_logs",
                    Transform::from_translation(corner + Vec3::new(0.55, 0.0, -0.55))
                        .with_rotation(Quat::from_rotation_y(0.4)),
                ),
                (
                    "rubble_logs",
                    Transform::from_translation(corner + Vec3::new(-0.75, f + 0.03, 0.75))
                        .with_rotation(Quat::from_rotation_y(-0.9))
                        .with_scale(Vec3::splat(0.7)),
                ),
            ],
        );
        // The ceiling's boards under the hole; a rafter from the +X wall's
        // top down to the floor.
        rubble(
            b,
            Vec3::new(1.4, f + 0.03, -1.5),
            0.3,
            "rubble_planks",
            Mat::Planks,
        );
        fallen_beam(
            b,
            Vec3::new(x - 0.1, f + h - 0.1, -1.0),
            Vec3::new(0.3, f + 0.12, -2.2),
            "beam_fallen",
        );
        // The room as it was, knocked about; the crate out of the corner.
        b.stove(Vec3::new(2.6, f, 2.3), 0.0);
        b.bed(Vec3::new(-2.8, f, 1.5), 0.0);
        b.table(Vec3::new(0.0, f, 0.5), 0.5, Some("kitchen"));
        b.chair(Vec3::new(0.6, f, -0.3), 2.0);
        b.shelf(Vec3::new(-1.0, f, 2.7), 0.0, Some("house"));
        b.crate_box(Vec3::new(-2.9, f, -1.0), 0.6, Some("tools"));
    } else {
        // Charred beams: one on the floor, one down from the -X wall;
        // the roof's fall in a heap; ash.
        fallen_beam(
            b,
            Vec3::new(-0.4, f + 0.12, -0.9),
            Vec3::new(2.2, f + 0.12, -1.9),
            "beam_charred",
        );
        fallen_beam(
            b,
            Vec3::new(-x + 0.1, f + 1.5, -1.6),
            Vec3::new(-2.3, f + 0.12, 0.1),
            "beam_charred",
        );
        rubble(
            b,
            Vec3::new(1.6, f + 0.03, 0.4),
            1.0,
            "rubble_planks",
            Mat::Planks,
        );
        for (ax, az) in [(-1.0, 2.3), (-2.4, -2.1), (0.9, -2.3)] {
            ash(b, Vec3::new(ax, f, az));
        }
        // What didn't burn: the stove, the bedstead, the table's legs.
        b.stove(Vec3::new(2.6, f, 2.3), 0.0);
        b.loot(Vec3::new(2.6, f + 0.9, 2.3), "kitchen");
        let at = Vec3::new(-2.8, f, 1.5);
        let from = b.geo.pieces.len();
        b.bx(
            Vec3::new(0.9, 0.42, 1.95),
            at + Vec3::Y * 0.21,
            Mat::DarkMetal,
        );
        b.dress(from, vec![("bed_burnt", Transform::from_translation(at))]);
        let at = Vec3::new(0.0, f, 0.5);
        let from = b.geo.pieces.len();
        b.bx(Vec3::new(1.2, 0.3, 0.7), at + Vec3::Y * 0.15, Mat::Timber);
        b.dress(from, vec![("table_burnt", Transform::from_translation(at))]);
        // The shelf's and the crate's things in the ash.
        b.loot(Vec3::new(-1.0, f + 0.1, 2.4), "house");
        b.loot(Vec3::new(2.9, f + 0.05, -2.4), "tools");
        burn(b, start);
    }
}

/// A box from `lo` to `hi` (its length), `wide` across along `across`
/// and `thick`: a slab come down at a slant.
fn slab_down(b: &mut Builder, lo: Vec3, hi: Vec3, across: Vec3, wide: f32, thick: f32) {
    let v = hi - lo;
    let xh = v.normalize();
    let zh = (across - xh * across.dot(xh)).normalize();
    let yh = zh.cross(xh);
    b.piece(
        Shape::Box(Vec3::new(v.length(), thick, wide)),
        Transform::from_translation((lo + hi) * 0.5)
            .with_rotation(Quat::from_mat3(&Mat3::from_cols(xh, yh, zh))),
        Mat::Concrete,
        true,
    );
}

/// A flat roof (as `Builder::flat_roof`) fallen through over `hole` (x0,
/// x1, z0, z1, inside the walls): the slab round it, its coping, a broken
/// edge hanging in on its rebar from the hole's -X (`lip` -1) or +X (1)
/// side, and the sky through it (`Geo::skylights`).
fn holed_roof(b: &mut Builder, w: f32, d: f32, y: f32, hole: [f32; 4], lip: f32) {
    let [x0, x1, z0, z1] = hole;
    let (x, z) = (w * 0.5 + 0.2, d * 0.5 + 0.2);
    let t = 0.25;
    for (xa, xb, za, zb) in [
        (-x, x, -z, z0),
        (-x, x, z1, z),
        (-x, x0, z0, z1),
        (x1, x, z0, z1),
    ] {
        b.bx(
            Vec3::new(xb - xa, t, zb - za),
            Vec3::new((xa + xb) * 0.5, y + t * 0.5, (za + zb) * 0.5),
            Mat::Concrete,
        );
    }
    for s in [-1.0f32, 1.0] {
        b.piece(
            Shape::Box(Vec3::new(2.0 * x + 0.08, 0.1, 0.04)),
            Transform::from_xyz(0.0, y + 0.22, s * (z + 0.02)),
            Mat::DarkMetal,
            false,
        );
        b.piece(
            Shape::Box(Vec3::new(0.04, 0.1, 2.0 * z)),
            Transform::from_xyz(s * (x + 0.02), y + 0.22, 0.0),
            Mat::DarkMetal,
            false,
        );
    }
    b.geo.skylights.push((
        Vec3::new((x0 + x1) * 0.5, y, (z0 + z1) * 0.5),
        Vec2::new((x1 - x0) * 0.5, (z1 - z0) * 0.5),
    ));
    // Its top end in the slab, hanging steeply into the hole.
    let edge = if lip < 0.0 { x0 } else { x1 };
    let top = Vec3::new(edge + lip * 0.05, y + 0.1, (z0 + z1) * 0.5);
    let dir = Vec3::new(-lip * 0.5, -0.866, 0.0);
    let len = 0.7;
    b.piece(
        Shape::Box(Vec3::new(len, 0.12, (z1 - z0) * 0.6)),
        Transform::from_translation(top + dir * len * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::X, dir.normalize())),
        Mat::Concrete,
        false,
    );
}

/// A shop in ruins, from `shop`'s plan (same footprint, same doors).
/// Damaged: looted, the front's glass all out; part of the roof slab
/// fallen in over the front room's +X side, one piece down on a heap of
/// concrete (a slope you can walk up) and one hanging from the hole's
/// edge onto it; two of the shelves toppled on their faces. Burnt: the
/// plaster sooted, no doors or glass, the roof fallen in over the
/// counter, a slab of it leaning from the floor onto the charred counter
/// and another broken on the floor; the shelves and crates burnt to ash
/// and what was on them, a steel locker left in the back room.
fn shop_ruin(b: &mut Builder, ruin: Ruin) {
    let start = b.geo.pieces.len();
    let (w, d, f, h) = (9.0, 7.0, 0.3, 3.1);
    let damaged = ruin == Ruin::Damaged;
    b.plinth(w, d, f, Mat::Concrete, Mat::WoodFloor);
    b.glass_out = true;
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
    b.glass_out = false;
    b.wall(
        Vec2::new(-4.5, 1.6),
        Vec2::new(4.5, 1.6),
        f,
        h,
        0.12,
        Mat::Plaster,
        &[door(5.5)],
    );
    let roof = f + h;
    if damaged {
        holed_roof(b, w, d, roof, [0.9, 3.7, -3.0, -0.9], -1.0);
        // The slab's fall: a heap, a piece down on it to the floor, a
        // piece hanging from the hole's +X edge onto that.
        rubble(
            b,
            Vec3::new(2.9, f + 0.03, -2.4),
            0.3,
            "rubble_concrete",
            Mat::Concrete,
        );
        slab_down(
            b,
            Vec3::new(2.2, f + 0.1, -1.0),
            Vec3::new(2.2, f + 0.62, -2.6),
            Vec3::X,
            2.4,
            0.2,
        );
        slab_down(
            b,
            Vec3::new(2.9, f + 0.55, -1.95),
            Vec3::new(3.75, roof + 0.08, -1.95),
            Vec3::Z,
            1.6,
            0.2,
        );
        b.counter(Vec3::new(-1.0, f, -0.4), 0.0, 3.0, Some("food_common"));
        for x in [-2.2, 3.8] {
            b.shelf(Vec3::new(x, f, 1.2), 0.0, Some("food_common"));
        }
        // Toppled on their faces (their feet where they stood), what's
        // left on their backs.
        for x in [-3.6, 2.4] {
            let from = b.geo.pieces.len();
            b.bx(
                Vec3::new(1.0, 0.35, 1.8),
                Vec3::new(x, f + 0.175, 0.2),
                Mat::Timber,
            );
            b.dress(
                from,
                vec![(
                    "shelf",
                    Transform::from_xyz(x, f + 0.175, 1.1)
                        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                )],
            );
            b.loot(Vec3::new(x, f + 0.4, 0.2), "food_common");
        }
        b.crate_box(Vec3::new(-3.9, f, 2.6), 0.7, Some("food_common"));
        b.crate_box(Vec3::new(-2.0, f, 2.7), 0.6, Some("tools"));
        b.shelf(Vec3::new(3.0, f, 3.0), 0.0, Some("medical"));
    } else {
        holed_roof(b, w, d, roof, [-3.9, -0.5, -2.6, 0.9], 1.0);
        // The counter burnt to its carcass, a board of its top left.
        b.bx(
            Vec3::new(3.0, 0.85, 0.6),
            Vec3::new(-1.0, f + 0.425, -0.4),
            Mat::Timber,
        );
        b.deco(
            Vec3::new(1.58, 0.04, 0.62),
            Vec3::new(-0.31, f + 0.87, -0.4),
            Mat::Timber,
        );
        b.loot(Vec3::new(-0.3, f + 0.95, -0.4), "food_common");
        // The roof's fall: a slab from the floor onto the counter's end,
        // another broken across a heap in the corner.
        slab_down(
            b,
            Vec3::new(-4.0, f + 0.1, -0.4),
            Vec3::new(-2.2, f + 0.95, -0.4),
            Vec3::Z,
            1.6,
            0.2,
        );
        rubble(
            b,
            Vec3::new(-3.3, f + 0.03, -2.4),
            0.5,
            "rubble_concrete",
            Mat::Concrete,
        );
        slab_down(
            b,
            Vec3::new(-1.4, f + 0.09, -1.5),
            Vec3::new(-2.8, f + 0.38, -2.2),
            Vec3::Z,
            1.1,
            0.18,
        );
        // The shelves and crates gone to ash, their tins in it.
        for (ax, az) in [(-2.9, 1.05), (3.1, 1.05), (-2.0, 2.7), (-3.8, 2.6)] {
            ash(b, Vec3::new(ax, f, az));
        }
        b.loot(Vec3::new(2.4, f + 0.1, 0.9), "food_common");
        b.loot(Vec3::new(-2.0, f + 0.1, 2.7), "tools");
        b.locker(Vec3::new(3.6, f, 3.0), 0.0);
        b.loot(Vec3::new(2.6, f + 0.05, 2.9), "medical");
        burn(b, start);
    }
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
    for (i, (x, z)) in [(-4.0, 4.0), (-4.0, 5.3), (-2.9, 4.6), (-4.0, 4.6)]
        .into_iter()
        .enumerate()
    {
        let from = b.geo.pieces.len();
        b.bx(
            Vec3::new(1.1, 0.6, 0.5),
            Vec3::new(x, f + 0.3, z),
            Mat::Canvas,
        );
        let turn = Quat::from_rotation_y(std::f32::consts::PI * (i % 2) as f32);
        b.dress(
            from,
            vec![("hay_bale", Transform::from_xyz(x, f, z).with_rotation(turn))],
        );
    }
    // Along the side wall between its door (z -4) and window (z 3).
    b.workbench(
        Vec3::new(5.0, f, 0.5),
        std::f32::consts::FRAC_PI_2,
        Some("workshop"),
    );
    b.crate_box(Vec3::new(4.5, f, 5.5), 0.8, Some("food_common"));
    b.crate_box(Vec3::new(-4.5, f, -5.5), 0.6, Some("tools"));
    b.barrel(Vec3::new(5.2, f, 6.9));
    b.barrel(Vec3::new(4.6, f, 7.2));
}

fn sawmill(b: &mut Builder) {
    // An open shed: posts on concrete pads under a pitched iron roof (the
    // ridge along the length, on tie beams and purlins), over a saw bench
    // with a log on its carriage.
    let (w, d, h) = (24.0, 12.0, 4.5);
    let xs = [-11.5, -5.75, 0.0, 5.75, 11.5];
    for x in xs {
        for z in [-5.5, 5.5] {
            b.bx(
                Vec3::new(0.5, 0.5, 0.5),
                Vec3::new(x, -0.1, z),
                Mat::Concrete,
            );
            b.bx(
                Vec3::new(0.25, h - 0.15, 0.25),
                Vec3::new(x, 0.15 + (h - 0.15) * 0.5, z),
                Mat::Timber,
            );
        }
        // Tie beam across, at the eaves.
        b.bx(
            Vec3::new(0.22, 0.3, d - 0.5),
            Vec3::new(x, h - 0.15, 0.0),
            Mat::Timber,
        );
        // King post up to the ridge, and a rafter down each side.
        let pitch = 15f32.to_radians();
        let rise = (d * 0.5) * pitch.tan();
        b.bx(
            Vec3::new(0.18, rise, 0.18),
            Vec3::new(x, h + rise * 0.5, 0.0),
            Mat::Timber,
        );
        for side in [-1.0f32, 1.0] {
            let z = side * d * 0.25;
            b.piece(
                Shape::Box(Vec3::new(0.12, 0.18, d * 0.5 / pitch.cos())),
                Transform::from_xyz(x, h + rise - z.abs() * pitch.tan() - 0.08, z)
                    .with_rotation(Quat::from_rotation_x(side * pitch)),
                Mat::Timber,
                false,
            );
        }
    }
    let pitch = 15f32.to_radians();
    let half = d * 0.5 + 0.6;
    let slope = half / pitch.cos();
    let rise = (d * 0.5) * pitch.tan();
    for side in [-1.0f32, 1.0] {
        let tilt = Quat::from_rotation_x(side * pitch);
        // Sheeting, then two purlins under it along the length.
        b.piece(
            Shape::Box(Vec3::new(w + 1.0, 0.05, slope)),
            Transform::from_xyz(
                0.0,
                h + rise - half * 0.5 * pitch.tan() + 0.2,
                side * half * 0.5,
            )
            .with_rotation(tilt),
            Mat::Corrugated,
            true,
        );
        for f in [0.33, 0.75] {
            let z = side * d * 0.5 * f;
            b.piece(
                Shape::Box(Vec3::new(w + 0.6, 0.14, 0.12)),
                Transform::from_xyz(0.0, h + rise - z.abs() * pitch.tan() + 0.08, z)
                    .with_rotation(tilt),
                Mat::Timber,
                false,
            );
        }
    }
    b.piece(
        Shape::Box(Vec3::new(w + 0.6, 0.2, 0.16)),
        Transform::from_xyz(0.0, h + rise + 0.05, 0.0),
        Mat::Timber,
        false,
    );
    // The saw bench: trestle legs, two rails, a carriage with a log on it.
    let (bx0, bx1) = (-7.0, 3.0);
    let mut x = bx0;
    while x <= bx1 + 0.01 {
        for z in [-0.55, 0.55] {
            b.bx(
                Vec3::new(0.14, 0.8, 0.14),
                Vec3::new(x, 0.4, z),
                Mat::Timber,
            );
        }
        b.bx(
            Vec3::new(0.12, 0.12, 1.3),
            Vec3::new(x, 0.74, 0.0),
            Mat::Timber,
        );
        x += 2.0;
    }
    for z in [-0.5, 0.5] {
        b.bx(
            Vec3::new(bx1 - bx0 + 0.3, 0.08, 0.1),
            Vec3::new((bx0 + bx1) * 0.5, 0.84, z),
            Mat::DarkMetal,
        );
    }
    b.bx(
        Vec3::new(3.0, 0.08, 1.3),
        Vec3::new(-4.5, 0.92, 0.0),
        Mat::Planks,
    );
    for x in [-5.6, -3.4] {
        b.bx(
            Vec3::new(0.15, 0.25, 1.1),
            Vec3::new(x, 1.08, 0.0),
            Mat::DarkMetal,
        );
    }
    b.piece(
        Shape::Cylinder(0.32, 3.4),
        Transform::from_xyz(-4.5, 1.28, 0.0)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        Mat::BarkSpruce,
        true,
    );
    // The blade, its guard, and the motor driving it by a belt.
    b.piece(
        Shape::Cylinder(0.65, 0.012),
        Transform::from_xyz(-1.0, 0.95, 0.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Mat::GunMetal,
        false,
    );
    b.piece(
        Shape::Cylinder(0.08, 0.3),
        Transform::from_xyz(-1.0, 0.95, 0.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Mat::DarkMetal,
        false,
    );
    b.bx(
        Vec3::new(1.5, 0.1, 0.24),
        Vec3::new(-1.0, 1.66, 0.0),
        Mat::PaintedMetal,
    );
    // The guard's post stands on the floor beside the bench.
    b.bx(
        Vec3::new(0.08, 1.66, 0.08),
        Vec3::new(-0.2, 0.83, 0.7),
        Mat::PaintedMetal,
    );
    b.bx(
        Vec3::new(0.08, 0.08, 0.7),
        Vec3::new(-0.2, 1.62, 0.35),
        Mat::PaintedMetal,
    );
    b.bx(
        Vec3::new(0.9, 0.6, 0.6),
        Vec3::new(-1.0, 0.3, 1.5),
        Mat::PaintedMetal,
    );
    b.bx(
        Vec3::new(0.9, 0.06, 0.62),
        Vec3::new(-1.0, 0.63, 1.5),
        Mat::DarkMetal,
    );
    let belt = Vec3::new(0.0, 0.95 - 0.45, 0.0 - 1.5);
    b.piece(
        Shape::Box(Vec3::new(0.08, 0.02, belt.length())),
        Transform::from_xyz(-1.0, 0.7, 0.75).looking_to(belt, Vec3::Y),
        Mat::DarkMetal,
        false,
    );
    b.workbench(Vec3::new(8.0, 0.0, 3.5), 0.0, Some("workshop"));
    b.workbench(Vec3::new(8.0, 0.0, -3.5), 0.0, Some("tools"));
    // Logs waiting, and sawn boards stacked on bearers with stickers.
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
    for x in [2.6, 4.0, 5.4] {
        b.bx(
            Vec3::new(0.15, 0.15, 1.2),
            Vec3::new(x, 0.075, -4.0),
            Mat::Timber,
        );
    }
    for layer in 0..5 {
        let y = 0.15 + layer as f32 * 0.09;
        for k in 0..5 {
            b.bx(
                Vec3::new(3.4, 0.05, 0.2),
                Vec3::new(4.0, y + 0.025, -4.45 + k as f32 * 0.22),
                Mat::Planks,
            );
        }
        if layer < 4 {
            for x in [2.6, 4.0, 5.4] {
                b.bx(
                    Vec3::new(0.04, 0.04, 1.1),
                    Vec3::new(x, y + 0.07, -4.0),
                    Mat::Timber,
                );
            }
        }
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
            &[window(3.0), door(7.0), window(11.3)],
            &[window(5.0)],
            &[window(4.0), window(10.0)],
            &[door(5.0)],
        ],
    );
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
    b.chair(Vec3::new(5.4, f, -2.35), -0.2);
    b.locker(Vec3::new(6.6, f, -4.45), 0.0);
    for (x, z) in [(-6.3, 4.2), (-5.6, 4.35), (-6.2, 3.55)] {
        b.barrel(Vec3::new(x, f, z));
    }
    b.shelf(
        Vec3::new(6.6, f, -1.0),
        std::f32::consts::FRAC_PI_2,
        Some("medical"),
    );
    b.workbench(Vec3::new(-5.5, f, -3.5), 0.0, Some("workshop"));
}

/// The works' machine floor: an overhead crane on runway beams along the
/// columns, its hook hanging, and a row of lathes.
fn factory_fitout(b: &mut Builder, w: f32, d: f32, h: f32, f: f32) {
    let xc = w * 0.5 - 0.26;
    let rail_y = f + h - 1.6;
    for side in [-1.0f32, 1.0] {
        // Runway beam on brackets off the columns (at +-3.75 and +-7.5).
        b.piece(
            Shape::Box(Vec3::new(0.3, 0.4, d - 1.2)),
            Transform::from_xyz(side * (xc - 0.4), rail_y, 0.0),
            Mat::RustyMetal,
            false,
        );
        for z in [-7.5, -3.75, 3.75, 7.5] {
            b.piece(
                Shape::Box(Vec3::new(0.55, 0.3, 0.25)),
                Transform::from_xyz(side * (xc - 0.3), rail_y - 0.35, z),
                Mat::RustyMetal,
                false,
            );
        }
    }
    // The bridge across, its trolley and hoist, the hook let down.
    let bz = 1.8;
    b.piece(
        Shape::Box(Vec3::new(2.0 * (xc - 0.4) + 0.3, 0.6, 0.5)),
        Transform::from_xyz(0.0, rail_y + 0.5, bz),
        Mat::PaintedMetal,
        false,
    );
    for side in [-1.0f32, 1.0] {
        b.piece(
            Shape::Box(Vec3::new(0.5, 0.3, 1.4)),
            Transform::from_xyz(side * (xc - 0.4), rail_y + 0.35, bz),
            Mat::PaintedMetal,
            false,
        );
    }
    let hx = -4.0;
    b.piece(
        Shape::Box(Vec3::new(0.9, 0.5, 0.9)),
        Transform::from_xyz(hx, rail_y + 0.0, bz),
        Mat::DarkMetal,
        false,
    );
    let drop = 4.0;
    for dz in [-0.06, 0.06] {
        b.piece(
            Shape::Box(Vec3::new(0.03, drop, 0.03)),
            Transform::from_xyz(hx, rail_y - 0.25 - drop * 0.5, bz + dz),
            Mat::DarkMetal,
            false,
        );
    }
    b.piece(
        Shape::Box(Vec3::new(0.35, 0.3, 0.3)),
        Transform::from_xyz(hx, rail_y - 0.25 - drop - 0.15, bz),
        Mat::PaintedMetal,
        false,
    );
    b.piece(
        Shape::Box(Vec3::new(0.08, 0.3, 0.08)),
        Transform::from_xyz(hx, rail_y - 0.25 - drop - 0.45, bz),
        Mat::DarkMetal,
        false,
    );
    // Lathes in a row: a bed on two pedestals, headstock, chuck,
    // tailstock.
    for x in [-9.0, -3.0, 3.0] {
        let z = -2.0;
        for px in [x - 0.8, x + 0.8] {
            b.bx(
                Vec3::new(0.4, 0.8, 0.5),
                Vec3::new(px, f + 0.4, z),
                Mat::PaintedMetal,
            );
        }
        b.bx(
            Vec3::new(2.4, 0.18, 0.45),
            Vec3::new(x, f + 0.89, z),
            Mat::DarkMetal,
        );
        b.bx(
            Vec3::new(0.5, 0.45, 0.5),
            Vec3::new(x - 0.9, f + 1.2, z),
            Mat::PaintedMetal,
        );
        b.piece(
            Shape::Cylinder(0.14, 0.12),
            Transform::from_xyz(x - 0.59, f + 1.2, z)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            Mat::DarkMetal,
            false,
        );
        b.bx(
            Vec3::new(0.3, 0.3, 0.32),
            Vec3::new(x + 0.9, f + 1.13, z),
            Mat::PaintedMetal,
        );
    }
}

/// A hall's steel frame: columns inside the long walls and a truss on
/// each pair under the open roof (`gable_roof_with`, pitch 14 degrees,
/// over `w + 0.5`), clear of the side windows.
fn hall_frame(b: &mut Builder, w: f32, d: f32, h: f32, f: f32) {
    let pitch = 14f32.to_radians();
    let eave = f + h;
    // The roof sheet's underside above x (see `gable_roof_with`).
    let (rw, ov, thick) = (w + 0.5, 0.4, 0.12);
    let cx = (rw * 0.5 + ov) * 0.5;
    let cy = eave + (rw * 0.5) * pitch.tan() * 0.5 - ov * pitch.tan() * 0.5 + 0.06;
    let under = |x: f32| cy + (cx - x.abs()) * pitch.tan() - thick * 0.5 / pitch.cos();
    let xc = w * 0.5 - 0.26;
    let mut zs = Vec::new();
    let mut z = 3.75;
    while z <= d * 0.5 - 1.5 {
        zs.push(z);
        zs.push(-z);
        z += 3.75;
    }
    for z in zs {
        for side in [-1.0f32, 1.0] {
            b.bx(
                Vec3::new(0.25, h, 0.25),
                Vec3::new(side * xc, f + h * 0.5, z),
                Mat::RustyMetal,
            );
        }
        // Bottom chord at the eaves, top chords under the sheeting, and
        // struts between at the quarters and the middle.
        b.piece(
            Shape::Box(Vec3::new(2.0 * xc + 0.25, 0.18, 0.14)),
            Transform::from_xyz(0.0, eave - 0.09, z),
            Mat::RustyMetal,
            false,
        );
        for side in [-1.0f32, 1.0] {
            let (x0, x1) = (side * (xc + 0.12), 0.0);
            let (y0, y1) = (under(x0) - 0.1, under(x1) - 0.1);
            let along = Vec2::new(x1 - x0, y1 - y0);
            b.piece(
                Shape::Box(Vec3::new(along.length(), 0.18, 0.14)),
                Transform::from_xyz((x0 + x1) * 0.5, (y0 + y1) * 0.5, z)
                    .with_rotation(Quat::from_rotation_z(along.y.atan2(along.x))),
                Mat::RustyMetal,
                false,
            );
            let xq = side * xc * 0.5;
            let top = under(xq) - 0.19;
            b.piece(
                Shape::Box(Vec3::new(0.1, top - eave, 0.1)),
                Transform::from_xyz(xq, (top + eave) * 0.5, z),
                Mat::RustyMetal,
                false,
            );
        }
        let top = under(0.0) - 0.19;
        b.piece(
            Shape::Box(Vec3::new(0.12, top - eave, 0.12)),
            Transform::from_xyz(0.0, (top + eave) * 0.5, z),
            Mat::RustyMetal,
            false,
        );
    }
}

fn hall(b: &mut Builder, w: f32, d: f32, h: f32, wall: Mat, roof: Mat, office: bool) {
    let f = 0.1;
    b.piece(
        Shape::Box(Vec3::new(w, 0.04, d)),
        Transform::from_xyz(0.0, f - 0.02, 0.0),
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
    b.gable_roof_with(w + 0.5, d + 0.5, f + h, 14.0, 0.4, roof, wall, false);
    hall_frame(b, w, d, h, f);
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
        wrecked_bus(b, Vec3::new(-w * 0.2, f, 0.0));
        // Office walls stop at the shell's inner face, not in it.
        let (x, z) = (w * 0.5 - 0.125, d * 0.5 - 0.125);
        b.wall(
            Vec2::new(w * 0.5 - 6.0, z),
            Vec2::new(w * 0.5 - 6.0, d * 0.5 - 5.0),
            f,
            2.8,
            0.12,
            Mat::Plaster,
            &[door(3.5)],
        );
        b.wall(
            Vec2::new(w * 0.5 - 6.0, d * 0.5 - 5.0),
            Vec2::new(x, d * 0.5 - 5.0),
            f,
            2.8,
            0.12,
            Mat::Plaster,
            &[window(3.0)],
        );
        // Clear of the bay's workbench's end.
        b.table(
            Vec3::new(w * 0.5 - 2.6, f, d * 0.5 - 2.5),
            0.0,
            Some("house"),
        );
        b.chair(Vec3::new(w * 0.5 - 2.7, f, d * 0.5 - 1.85), 0.15);
        b.locker(
            Vec3::new(w * 0.5 - 5.5, f, d * 0.5 - 0.45),
            std::f32::consts::PI,
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

// --- Landmarks: tall silhouettes each valley can be told by (models in
// `art/props/landmarks.py`, which fill these colliders; the pieces are
// their blockouts too).

/// A pillar of a given shape standing on y0.
fn column(b: &mut Builder, shape: Shape, y0: f32, mat: Mat) {
    let h = match shape {
        Shape::Box(s) => s.y,
        Shape::Cylinder(_, h) => h,
        Shape::Prism(s) => s.y,
    };
    b.piece(
        shape,
        Transform::from_xyz(0.0, y0 + h * 0.5, 0.0),
        mat,
        true,
    );
}

/// A brick water tower with a steel tank, 18 m to its finial.
fn water_tower(b: &mut Builder) {
    let from = b.geo.pieces.len();
    column(b, Shape::Cylinder(2.05, 0.7), -0.3, Mat::Concrete);
    column(b, Shape::Cylinder(1.8, 11.6), 0.4, Mat::Brick);
    column(b, Shape::Cylinder(2.95, 0.5), 12.0, Mat::PaintedMetal);
    column(b, Shape::Cylinder(2.5, 3.5), 12.5, Mat::PaintedMetal);
    column(b, Shape::Cylinder(1.3, 1.4), 16.0, Mat::RustyMetal);
    b.dress_structure(from, vec![("water_tower", Transform::IDENTITY)]);
    // The ladder up the back to the gallery (topping out just under its
    // deck, onto it), then on up the tank to the roof.
    b.ladder(
        Vec3::new(0.0, 0.3, 1.95),
        Vec3::new(0.0, 11.9, 1.95),
        0.4,
        Vec3::Z,
        Some(Vec3::new(0.0, 12.55, 2.6)),
    );
    b.ladder(
        Vec3::new(0.0, 12.8, 2.75),
        Vec3::new(0.0, 16.4, 2.75),
        0.4,
        Vec3::Z,
        Some(Vec3::new(0.0, 16.05, 1.9)),
    );
}

/// A quarry's steel headframe over its shaft, sheave wheels on top.
fn headframe(b: &mut Builder) {
    let from = b.geo.pieces.len();
    b.bx(Vec3::new(4.4, 0.4, 4.4), Vec3::ZERO, Mat::Concrete);
    for (x, z) in [(-1.4, -1.4), (1.4, -1.4), (-1.4, 1.4), (1.4, 1.4)] {
        b.bx(
            Vec3::new(0.3, 15.8, 0.3),
            Vec3::new(x, 8.1, z),
            Mat::RustyMetal,
        );
    }
    for side in [-1.0f32, 1.0] {
        let (a, c) = (
            Vec3::new(side * 1.4, 15.0, 1.4),
            Vec3::new(side * 2.3, 0.0, 7.0),
        );
        let d = c - a;
        b.piece(
            Shape::Box(Vec3::new(0.32, d.length(), 0.32)),
            Transform::from_translation((a + c) * 0.5)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize())),
            Mat::RustyMetal,
            true,
        );
    }
    b.bx(
        Vec3::new(3.6, 0.15, 3.6),
        Vec3::new(0.0, 16.075, 0.0),
        Mat::PaintedMetal,
    );
    b.piece(
        Shape::Box(Vec3::new(1.8, 3.0, 2.9)),
        Transform::from_xyz(0.0, 17.6, 0.0),
        Mat::DarkMetal,
        false,
    );
    b.dress_structure(from, vec![("headframe", Transform::IDENTITY)]);
}

/// A box from `a` to `c` (its centre line), `w` across in x (or, along x,
/// in z) and `d` the other way.
fn strut(b: &mut Builder, a: Vec3, c: Vec3, w: f32, d: f32, mat: Mat) {
    let v = c - a;
    b.piece(
        Shape::Box(Vec3::new(w, v.length(), d)),
        Transform::from_translation((a + c) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, v.normalize())),
        mat,
        true,
    );
}

/// A sloping walk in the YZ plane, its top surface's centre line from `a`
/// to `c`.
fn walk(b: &mut Builder, a: Vec3, c: Vec3, width: f32, thick: f32, mat: Mat) {
    let along = c - a;
    let normal = Vec3::new(0.0, along.z, -along.y).normalize();
    b.piece(
        Shape::Box(Vec3::new(width, thick, along.length())),
        Transform::from_translation((a + c) * 0.5 - normal * thick * 0.5)
            .with_rotation(Quat::from_rotation_x(-along.y.atan2(along.z))),
        mat,
        true,
    );
}

// The mill race's measures, as `art/props/heroes.py` (MR_* there), in its
// tower's space; the prefab's origin is its footprint's middle, MR_SHIFT
// along z from the tower.
const MR_SHIFT: f32 = -19.3;
const MR_DECK: f32 = 19.0;
const MR_GATE: f32 = MR_DECK + 5.0;
const MR_ROOF: f32 = MR_DECK + 6.0;
const MR_LEG: (f32, f32) = (2.6, 2.1);
const MR_TANK: (f32, f32) = (4.4, 2.4);
const MR_WALK: (f32, f32) = (0.75, 1.0);
const MR_WALK_TOP: (f32, f32) = (3.2, 40.0);
const MR_TROUGH: (f32, f32) = (-0.5, 1.2);
const MR_LEVEL: f32 = 5.1;
const MR_WHEEL: (f32, f32, f32) = (2.4, 2.55, 43.0);
/// The wheel house's inside, x (a pen on the ground: the terrain has no
/// holes for a pit); the curb between its tailrace and its dry floor, the
/// floor, the walls' top.
const MR_PIT: (f32, f32) = (-3.0, 0.22);
const MR_CURB: f32 = -1.3;
const MR_FLOOR: f32 = 0.05;
const MR_WALL_TOP: f32 = 1.6;

fn mr_walk_y(z: f32) -> f32 {
    let (z0, z1) = MR_WALK_TOP;
    let top = MR_DECK + 0.3;
    top - (top - 0.3) * (z - z0) / (z1 - z0)
}

fn mr_trough_y(z: f32) -> f32 {
    (mr_walk_y(z.max(MR_WALK_TOP.0)) + 0.6).max(MR_LEVEL)
}

/// Marrow Creek's hero landmark: a timber sluice tower with its header
/// tank, the race running down a trestle beside a walkway to an overshot
/// wheel. Walk up the walkway to the deck round the tank (a crate's up
/// there). Drawn by `mill_race` (`art/props/heroes.py`).
fn mill_race(b: &mut Builder) {
    let from = b.geo.pieces.len();
    let top = MR_DECK + 0.3;
    let (lf, lt) = MR_LEG;
    let leg = |sx: f32, sz: f32, y: f32| {
        let r = lf + (lt - lf) * y / MR_DECK;
        Vec3::new(sx * r, y, sz * r)
    };
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            b.bx(
                Vec3::new(0.9, 0.5, 0.9),
                Vec3::new(sx * lf, 0.0, sz * lf),
                Mat::Concrete,
            );
            strut(
                b,
                leg(sx, sz, 0.25),
                leg(sx, sz, MR_DECK),
                0.36,
                0.36,
                Mat::Timber,
            );
        }
    }
    // The deck, its railing (open where the walkway arrives), the tank.
    let half = 3.2;
    b.bx(
        Vec3::new(2.0 * half, 0.3, 2.0 * half),
        Vec3::new(0.0, MR_DECK + 0.15, 0.0),
        Mat::Planks,
    );
    let (wx, ww) = MR_WALK;
    let e = half - 0.06;
    let gap = (wx - ww * 0.5 - 0.05, wx + ww * 0.5 + 0.05);
    let rail = |b: &mut Builder, a: Vec2, c: Vec2| {
        let mid = (a + c) * 0.5;
        let size = (c - a).abs() + Vec2::splat(0.07);
        b.bx(
            Vec3::new(size.x, 1.05, size.y),
            Vec3::new(mid.x, top + 0.525, mid.y),
            Mat::Timber,
        );
    };
    rail(b, Vec2::new(-e, -e), Vec2::new(e, -e));
    rail(b, Vec2::new(e, -e), Vec2::new(e, e));
    rail(b, Vec2::new(-e, -e), Vec2::new(-e, e));
    rail(b, Vec2::new(-e, e), Vec2::new(gap.0, e));
    rail(b, Vec2::new(gap.1, e), Vec2::new(e, e));
    let (side, th) = MR_TANK;
    let hs = side * 0.5;
    for k in 0..4 {
        let rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 * k as f32);
        b.piece(
            Shape::Box(Vec3::new(side, th, 0.1)),
            Transform::from_translation(rot * Vec3::new(0.0, top + th * 0.5, -hs + 0.05))
                .with_rotation(rot),
            Mat::Planks,
            true,
        );
    }
    // The sluice gate's posts and head.
    let (tx, tw) = MR_TROUGH;
    for x in [tx - tw * 0.5 - 0.12, tx + tw * 0.5 + 0.12] {
        b.bx(
            Vec3::new(0.24, MR_GATE - top, 0.24),
            Vec3::new(x, (top + MR_GATE) * 0.5, hs + 0.14),
            Mat::Timber,
        );
    }
    b.bx(
        Vec3::new(tw + 0.6, 0.3, 0.34),
        Vec3::new(tx, MR_GATE, hs + 0.14),
        Mat::Timber,
    );
    // The roof's posts at the deck's corners (the roof's out of reach).
    let rp = half - 0.2;
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            b.bx(
                Vec3::new(0.22, MR_ROOF - top, 0.22),
                Vec3::new(sx * rp, (top + MR_ROOF) * 0.5, sz * rp),
                Mat::Timber,
            );
        }
    }
    // The walkway, its handrail, and the trough beside it (which keeps
    // you off the other side).
    let (z0, z1) = MR_WALK_TOP;
    walk(
        b,
        Vec3::new(wx, mr_walk_y(z0), z0),
        Vec3::new(wx, mr_walk_y(z1), z1),
        ww,
        0.12,
        Mat::Planks,
    );
    let rx = wx + ww * 0.5 + 0.05;
    strut(
        b,
        Vec3::new(rx, mr_walk_y(z0) + 0.5, z0),
        Vec3::new(rx, mr_walk_y(z1) + 0.5, z1),
        0.06,
        1.1,
        Mat::Timber,
    );
    let wz1 = MR_WHEEL.2 - 1.2;
    let zk = z0 + (top - (MR_LEVEL - 0.6)) * (z1 - z0) / (top - 0.3);
    for (za, zb) in [(hs + 0.1, zk), (zk, wz1)] {
        let (ya, yb) = (mr_trough_y(za), mr_trough_y(zb));
        strut(
            b,
            Vec3::new(tx, ya + 0.32, za),
            Vec3::new(tx, yb + 0.32, zb),
            tw,
            0.88,
            Mat::Planks,
        );
    }
    // The bents, every 4 m: two posts, and a stub up from them to the
    // trough.
    let mut z = 6.0;
    while z < wz1 - 1.0 {
        let cap = (mr_walk_y(z.min(z1)) - 0.27).min(mr_trough_y(z) - 0.18);
        for x in [tx - tw * 0.5 - 0.1, wx + ww * 0.5 + 0.05] {
            b.bx(
                Vec3::new(0.22, cap + 0.95, 0.22),
                Vec3::new(x, (cap - 1.05) * 0.5, z),
                Mat::Timber,
            );
        }
        let under = mr_trough_y(z) - 0.12;
        if under > cap + 0.05 {
            b.bx(
                Vec3::new(0.2, under - cap + 0.1, 0.2),
                Vec3::new(tx, (under + cap - 0.1) * 0.5, z),
                Mat::Timber,
            );
        }
        z += 4.0;
    }
    // The wheel house round the wheel's foot: the tailrace behind a curb,
    // a dry floor on the -X side, a doorway at the +Z end of that wall.
    let (wr, wy, wz) = MR_WHEEL;
    let ((px0, px1), pz) = (MR_PIT, wr + 0.5);
    let (pcx, pw) = ((px0 + px1) * 0.5, px1 - px0);
    let fl = MR_FLOOR;
    b.bx(
        Vec3::new(pw + 0.8, 0.3, 2.0 * pz + 0.8),
        Vec3::new(pcx, fl - 0.15, wz),
        Mat::Concrete,
    );
    let (wh, wy0) = (MR_WALL_TOP - fl, (MR_WALL_TOP + fl) * 0.5);
    let (door0, door1) = (wz + pz - 1.65, wz + pz - 0.25);
    for (z0, z1) in [(wz - pz - 0.4, door0), (door1, wz + pz + 0.4)] {
        b.bx(
            Vec3::new(0.4, wh, z1 - z0),
            Vec3::new(px0 - 0.2, wy0, (z0 + z1) * 0.5),
            Mat::Concrete,
        );
    }
    b.bx(
        Vec3::new(0.4, wh, 2.0 * pz + 0.8),
        Vec3::new(px1 + 0.2, wy0, wz),
        Mat::Concrete,
    );
    for sz in [-1.0f32, 1.0] {
        b.bx(
            Vec3::new(pw, wh, 0.4),
            Vec3::new(pcx, wy0, wz + sz * (pz + 0.2)),
            Mat::Concrete,
        );
    }
    b.bx(
        Vec3::new(0.2, 0.55, 2.0 * pz),
        Vec3::new(MR_CURB - 0.1, fl + 0.275, wz),
        Mat::Concrete,
    );
    b.piece(
        Shape::Cylinder(wr, tw),
        Transform::from_xyz(tx, wy, wz)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        Mat::Timber,
        true,
    );
    for p in &mut b.geo.pieces[from..] {
        p.at.translation.z += MR_SHIFT;
    }
    b.dress_landmark(
        from,
        "mill_race",
        Transform::from_xyz(0.0, 0.0, MR_SHIFT),
        ["mill_race_lod1", "mill_race_lod2"],
    );
    // Up on the deck, for whoever climbs, in its corner clear of the
    // tank.
    b.crate_box(Vec3::new(-2.7, top, -2.7 + MR_SHIFT), 0.1, Some("tools"));
    // In the wheel house by the wheel, the millwright's chest.
    b.furnish(
        "tool_chest",
        Vec3::new(0.9, 0.5, 0.5),
        Vec3::new(-2.7, MR_FLOOR, wz - 1.2 + MR_SHIFT),
        std::f32::consts::FRAC_PI_2,
        Some("tools"),
    );
    // The gatekeeper's hut between the tower's feet: plank walls, a
    // doorway on +X, a ceiling under the girts; his winch for hauling up
    // to the deck, a bench, rope, a lantern.
    let (hs, y0, hh) = (2.25, 0.2, 2.6);
    let at = |x: f32, z: f32| Vec2::new(x, z + MR_SHIFT);
    b.bx(
        Vec3::new(2.0 * hs, 0.2, 2.0 * hs),
        Vec3::new(0.0, 0.1, MR_SHIFT),
        Mat::WoodFloor,
    );
    let walls: [(Vec2, Vec2, &[Opening]); 4] = [
        (at(-hs, -hs), at(hs, -hs), &[window(hs)]),
        (at(hs, -hs), at(hs, hs), &[self::gap(hs, 0.9, 2.0)]),
        (at(hs, hs), at(-hs, hs), &[]),
        (at(-hs, hs), at(-hs, -hs), &[]),
    ];
    for (a, c, ops) in walls {
        b.wall(a, c, y0, hh, 0.12, Mat::Planks, ops);
    }
    b.bx(
        Vec3::new(2.0 * hs + 0.12, 0.1, 2.0 * hs + 0.12),
        Vec3::new(0.0, y0 + hh + 0.05, MR_SHIFT),
        Mat::Planks,
    );
    b.workbench(
        Vec3::new(-hs + 0.5, y0, 0.3 + MR_SHIFT),
        std::f32::consts::FRAC_PI_2,
        Some("tools"),
    );
    b.furnish(
        "winch",
        Vec3::new(1.2, 1.0, 0.8),
        Vec3::new(0.6, y0, -hs + 0.7 + MR_SHIFT),
        0.0,
        None,
    );
    b.strew(
        "rope_coil",
        Vec3::new(0.7, 0.12, 0.7),
        Vec3::new(1.2, y0, hs - 0.6 + MR_SHIFT),
        0.4,
    );
}

// The ropeway's measures, as `art/props/heroes.py` (RW_*, PY_* there).
// The station's prefab origin is its footprint's middle, RW_SHIFT from
// the station's own.
const RW_SHIFT: Vec3 = Vec3::new(-1.3, 0.0, -0.7);
const RW_PIER: (f32, f32) = (4.0, 3.5);
const RW_DECK: f32 = 9.0;
const RW_DECK_Z: (f32, f32) = (-6.5, 4.5);
const RW_SHED: (f32, f32, f32) = (8.0, 4.0, 7.0);
const RW_SHED_C: (f32, f32) = (-0.5, 1.0);
const RW_TOWER: (f32, f32, f32) = (1.6, 24.0, 7.0);
const RW_WHEEL: (f32, f32) = (2.6, -4.6);
const RW_STAIR_X: f32 = 5.6;
const PY_TOP: f32 = 20.0;
const PY_BASE: f32 = 2.0;
const PY_HEAD: f32 = 0.6;
const PY_ARM: f32 = 3.4;

/// Kettle Pass's hero landmark: the upper terminal of a mine's aerial
/// ropeway, its ropes broken. Up the stair to the loading deck, the bull
/// wheel and the shed (a crate in it); the tension tower behind. Drawn
/// by `ropeway_station` (`art/props/heroes.py`); its line's pylons are
/// `ropeway_pylon` and `ropeway_pylon_down`.
fn ropeway_station(b: &mut Builder) {
    let from = b.geo.pieces.len();
    let top = RW_DECK + 0.3;
    let (px, pz) = RW_PIER;
    let (zf, zb) = RW_DECK_Z;
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            b.bx(
                Vec3::new(0.8, RW_DECK + 0.2, 0.8),
                Vec3::new(sx * px, (RW_DECK - 0.8) * 0.5, sz * pz),
                Mat::Concrete,
            );
        }
        b.bx(
            Vec3::new(2.0 * px + 0.9, 0.5, 0.4),
            Vec3::new(0.0, RW_DECK - 0.25, sx * pz),
            Mat::RustyMetal,
        );
    }
    b.bx(
        Vec3::new(2.0 * px + 1.0, 0.3, zb - zf),
        Vec3::new(0.0, RW_DECK + 0.15, (zf + zb) * 0.5),
        Mat::Planks,
    );
    // The shed: back and side walls (a door in the +X one), open at the
    // front under a beam on a post; its roof.
    let (sw, sh, sd) = RW_SHED;
    let (sx0, sz0) = RW_SHED_C;
    let (hw, hd) = (sw * 0.5, sd * 0.5);
    let y = top + sh * 0.5;
    b.bx(
        Vec3::new(sw, sh, 0.15),
        Vec3::new(sx0, y, sz0 + hd - 0.075),
        Mat::Planks,
    );
    b.bx(
        Vec3::new(0.15, sh, sd - 0.15),
        Vec3::new(sx0 - hw + 0.075, y, sz0 - 0.075),
        Mat::Planks,
    );
    let (d0, d1) = (sz0 + 1.0, sz0 + 2.2);
    let x = sx0 + hw - 0.075;
    b.bx(
        Vec3::new(0.15, sh, d0 - (sz0 - hd)),
        Vec3::new(x, y, (sz0 - hd + d0) * 0.5),
        Mat::Planks,
    );
    b.bx(
        Vec3::new(0.15, sh, sz0 + hd - 0.15 - d1),
        Vec3::new(x, y, (d1 + sz0 + hd - 0.15) * 0.5),
        Mat::Planks,
    );
    b.bx(
        Vec3::new(0.15, sh - 2.1, d1 - d0),
        Vec3::new(x, top + 2.1 + (sh - 2.1) * 0.5, (d0 + d1) * 0.5),
        Mat::Planks,
    );
    b.bx(
        Vec3::new(sw - 0.3, 0.35, 0.3),
        Vec3::new(sx0, top + sh - 0.175, sz0 - hd + 0.15),
        Mat::Timber,
    );
    b.bx(
        Vec3::new(0.25, sh - 0.35, 0.25),
        Vec3::new(sx0, top + (sh - 0.35) * 0.5, sz0 - hd + 0.15),
        Mat::Timber,
    );
    b.piece(
        Shape::Prism(Vec3::new(sw + 1.0, 2.2, sd + 1.6)),
        Transform::from_xyz(sx0, top + sh, sz0 - 0.4),
        Mat::Corrugated,
        true,
    );
    // The bull wheel on its post.
    let (wr, wz) = RW_WHEEL;
    b.piece(
        Shape::Cylinder(0.35, 1.9),
        Transform::from_xyz(0.0, top + 0.95, wz),
        Mat::DarkMetal,
        true,
    );
    b.piece(
        Shape::Cylinder(wr, 0.24),
        Transform::from_xyz(0.0, top + 1.6, wz),
        Mat::DarkMetal,
        true,
    );
    // The tension tower behind the deck and its counterweight.
    let (th, ty, tz) = RW_TOWER;
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            b.bx(
                Vec3::new(0.6, 0.5, 0.6),
                Vec3::new(sx * th, 0.0, tz + sz * th),
                Mat::Concrete,
            );
            strut(
                b,
                Vec3::new(sx * th, 0.25, tz + sz * th),
                Vec3::new(sx * th * 0.75, ty - 0.3, tz + sz * th * 0.75),
                0.24,
                0.24,
                Mat::RustyMetal,
            );
        }
    }
    b.bx(
        Vec3::new(2.0 * th + 0.4, 0.3, 2.0 * th + 0.4),
        Vec3::new(0.0, ty - 0.15, tz),
        Mat::RustyMetal,
    );
    b.bx(
        Vec3::new(1.6, 2.2, 1.6),
        Vec3::new(0.0, 15.0, tz),
        Mat::Concrete,
    );
    b.bx(
        Vec3::new(0.08, ty - 16.25, 0.08),
        Vec3::new(0.0, (ty + 15.95) * 0.5, tz),
        Mat::DarkMetal,
    );
    // The stair: a flight up towards the front, the landing, a flight back
    // up beside the deck; handrails on their outer sides.
    let (xo, xi) = (RW_STAIR_X + 0.9, RW_STAIR_X - 0.4);
    let zl = zf + 2.2;
    for (a, c) in [
        (
            Vec3::new(xo, 0.0, zb + 0.5),
            Vec3::new(xo, top * 0.5, zl + 0.8),
        ),
        (
            Vec3::new(xi, top * 0.5, zl + 0.8),
            Vec3::new(xi, top, zb - 1.0),
        ),
    ] {
        walk(b, a, c, 1.1, 0.12, Mat::Planks);
        // From a metre back down the slope, into the ground or the landing.
        let r = Vec3::new(0.5, 0.5, 0.0);
        let back = (c - a).normalize();
        strut(b, a + r - back, c + r, 0.06, 1.0, Mat::Timber);
    }
    b.bx(
        Vec3::new(2.7, 0.15, 1.6),
        Vec3::new((xo + xi) * 0.5, top * 0.5 - 0.075, zl),
        Mat::Planks,
    );
    for x in [xi - 0.5, xo + 0.5] {
        b.bx(
            Vec3::new(0.2, top * 0.5 + 0.85, 0.2),
            Vec3::new(x, (top * 0.5 + 0.85) * 0.5 - 0.5, zl - 0.4),
            Mat::Timber,
        );
    }
    for p in &mut b.geo.pieces[from..] {
        p.at.translation += RW_SHIFT;
    }
    b.dress_landmark(
        from,
        "ropeway_station",
        Transform::from_translation(RW_SHIFT),
        ["ropeway_station_lod1", "ropeway_station_lod2"],
    );
    b.crate_box(
        Vec3::new(sx0 - hw + 0.7, top, sz0 + hd - 0.7) + RW_SHIFT,
        0.7,
        Some("tools"),
    );
    // The machine room: the motor drives the gearbox, whose shaft goes
    // out to the bull wheel; the brake on the line shaft; the engineman's
    // stand by the open front, facing down the line; his chest.
    let at = |x: f32, z: f32| Vec3::new(x, top, z) + RW_SHIFT;
    b.furnish(
        "gearbox",
        Vec3::new(1.0, 0.75, 0.85),
        at(0.0, 0.4),
        0.0,
        None,
    );
    b.furnish(
        "drive_motor",
        Vec3::new(1.7, 1.1, 0.8),
        at(1.45, 0.4),
        std::f32::consts::PI,
        None,
    );
    b.furnish(
        "brake_drum",
        Vec3::new(1.8, 1.4, 1.0),
        at(-1.7, 0.4),
        0.0,
        None,
    );
    b.furnish(
        "control_stand",
        Vec3::new(0.9, 1.4, 0.5),
        at(-3.3, -1.4),
        std::f32::consts::PI,
        None,
    );
    b.furnish(
        "tool_chest",
        Vec3::new(0.9, 0.5, 0.5),
        at(2.5, 3.8),
        0.0,
        Some("workshop"),
    );
    b.strew("rope_coil", Vec3::new(0.7, 0.12, 0.7), at(-2.6, 3.4), 1.1);
    b.barrel(at(-3.9, 1.6));
}

/// A pylon's colliders, standing at the origin, then `at`: its legs, the
/// crossarm on its head. `lying`, two girts hold the upper legs up.
fn pylon_body(b: &mut Builder, at: Transform, lying: bool) {
    let h = |y: f32| PY_BASE + (PY_HEAD - PY_BASE) * (y - 0.25) / (PY_TOP - 0.25);
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            let a = at.transform_point(Vec3::new(sx * h(0.25), 0.25, sz * h(0.25)));
            let c = at.transform_point(Vec3::new(sx * h(PY_TOP), PY_TOP, sz * h(PY_TOP)));
            strut(b, a, c, 0.2, 0.2, Mat::RustyMetal);
        }
    }
    b.piece(
        Shape::Box(Vec3::new(1.4, 0.3, 1.4)),
        at * Transform::from_xyz(0.0, PY_TOP - 0.15, 0.0),
        Mat::RustyMetal,
        true,
    );
    // Lying, the crossarm's pressed down into the grass.
    let sunk = if lying { -0.3 } else { 0.0 };
    b.piece(
        Shape::Box(Vec3::new(2.0 * PY_ARM, 0.4, 0.3)),
        at * Transform::from_xyz(0.0, PY_TOP + 0.2, sunk),
        Mat::RustyMetal,
        true,
    );
    if lying {
        // Halfway along, from the lower legs (-z) to under the upper ones.
        let y = (0.25 + PY_TOP) * 0.5;
        for sx in [-1.0f32, 1.0] {
            let a = at.transform_point(Vec3::new(sx * h(y), y, -h(y)));
            let c = at.transform_point(Vec3::new(sx * h(y), y, h(y) - 0.1));
            strut(b, a, c, 0.08, 0.08, Mat::RustyMetal);
        }
    }
}

fn pylon_footings(b: &mut Builder) {
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            b.bx(
                Vec3::new(0.9, 0.6, 0.9),
                Vec3::new(sx * PY_BASE, 0.0, sz * PY_BASE),
                Mat::Concrete,
            );
        }
    }
}

/// One of the ropeway's lattice pylons, 21 m, its crossarm along x (the
/// line runs along z), the ropes broken and hanging.
fn ropeway_pylon(b: &mut Builder) {
    let from = b.geo.pieces.len();
    pylon_footings(b);
    pylon_body(b, Transform::IDENTITY, false);
    b.dress_landmark(
        from,
        "ropeway_pylon",
        Transform::IDENTITY,
        ["ropeway_pylon_lod1", "ropeway_pylon_lod2"],
    );
    // The ladder up the -Z +X leg's face (art/props/heroes.py): it leans in
    // with the leg; over the top onto the head box behind the crossarm.
    let leg = |y: f32| 2.0 + (0.6 - 2.0) * y / 20.0;
    b.ladder(
        Vec3::new(leg(0.3) - 0.25, 0.3, -leg(0.3) - 0.12),
        Vec3::new(leg(19.8) - 0.25, 19.8, -leg(19.8) - 0.12),
        0.4,
        Vec3::NEG_Z,
        Some(Vec3::new(0.3, 20.05, -0.45)),
    );
}

/// A pylon fallen along -z off its sheared legs, as `ropeway_pylon_down`
/// lies; the prefab's origin is its footprint's middle.
fn ropeway_pylon_down(b: &mut Builder) {
    // Its footings 11.5 m along z from its footprint's middle.
    let shift = Vec3::Z * 11.5;
    let from = b.geo.pieces.len();
    pylon_footings(b);
    let taper = ((PY_BASE - PY_HEAD) / PY_TOP).atan();
    let at = Transform::from_xyz(0.4, PY_BASE - 0.15, -PY_BASE - 2.5)
        * Transform::from_rotation(Quat::from_rotation_z(0.08))
        * Transform::from_rotation(Quat::from_rotation_x(
            -(std::f32::consts::FRAC_PI_2 + taper),
        ));
    pylon_body(b, at, true);
    for p in &mut b.geo.pieces[from..] {
        p.at.translation += shift;
    }
    b.dress_landmark(
        from,
        "ropeway_pylon_down",
        Transform::from_translation(shift),
        ["ropeway_pylon_down_lod1", "ropeway_pylon_down_lod2"],
    );
}

// The blast furnace's measures, as `art/props/heroes.py` (BF_* there), in
// its furnace's space; the prefab's origin is its footprint's middle,
// BF_SHIFT from the furnace.
const BF_SHIFT: Vec3 = Vec3::new(-5.7, 0.0, -10.6);
const BF_DECK: f32 = 24.0;
const BF_BAY: ((f32, f32), f32) = ((-6.0, -1.3), 7.4);
const BF_STOVES: (f32, [f32; 3], f32, f32) = (3.0, [12.0, 18.5, 25.0], 4.0, 20.0);
const BF_CHIMNEY: (f32, f32, (f32, f32), f32) = (1.4, 1.0, (18.5, 11.5), 38.0);
const BF_CATCHER: (f32, (f32, f32), f32, f32) = (2.8, (-14.0, 5.0), 6.0, 14.0);
const BF_SKIP: ((f32, f32), (f32, f32)) = ((0.0, 36.0), (0.0, 5.5));
const BF_WALK_X: f32 = 1.7;
const BF_CAST: (f32, (f32, f32), f32) = (16.0, (-4.0, -18.0), 9.0);

/// A solid upright cylinder from y0 to y1 at (x, z).
fn drum(b: &mut Builder, r: f32, x: f32, z: f32, y0: f32, y1: f32, mat: Mat) {
    b.piece(
        Shape::Cylinder(r, y1 - y0),
        Transform::from_xyz(x, (y0 + y1) * 0.5, z),
        mat,
        true,
    );
}

/// Cinder Works' hero landmark: a blast furnace, its stack to 26 m and
/// its bleeders to 33, three hot-blast stoves and their chimney, the
/// downcomer to the dust catcher. Walk into the cast house at its foot,
/// or up the walkway beside the skip bridge to the charging platform
/// (crates in both). Drawn by `blast_furnace` (`art/props/heroes.py`).
fn blast_furnace(b: &mut Builder) {
    let from = b.geo.pieces.len();
    b.bx(
        Vec3::new(14.0, 1.4, 14.0),
        Vec3::new(0.0, 0.5, 0.0),
        Mat::Concrete,
    );
    // The furnace, inside its outline: hearth, bosh, the stack in three,
    // the top cone, the top house.
    let rm = Mat::RustyMetal;
    drum(b, 3.4, 0.0, 0.0, 1.2, 6.0, rm);
    drum(b, 3.4, 0.0, 0.0, 6.0, 9.6, rm);
    for (r, y0, y1) in [(3.8, 9.6, 14.4), (3.4, 14.4, 19.2), (3.0, 19.2, 24.0)] {
        drum(b, r, 0.0, 0.0, y0, y1, rm);
    }
    drum(b, 1.4, 0.0, 0.0, 24.0, 26.0, rm);
    b.bx(
        Vec3::new(3.4, 3.2, 3.4),
        Vec3::new(0.0, 27.4, 0.0),
        Mat::Corrugated,
    );
    for k in 0..8 {
        let a = std::f32::consts::TAU * (k as f32 + 0.5) / 8.0;
        b.bx(
            Vec3::new(0.5, 7.9, 0.5),
            Vec3::new(4.75 * a.cos(), 5.15, 4.75 * a.sin()),
            rm,
        );
    }
    // The charging platform: a square deck round the stack, its railing
    // open at the +X +Z corner where the walkway arrives.
    let (dk, hole) = (BF_DECK, 3.0);
    let rim = 6.0 - hole;
    for (sx, sz, w, d) in [
        (0.0, -1.0, 12.0, rim),
        (0.0, 1.0, 12.0, rim),
        (-1.0, 0.0, rim, 2.0 * hole),
        (1.0, 0.0, rim, 2.0 * hole),
    ] {
        b.bx(
            Vec3::new(w, 0.3, d),
            Vec3::new(sx * (hole + rim * 0.5), dk - 0.15, sz * (hole + rim * 0.5)),
            Mat::PaintedMetal,
        );
    }
    // A bay off the -X +Z corner, out past the foundation: the crate's
    // there.
    let ((bx0, bx1), bz1) = BF_BAY;
    b.bx(
        Vec3::new(bx1 - bx0, 0.3, bz1 - 6.0),
        Vec3::new((bx0 + bx1) * 0.5, dk - 0.15, (6.0 + bz1) * 0.5),
        Mat::PaintedMetal,
    );
    let e = 5.95;
    let bo = bz1 - 0.05;
    for (a, c) in [
        (Vec2::new(-e, -e), Vec2::new(e, -e)),
        (Vec2::new(-e, -e + 0.04), Vec2::new(-e, bo - 0.04)),
        (Vec2::new(-e, bo), Vec2::new(bx1, bo)),
        (Vec2::new(bx1, bo - 0.04), Vec2::new(bx1, e + 0.04)),
        (Vec2::new(bx1 + 0.04, e), Vec2::new(0.9, e)),
        (Vec2::new(e, -e + 0.04), Vec2::new(e, 3.4)),
    ] {
        let mid = (a + c) * 0.5;
        let size = (c - a).abs() + Vec2::splat(0.06);
        b.bx(
            Vec3::new(size.x, 1.1, size.y),
            Vec3::new(mid.x, dk + 0.55, mid.y),
            Mat::PaintedMetal,
        );
    }
    // The uptakes, their crossing, the bleeders.
    for (x, z) in [(2.0, 0.0), (-2.0, 0.0), (0.0, 2.0), (0.0, -2.0)] {
        strut(
            b,
            Vec3::new(x * 0.9, 24.6, z * 0.9),
            Vec3::new(x * 1.15, 31.0, z * 1.15),
            0.8,
            0.8,
            rm,
        );
    }
    for (a, c) in [
        (Vec3::new(-2.3, 31.0, -2.3), Vec3::new(2.3, 31.0, 2.3)),
        (Vec3::new(2.3, 31.05, -2.3), Vec3::new(-2.3, 31.05, 2.3)),
    ] {
        strut(b, a, c, 0.9, 0.9, rm);
    }
    // The downcomer and the dust catcher on its legs.
    let (cr, (cx, cz), cy0, cy1) = BF_CATCHER;
    strut(
        b,
        Vec3::new(-2.3, 31.0, -2.3),
        Vec3::new(cx + 0.6, cy1 + 2.0, cz),
        1.4,
        1.4,
        rm,
    );
    drum(b, cr, cx, cz, cy0, cy1, rm);
    drum(b, 1.2, cx, cz, 2.2, cy0, rm);
    drum(b, 1.0, cx, cz, cy1, cy1 + 2.0, rm);
    for k in 0..4 {
        let a = std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_2 * k as f32;
        strut(
            b,
            Vec3::new(cx + (cr + 0.6) * a.cos(), 0.0, cz + (cr + 0.6) * a.sin()),
            Vec3::new(
                cx + (cr - 0.2) * a.cos(),
                cy0 + 0.5,
                cz + (cr - 0.2) * a.sin(),
            ),
            0.4,
            0.4,
            rm,
        );
    }
    // The stoves, the hot-blast main, the chimney.
    let (sr, xs, sz, sh) = BF_STOVES;
    for x in xs {
        drum(b, sr + 0.25, x, sz, 0.0, 0.8, rm);
        drum(b, sr, x, sz, 0.8, sh, rm);
        drum(b, sr * 0.8, x, sz, sh, sh + 1.5, rm);
        strut(
            b,
            Vec3::new(x, 8.0, sz - sr + 0.2),
            Vec3::new(x, 8.0, sz - sr - 1.1),
            0.9,
            0.9,
            rm,
        );
    }
    strut(
        b,
        Vec3::new(5.9, 8.0, 2.4),
        Vec3::new(xs[2] + 1.2, 8.0, sz - sr - 1.2),
        1.1,
        1.1,
        rm,
    );
    let (r0, _, (hx, hz), hh) = BF_CHIMNEY;
    drum(b, r0 + 0.4, hx, hz, 0.0, 1.0, Mat::Brick);
    drum(b, 1.3, hx, hz, 1.0, 19.0, Mat::Brick);
    drum(b, 1.05, hx, hz, 19.0, hh, Mat::Brick);
    // The skip bridge's chords, and the walkway beside it to the platform,
    // its handrail.
    let ((fx, fz), (tx, tz)) = BF_SKIP;
    let (ft, hd) = (Vec3::new(fx, 0.3, fz), Vec3::new(tx, BF_DECK + 2.5, tz));
    for sx in [-1.0f32, 1.0] {
        for dy in [0.0, -1.8] {
            let o = Vec3::new(sx * 1.0, dy, 0.0);
            strut(b, ft + o, hd + o, 0.32, 0.32, rm);
        }
    }
    let wx = BF_WALK_X;
    let (w0, w1) = (Vec3::new(wx, 0.3, fz - 1.0), Vec3::new(wx, BF_DECK, 5.6));
    walk(b, w0, w1, 1.0, 0.12, Mat::PaintedMetal);
    let back = (w1 - w0).normalize();
    let r = Vec3::new(0.47, 0.525, 0.0);
    // From two metres back down the slope, into the ground.
    strut(
        b,
        w0 + r - back * 2.0,
        w1 + r,
        0.05,
        1.05,
        Mat::PaintedMetal,
    );
    // The stockhouse pit and the hoist house.
    b.bx(
        Vec3::new(4.0, 0.6, 5.0),
        Vec3::new(fx, 0.0, fz + 1.5),
        Mat::Concrete,
    );
    let (hx_, hz_) = (7.0, 33.0);
    b.bx(
        Vec3::new(6.0, 4.5, 6.0),
        Vec3::new(hx_, 2.25, hz_),
        Mat::Brick,
    );
    b.piece(
        Shape::Prism(Vec3::new(6.8, 1.9, 6.8)),
        Transform::from_xyz(hx_, 4.5, hz_),
        Mat::Corrugated,
        true,
    );
    // The cast house: its floor, columns, sheeted upper sides, roof and
    // back gable; the ladle car.
    let (cw, (cz0, cz1), ce) = BF_CAST;
    let hw = cw * 0.5;
    b.bx(
        Vec3::new(cw + 0.6, 0.2, cz0 - cz1),
        Vec3::new(0.0, 0.1, (cz0 + cz1) * 0.5),
        Mat::Concrete,
    );
    for k in 0..5 {
        let z = cz0 + (cz1 - cz0) * k as f32 / 4.0;
        for sx in [-1.0f32, 1.0] {
            b.bx(
                Vec3::new(0.4, ce, 0.4),
                Vec3::new(sx * hw, 0.2 + ce * 0.5, z),
                rm,
            );
        }
    }
    for sx in [-1.0f32, 1.0] {
        b.bx(
            Vec3::new(0.1, ce - 4.0, cz0 - cz1),
            Vec3::new(sx * (hw + 0.25), 4.2 + (ce - 4.0) * 0.5, (cz0 + cz1) * 0.5),
            Mat::Corrugated,
        );
    }
    b.piece(
        Shape::Prism(Vec3::new(cw + 1.2, 3.0, cz0 - cz1 + 1.0)),
        Transform::from_xyz(0.0, ce + 0.2, (cz0 + cz1) * 0.5),
        Mat::Corrugated,
        true,
    );
    let lz = cz1 + 2.2;
    b.bx(Vec3::new(2.4, 0.8, 3.2), Vec3::new(0.0, 0.9, lz), rm);
    drum(b, 1.5, 0.0, lz, 1.3, 3.5, Mat::DarkMetal);
    for p in &mut b.geo.pieces[from..] {
        p.at.translation += BF_SHIFT;
    }
    b.dress_landmark(
        from,
        "blast_furnace",
        Transform::from_translation(BF_SHIFT),
        ["blast_furnace_lod1", "blast_furnace_lod2"],
    );
    // The cast house floor: the clay gun on the foundation's edge aimed at
    // the taphole, the tapping bars racked between columns, a sand bed of
    // pigs by the runner, the crew's chest.
    let fat = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) + BF_SHIFT;
    b.furnish(
        "mud_gun",
        Vec3::new(0.8, 1.4, 2.8),
        fat(2.0, 1.2, -5.9),
        std::f32::consts::PI,
        None,
    );
    b.furnish(
        "bar_rack",
        Vec3::new(2.0, 2.2, 0.4),
        fat(-7.6, 0.2, -10.0),
        -std::f32::consts::FRAC_PI_2,
        None,
    );
    b.strew(
        "pig_bed",
        Vec3::new(3.0, 0.2, 2.0),
        fat(3.0, 0.2, -12.0),
        0.0,
    );
    b.furnish(
        "tool_chest",
        Vec3::new(0.9, 0.5, 0.5),
        fat(-5.5, 0.2, -8.0),
        0.3,
        Some("workshop"),
    );
    // The stoves' cold-blast valves at their feet, a chest by the middle
    // one.
    for x in xs {
        b.furnish(
            "gate_valve",
            Vec3::new(1.3, 2.4, 1.1),
            fat(x, 0.0, sz - sr - 0.8),
            std::f32::consts::FRAC_PI_2,
            None,
        );
    }
    b.furnish(
        "tool_chest",
        Vec3::new(0.9, 0.5, 0.5),
        fat(xs[1] + 1.8, 0.0, sz - sr - 0.9),
        0.0,
        Some("tools"),
    );
    // A crate in the platform's bay, one in the cast house.
    b.crate_box(
        Vec3::new(-3.6, BF_DECK, 6.75) + BF_SHIFT,
        0.7,
        Some("workshop"),
    );
    b.crate_box(Vec3::new(5.5, 0.2, -12.0) + BF_SHIFT, 0.8, Some("tools"));
}

/// A railway signal box: an enterable brick ground floor (its lever frame,
/// a desk), the glazed cab above it up an outside stair.
fn signal_box(b: &mut Builder) {
    let (w, d, f, h) = (6.0, 3.6, 0.3, 2.7);
    b.plinth(w, d, f, Mat::Concrete, Mat::WoodFloor);
    b.shell(
        w,
        d,
        f,
        h,
        0.3,
        Mat::Brick,
        [
            &[door(1.5), window(4.3)],
            &[],
            &[window(1.5), window(4.5)],
            &[window(1.8)],
        ],
    );
    // The cab (drawn by its model, `art/props/landmarks.py`) on a slab
    // over the brick, its roof, and its stair as a ramp up the +X end.
    let from = b.geo.pieces.len();
    b.bx(
        Vec3::new(w + 0.6, 0.15, d + 0.6),
        Vec3::new(0.0, f + h + 0.075, 0.0),
        Mat::Concrete,
    );
    let floor = f + h + 0.15;
    b.bx(
        Vec3::new(w, 2.25, d),
        Vec3::new(0.0, floor + 1.125, 0.0),
        Mat::Planks,
    );
    b.piece(
        Shape::Prism(Vec3::new(w + 0.8, 1.4, d + 0.8)),
        Transform::from_xyz(0.0, floor + 2.25, 0.0),
        Mat::Corrugated,
        true,
    );
    let (a, c) = (
        Vec3::new(w * 0.5 + 0.65, 0.0, -d * 0.5 + 0.1),
        Vec3::new(w * 0.5 + 0.65, floor - 0.15, 0.5),
    );
    let along = c - a;
    b.piece(
        Shape::Box(Vec3::new(0.9, 0.08, along.length())),
        Transform::from_translation((a + c) * 0.5 - Vec3::Y * 0.1)
            .with_rotation(Quat::from_rotation_x(-along.y.atan2(along.z))),
        Mat::Planks,
        true,
    );
    b.bx(
        Vec3::new(0.9, 0.1, 1.2),
        Vec3::new(w * 0.5 + 0.65, floor - 0.16, 1.1),
        Mat::Planks,
    );
    b.dress_structure(from, vec![("signal_box", Transform::IDENTITY)]);
    // Inside: the lever frame along the back wall, a desk and a shelf.
    let from = b.geo.pieces.len();
    let lever = Vec3::new(0.5, f, d * 0.5 - 0.15 - 0.3);
    b.bx(
        Vec3::new(2.44, 1.0, 0.5),
        lever + Vec3::new(0.0, 0.5, 0.04),
        Mat::DarkMetal,
    );
    b.dress(
        from,
        vec![("lever_frame", Transform::from_translation(lever))],
    );
    b.table(Vec3::new(-1.9, f, 1.0), 0.0, Some("house"));
    b.chair(Vec3::new(-1.9, f, 0.4), std::f32::consts::PI);
    b.shelf(
        Vec3::new(-w * 0.5 + 0.35, f, -0.6),
        std::f32::consts::FRAC_PI_2,
        Some("tools"),
    );
}

/// A village chapel: a whitewashed nave you can go into (pews, an altar
/// table) under a green iron roof, its bell tower at the east end.
fn chapel(b: &mut Builder) {
    let (w, d, f, h) = (10.0, 14.0, 0.5, 4.5);
    let tall = |c: f32| Opening {
        centre: c,
        width: 1.0,
        bottom: 1.5,
        top: 3.6,
        kind: OpeningKind::Window,
    };
    b.plinth(w, d, f, Mat::Concrete, Mat::WoodFloor);
    b.shell(
        w,
        d,
        f,
        h,
        0.35,
        Mat::Plaster,
        [
            &[door(w * 0.5)],
            &[tall(3.0), tall(7.0), tall(11.0)],
            &[],
            &[tall(3.0), tall(7.0), tall(11.0)],
        ],
    );
    b.gable_roof_with(
        w + 0.4,
        d + 0.4,
        f + h,
        38.0,
        0.5,
        Mat::PaintedMetal,
        Mat::Plaster,
        true,
    );
    // Pews either side of the aisle, the altar table at the east end.
    for k in 0..5 {
        let z = -d * 0.5 + 3.0 + k as f32 * 1.1;
        for x in [-2.6, 2.6] {
            b.pew(Vec3::new(x, f, z), std::f32::consts::PI);
        }
    }
    b.table(Vec3::new(0.0, f, d * 0.5 - 1.6), 0.0, Some("house"));
    b.shelf(
        Vec3::new(-w * 0.5 + 0.45, f, d * 0.5 - 1.0),
        std::f32::consts::FRAC_PI_2,
        Some("medical"),
    );
    // The bell tower against the east wall, on a foundation as deep as the
    // nave's (it's lifted with it on a slope), its door turned outward.
    let at = Vec3::new(0.0, 0.0, d * 0.5 + 0.175 + 2.0);
    b.bx(Vec3::new(4.2, 1.5, 4.2), at - Vec3::Y * 0.75, Mat::Concrete);
    let from = b.geo.pieces.len();
    let mut tower = Builder::default();
    steeple(&mut tower);
    let turn =
        Transform::from_translation(at).with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
    for mut p in tower.geo.pieces {
        p.at = turn * p.at;
        b.geo.pieces.push(p);
    }
    b.dress_structure(from, vec![("steeple", turn)]);
}

/// A chapel's bell tower: brick, a whitewashed belfry, an onion dome.
fn steeple(b: &mut Builder) {
    let from = b.geo.pieces.len();
    b.bx(
        Vec3::new(4.2, 0.5, 4.2),
        Vec3::new(0.0, 0.25, 0.0),
        Mat::Concrete,
    );
    b.bx(
        Vec3::new(4.0, 7.6, 4.0),
        Vec3::new(0.0, 4.3, 0.0),
        Mat::Brick,
    );
    column(b, Shape::Cylinder(1.9, 4.3), 8.0, Mat::Plaster);
    column(b, Shape::Cylinder(1.5, 4.7), 12.3, Mat::PaintedMetal);
    b.piece(
        Shape::Box(Vec3::new(0.1, 2.0, 0.1)),
        Transform::from_xyz(0.0, 18.0, 0.0),
        Mat::RustyMetal,
        false,
    );
    b.dress_structure(from, vec![("steeple", Transform::IDENTITY)]);
}

/// A works chimney: a tapering brick stack banded with iron, a red lamp
/// on top. You see it from anywhere in the valley the fog allows.
fn chimney(b: &mut Builder) {
    b.bx(
        Vec3::new(5.0, 2.0, 5.0),
        Vec3::new(0.0, 1.0, 0.0),
        Mat::Concrete,
    );
    let (segs, total) = (6, 36.0);
    let h = total / segs as f32;
    let mut y = 2.0;
    for i in 0..segs {
        let r = 2.1 - 0.13 * i as f32;
        b.piece(
            Shape::Cylinder(r, h),
            Transform::from_xyz(0.0, y + h * 0.5, 0.0),
            Mat::Brick,
            true,
        );
        b.piece(
            Shape::Cylinder(r + 0.07, 0.3),
            Transform::from_xyz(0.0, y + h, 0.0),
            Mat::RustyMetal,
            false,
        );
        y += h;
    }
    b.geo.lights.push(Vec3::new(0.0, y + 0.4, 0.0));
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

/// A dead bus standing on its wheels (one gone, that corner on a block):
/// body, a band of dark windows, roof, bumpers, wheel arches, a door.
/// A car left where it stopped (`art/vehicles/wrecks.py`, Kit's): its
/// shell drawn, and collided as a few boxes that follow it, every door, bonnet, boot and
/// wheel its own breakable (`destruction::strip::car_parts`), in the
/// model's own space (the ground under the wheelbase's middle, its front
/// to -Z).
fn car_wreck(b: &mut Builder, model: &'static str) {
    let body = crate::destruction::strip::car_body(model);
    if body.is_empty() {
        return;
    }
    let from = b.geo.pieces.len();
    // (Boxes that follow the body, clear of its parts: Destruction's.)
    for &(centre, size) in &body {
        b.piece(
            Shape::Box(size),
            Transform::from_translation(centre),
            Mat::RustyMetal,
            true,
        );
    }
    // What the shell stands on with its wheels gone: the lowest box's
    // middle down to the ground, inside the track and between the axles
    // (so it never touches a wheel), hidden under the body.
    if let Some(&(centre, size)) = body
        .iter()
        .min_by(|a, c| (a.0.y - a.1.y * 0.5).total_cmp(&(c.0.y - c.1.y * 0.5)))
    {
        let pan = centre.y - size.y * 0.5;
        if pan > 0.0 {
            b.piece(
                Shape::Box(Vec3::new(size.x * 0.6, pan, size.z * 0.44)),
                Transform::from_xyz(centre.x, pan * 0.5, centre.z),
                Mat::RustyMetal,
                true,
            );
        }
    }
    let to = b.geo.pieces.len();
    let body: &'static str = match model {
        "wreck_lada" => "wreck_lada#body",
        "wreck_uaz" => "wreck_uaz#body",
        _ => "wreck_lorry#body",
    };
    b.geo.props.push(PropDef {
        models: vec![(body, Transform::IDENTITY)],
        pieces: from..to,
        inside: false,
        shadow: true,
        lods: None,
    });
    b.geo
        .breakables
        .extend(crate::destruction::strip::car_parts(
            model,
            Transform::IDENTITY,
        ));
}

pub(super) fn wrecked_bus(b: &mut Builder, floor: Vec3) {
    let (w, len) = (2.5, 10.0);
    let wheel_r = 0.48;
    let body_lo = floor.y + wheel_r + 0.15;
    // Lower body (the skirt and panels below the windows): a core, and
    // its side panels each a thing to prise off (`Breakable`, for scrap
    // and cover), the core still solid behind.
    let skin = 0.03;
    // (Bare, rusted under its skin: a panel prised off shows it.)
    b.bx(
        Vec3::new(w - 2.0 * skin, 1.05, len),
        floor + Vec3::new(0.0, body_lo + 0.525 - floor.y, 0.0),
        Mat::RustyMetal,
    );
    let door_z = -len * 0.5 + 1.0;
    for side in [-1.0f32, 1.0] {
        // The right side's front panel starts behind the door.
        let z0 = if side > 0.0 {
            door_z + 0.55
        } else {
            -len * 0.5 + 0.15
        };
        let spans = [(z0, -0.05), (0.05, len * 0.5 - 0.15)];
        for (a, c) in spans {
            let mid = floor
                + Vec3::new(
                    side * (w * 0.5 - skin * 0.5),
                    body_lo + 0.525 - floor.y,
                    (a + c) * 0.5,
                );
            // Its outer face to local -Z: facing out of the side.
            let face = Quat::from_rotation_y(-side * std::f32::consts::FRAC_PI_2);
            b.geo.breakables.push(Breakable {
                kind: BreakKind::BusPanel,
                at: Transform::from_translation(mid).with_rotation(face),
                size: Vec3::new(c - a, 1.0, skin),
                model: None,
                mat: Mat::PaintedMetal,
                yields: Some("bus_panel"),
            });
        }
        // Under the door, the skirt's core still reaches the side.
        if side > 0.0 {
            b.bx(
                Vec3::new(skin, 1.05, 0.55 + 0.5),
                floor + Vec3::new(w * 0.5 - skin * 0.5, body_lo + 0.525 - floor.y, door_z),
                Mat::PaintedMetal,
            );
        }
    }
    // Window band: dark glass set in a little, pillars between.
    let band = body_lo + 1.05;
    b.piece(
        Shape::Box(Vec3::new(w - 0.06, 0.9, len - 0.3)),
        Transform::from_translation(floor + Vec3::new(0.0, band + 0.45 - floor.y, 0.0)),
        Mat::Glass,
        false,
    );
    for k in 0..7 {
        let z = -len * 0.5 + 0.15 + k as f32 * (len - 0.3) / 6.0;
        b.bx(
            Vec3::new(w, 0.9, 0.12),
            floor + Vec3::new(0.0, band + 0.45 - floor.y, z),
            Mat::PaintedMetal,
        );
    }
    // Roof, a little proud all round.
    b.bx(
        Vec3::new(w + 0.04, 0.16, len + 0.04),
        floor + Vec3::new(0.0, band + 0.98 - floor.y, 0.0),
        Mat::PaintedMetal,
    );
    // Bumpers.
    for sgn in [-1.0, 1.0] {
        b.bx(
            Vec3::new(w + 0.06, 0.22, 0.14),
            floor + Vec3::new(0.0, body_lo + 0.15 - floor.y, sgn * (len * 0.5 + 0.05)),
            Mat::DarkMetal,
        );
    }
    // A folding door on the right, front: a thing to prise off.
    b.geo.breakables.push(Breakable {
        kind: BreakKind::BusDoor,
        at: Transform::from_translation(
            floor + Vec3::new(w * 0.5 + 0.02, body_lo + 0.95 - floor.y, door_z),
        )
        .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
        size: Vec3::new(0.95, 1.85, 0.04),
        model: None,
        mat: Mat::DarkMetal,
        yields: Some("scrap_metal"),
    });
    // Wheels: three corners on tyres, the fourth on a block.
    for (i, (x, z)) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
        .iter()
        .enumerate()
    {
        let at = floor + Vec3::new(x * (w * 0.5 - 0.25), wheel_r, z * (len * 0.5 - 1.9));
        if i == 3 {
            b.bx(
                Vec3::new(0.4, body_lo - floor.y, 0.5),
                Vec3::new(at.x, floor.y + (body_lo - floor.y) * 0.5, at.z),
                Mat::Concrete,
            );
            continue;
        }
        b.piece(
            Shape::Cylinder(wheel_r, 0.32),
            Transform::from_translation(at)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            Mat::DarkMetal,
            true,
        );
    }
}

fn ranger_tower(b: &mut Builder) {
    let top = tower_legs(b);
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

/// The survey post: the ranger tower's legs, stair and platform, but open
/// on top, a railing round it, and the survey mast with its beacon (art
/// `survey_mast`): unlike the ranger tower's roofed cabin from afar. Its
/// entrance is where the stair arrives (the ranger tower's door).
fn survey_tower(b: &mut Builder) {
    let top = tower_legs(b);
    let deck = top + 0.075;
    // The railing: posts and two rails round the edge, open where the
    // stair arrives (+X, z 1.2).
    let e = 2.1;
    let rail = |b: &mut Builder, a: Vec2, c: Vec2| {
        let v = c - a;
        let mid = (a + c) * 0.5;
        let yaw = -v.y.atan2(v.x);
        for (y, t) in [(1.0, 0.07), (0.5, 0.05)] {
            b.bx_rot(
                Vec3::new(v.length(), t, 0.05),
                Vec3::new(mid.x, deck + y, mid.y),
                yaw,
                Mat::Timber,
            );
        }
    };
    let corners = [
        Vec2::new(-e, -e),
        Vec2::new(e, -e),
        Vec2::new(e, e),
        Vec2::new(-e, e),
    ];
    rail(b, corners[3], corners[0]);
    rail(b, corners[0], corners[1]);
    rail(b, corners[2], corners[3]);
    // The +X side, broken for the stair's arrival.
    rail(b, Vec2::new(e, -e), Vec2::new(e, 1.2 - 0.6));
    rail(b, Vec2::new(e, 1.2 + 0.6), Vec2::new(e, e));
    for c in corners
        .into_iter()
        .chain([Vec2::new(e, 0.6), Vec2::new(e, 1.8)])
    {
        b.bx(
            Vec3::new(0.08, 1.05, 0.08),
            Vec3::new(c.x, deck + 0.525, c.y),
            Mat::Timber,
        );
    }
    let r = |v: f32| (v * 100.0).round() / 100.0 + 0.0;
    b.geo.entrances.push(extraction_core::map::Entrance {
        at: [r(2.04), 1.2],
        out: [1.0, 0.0],
        width: 0.95,
        clear: 1.0,
    });
    // The mast, its collider a pole; the model draws it and the beacon.
    let from = b.geo.pieces.len();
    b.bx(
        Vec3::new(0.24, 5.4, 0.24),
        Vec3::new(0.0, deck + 2.7, 0.0),
        Mat::PaintedMetal,
    );
    b.dress_structure(
        from,
        vec![("survey_mast", Transform::from_xyz(0.0, deck, 0.0))],
    );
    b.table(Vec3::new(-0.9, deck, 1.1), 0.0, Some("gadgets"));
}

/// The ranger tower's four legs, its stair in three flights up the +X
/// side and its platform: the platform's top, less its boards.
fn tower_legs(b: &mut Builder) -> f32 {
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
        // Stringers either side of the flight, their top edge along the
        // treads' undersides, from below the first step to the last one:
        // nothing stands proud of a tread or the landing to trip on.
        let (first, last) = (-1.6, -1.6 + (steps - 1) as f32 * 3.2 / steps as f32);
        let slope = (per - per / steps as f32) / (last - first);
        let cos = (1.0 + slope * slope).sqrt().recip();
        let drop = 0.06 + 0.11 / cos;
        let line = |z: f32| y0 + per / steps as f32 + (z - first) * slope - drop;
        let (z0, z1) = (first - 0.25, last);
        let (a, bb) = (
            Vec3::new(0.0, line(z0), side * z0),
            Vec3::new(0.0, line(z1), side * z1),
        );
        let along = bb - a;
        let tilt = Quat::from_rotation_x(-(along.y).atan2(along.z));
        for x in [2.01, 2.99] {
            b.piece(
                Shape::Box(Vec3::new(0.06, 0.22, along.length())),
                Transform::from_translation(Vec3::new(x, (a.y + bb.y) * 0.5, (a.z + bb.z) * 0.5))
                    .with_rotation(tilt),
                Mat::Timber,
                false,
            );
        }
        b.bx(
            Vec3::new(0.9, 0.08, 1.0),
            Vec3::new(2.5, y0 + per - 0.04, side * 1.9),
            Mat::Planks,
        );
        // The landing on two posts, tied back to the leg; the top landing,
        // whose posts would rise past the one below, hangs on knee braces
        // from the leg instead.
        let hi = y0 + per - 0.08;
        if k < 2 {
            for x in [2.1, 2.9] {
                b.bx(
                    Vec3::new(0.12, hi, 0.12),
                    Vec3::new(x, hi * 0.5, side * 2.3),
                    Mat::Timber,
                );
            }
        } else {
            // From the leg's outer face up to the tie beam's far end.
            let (from, to) = (
                Vec3::new(1.88, hi - 1.0, side * 1.8),
                Vec3::new(2.85, hi - 0.17, side * 1.8),
            );
            let d = to - from;
            b.piece(
                Shape::Box(Vec3::new(0.1, 0.1, d.length())),
                Transform::from_translation((from + to) * 0.5).looking_to(d, Vec3::Z),
                Mat::Timber,
                false,
            );
        }
        // (Under the landing, out of reach: no collider to snag the
        // floor plan.)
        b.piece(
            Shape::Box(Vec3::new(1.3, 0.12, 0.12)),
            Transform::from_xyz(2.3, y0 + per - 0.14, side * 1.8),
            Mat::Timber,
            false,
        );
    }
    b.bx(
        Vec3::new(4.4, 0.15, 4.4),
        Vec3::new(0.0, top, 0.0),
        Mat::Planks,
    );
    top
}

fn quarantine_gate(b: &mut Builder) {
    // Concrete blocks either side of the road and the barrier arm, raised.
    for x in [-5.2, 5.2] {
        let from = b.geo.pieces.len();
        b.bx(
            Vec3::new(1.4, 1.2, 1.4),
            Vec3::new(x, 0.6, 0.0),
            Mat::Concrete,
        );
        b.dress_structure(from, vec![("road_block", Transform::from_xyz(x, 0.0, 0.0))]);
        let from = b.geo.pieces.len();
        b.bx(
            Vec3::new(0.25, 3.5, 0.25),
            Vec3::new(x.signum() * 4.3, 1.75, 0.0),
            Mat::PaintedMetal,
        );
        b.dress_structure(
            from,
            vec![("gate_post", Transform::from_xyz(x.signum() * 4.3, 0.0, 0.0))],
        );
    }
    // The barrier arm, raised, pivoting on the left post: a pin through
    // the post and a counterweight below the pivot.
    let from = b.geo.pieces.len();
    let pivot = Vec3::new(-4.3, 1.1, 0.25);
    let lean = 0.15f32;
    let dir = Vec3::new(-lean.sin(), lean.cos(), 0.0);
    b.piece(
        Shape::Box(Vec3::new(0.12, 6.0, 0.12)),
        Transform::from_translation(pivot + dir * 2.9).with_rotation(Quat::from_rotation_z(lean)),
        Mat::PaintedMetal,
        false,
    );
    b.piece(
        Shape::Box(Vec3::new(0.3, 0.45, 0.2)),
        Transform::from_translation(pivot - dir * 0.3).with_rotation(Quat::from_rotation_z(lean)),
        Mat::Concrete,
        false,
    );
    b.piece(
        Shape::Cylinder(0.05, 0.42),
        Transform::from_translation(pivot - Vec3::Z * 0.12)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Mat::DarkMetal,
        false,
    );
    b.dress_structure(
        from,
        vec![(
            "barrier_arm",
            Transform::from_translation(pivot).with_rotation(Quat::from_rotation_z(lean)),
        )],
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
    b.bx(
        Vec3::new(2.6, 0.15, 2.6),
        Vec3::new(0.0, f + 2.5, 0.0),
        Mat::Corrugated,
    );
    b.counter(Vec3::new(0.0, f, 0.6), 0.0, 1.6, Some("ammo_common"));
}

/// A practice target: a canvas board on two posts, a black ring and a
/// white centre painted on, an earth bank behind it for the rounds.
fn target(b: &mut Builder) {
    for x in [-0.45, 0.45] {
        b.bx(
            Vec3::new(0.08, 1.7, 0.08),
            Vec3::new(x, 0.85, 0.0),
            Mat::Timber,
        );
    }
    b.bx(
        Vec3::new(0.9, 0.9, 0.03),
        Vec3::new(0.0, 1.2, 0.05),
        Mat::Canvas,
    );
    b.piece(
        Shape::Cylinder(0.3, 0.01),
        Transform::from_xyz(0.0, 1.2, 0.07)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Mat::DarkMetal,
        false,
    );
    b.piece(
        Shape::Cylinder(0.12, 0.01),
        Transform::from_xyz(0.0, 1.2, 0.08)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Mat::Plaster,
        false,
    );
    b.bx(
        Vec3::new(2.4, 1.0, 1.0),
        Vec3::new(0.0, 0.3, -1.2),
        Mat::Concrete,
    );
}

fn robot_gate(b: &mut Builder) {
    // The machines' gate: stepped white piers on concrete plinths, dark
    // seams where the panels meet, a scanner bar under the lintel, sensor
    // pods on top and conduits down the back.
    for side in [-1.0f32, 1.0] {
        let x = side * 8.0;
        b.bx(
            Vec3::new(2.4, 0.6, 4.6),
            Vec3::new(x, 0.3, 0.0),
            Mat::Concrete,
        );
        b.bx(
            Vec3::new(1.8, 5.0, 4.0),
            Vec3::new(x, 3.1, 0.0),
            Mat::RobotShell,
        );
        b.bx(
            Vec3::new(1.5, 6.6, 3.6),
            Vec3::new(x, 8.9, 0.0),
            Mat::RobotShell,
        );
        // A band proud of the joint, and seams up the inner face.
        b.bx(
            Vec3::new(1.86, 0.1, 4.06),
            Vec3::new(x, 5.6, 0.0),
            Mat::DarkMetal,
        );
        for (z, y0, y1) in [
            (-1.2, 0.8, 5.5),
            (1.2, 0.8, 5.5),
            (-0.9, 5.7, 12.0),
            (0.9, 5.7, 12.0),
        ] {
            let face = if y0 < 5.6 { 0.9 } else { 0.75 };
            b.bx(
                Vec3::new(0.04, y1 - y0, 0.12),
                Vec3::new(x - side * (face + 0.01), (y0 + y1) * 0.5, z),
                Mat::DarkMetal,
            );
        }
        // Conduit down the back, into the plinth.
        b.bx(
            Vec3::new(0.18, 12.0, 0.18),
            Vec3::new(x + side * 0.4, 6.4, 1.89),
            Mat::DarkMetal,
        );
        b.piece(
            Shape::Cylinder(0.35, 0.6),
            Transform::from_xyz(x, 13.9, -1.3),
            Mat::DarkMetal,
            true,
        );
        b.piece(
            Shape::Cylinder(0.2, 0.12),
            Transform::from_xyz(x, 14.06, -1.3)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Mat::Glass,
            false,
        );
    }
    b.bx(
        Vec3::new(17.6, 1.6, 4.0),
        Vec3::new(0.0, 12.8, 0.0),
        Mat::RobotShell,
    );
    b.bx(
        Vec3::new(17.66, 0.1, 4.06),
        Vec3::new(0.0, 12.1, 0.0),
        Mat::DarkMetal,
    );
    // The scanner: a dark bar under the lintel, its glass strip below.
    b.bx(
        Vec3::new(14.2, 0.3, 0.6),
        Vec3::new(0.0, 11.9, -1.2),
        Mat::DarkMetal,
    );
    b.bx(
        Vec3::new(14.0, 0.06, 0.45),
        Vec3::new(0.0, 11.72, -1.2),
        Mat::Glass,
    );
    b.geo.lights.push(Vec3::new(0.0, 11.5, -2.2));
}

fn boxcar(b: &mut Builder) {
    // A goods wagon on two bogies: wheels on the rails, an underframe, a
    // ribbed body with its sliding door half open, a low roof, ladders
    // and couplers at the ends. Its length runs along Z.
    let (w, d, f, h) = (3.0, 14.0, 1.2, 2.6);
    let half_pi = std::f32::consts::FRAC_PI_2;
    let bogie = extraction_core::terrain::BOGIE;
    for bz in [-bogie, bogie] {
        for x in [-0.82, 0.82] {
            b.bx(
                Vec3::new(0.12, 0.35, 2.5),
                Vec3::new(x, 0.6, bz),
                Mat::DarkMetal,
            );
        }
        b.bx(
            Vec3::new(2.0, 0.25, 0.45),
            Vec3::new(0.0, 0.88, bz),
            Mat::DarkMetal,
        );
        for dz in [-0.9, 0.9] {
            b.piece(
                Shape::Cylinder(0.07, 1.75),
                Transform::from_xyz(0.0, 0.45, bz + dz)
                    .with_rotation(Quat::from_rotation_z(half_pi)),
                Mat::DarkMetal,
                false,
            );
            for x in [-0.7, 0.7] {
                b.piece(
                    Shape::Cylinder(0.45, 0.1),
                    Transform::from_xyz(x, 0.45, bz + dz)
                        .with_rotation(Quat::from_rotation_z(half_pi)),
                    Mat::RustyMetal,
                    false,
                );
            }
        }
    }
    // Underframe: centre sill on the bolsters, side sills under the floor.
    b.bx(
        Vec3::new(0.4, 0.2, d - 0.1),
        Vec3::new(0.0, 1.1, 0.0),
        Mat::DarkMetal,
    );
    for x in [-1.36, 1.36] {
        b.bx(
            Vec3::new(0.16, 0.22, d - 0.1),
            Vec3::new(x, 1.04, 0.0),
            Mat::DarkMetal,
        );
    }
    b.bx(
        Vec3::new(w, 0.12, d),
        Vec3::new(0.0, f + 0.0, 0.0),
        Mat::Planks,
    );
    b.shell(
        w,
        d,
        f,
        h,
        0.06,
        Mat::RustyMetal,
        [&[], &[gap(d * 0.5, 2.0, 2.3)], &[], &[]],
    );
    // Ribs down both sides (clear of the doorway), the door on its rail,
    // slid half back.
    let mut z = -d * 0.5 + 0.6;
    while z < d * 0.5 - 0.5 {
        for side in [-1.0f32, 1.0] {
            if side > 0.0 && z.abs() < 1.15 {
                continue;
            }
            b.bx(
                Vec3::new(0.05, h - 0.1, 0.08),
                Vec3::new(side * (w * 0.5 + 0.055), f + h * 0.5, z),
                Mat::RustyMetal,
            );
        }
        z += 1.1;
    }
    b.bx(
        Vec3::new(0.06, 0.08, 5.0),
        Vec3::new(w * 0.5 + 0.1, f + 2.42, 0.6),
        Mat::DarkMetal,
    );
    b.bx(
        Vec3::new(0.05, 2.3, 2.1),
        Vec3::new(w * 0.5 + 0.1, f + 1.2, 1.1),
        Mat::RustyMetal,
    );
    b.bx(
        Vec3::new(w + 0.1, 0.06, d + 0.1),
        Vec3::new(0.0, f + h + 0.03, 0.0),
        Mat::RustyMetal,
    );
    b.piece(
        Shape::Prism(Vec3::new(w + 0.2, 0.22, d + 0.2)),
        Transform::from_xyz(0.0, f + h + 0.06, 0.0),
        Mat::RustyMetal,
        true,
    );
    // Ladders at diagonally opposite corners, couplers at both ends.
    for (end, x) in [(-1.0f32, -1.1f32), (1.0, 1.1)] {
        let z = end * (d * 0.5 + 0.06);
        for dx in [-0.2, 0.2] {
            b.bx(
                Vec3::new(0.04, h, 0.04),
                Vec3::new(x + dx, f + h * 0.5, z),
                Mat::DarkMetal,
            );
        }
        let mut y = f + 0.3;
        while y < f + h - 0.1 {
            b.bx(
                Vec3::new(0.44, 0.03, 0.03),
                Vec3::new(x, y, z),
                Mat::DarkMetal,
            );
            y += 0.35;
        }
        // Climbable, onto the roof.
        b.ladder(
            Vec3::new(x, f + 0.3, z),
            Vec3::new(x, f + h - 0.1, z),
            0.4,
            Vec3::Z * end,
            Some(Vec3::new(x, f + h + 0.15, end * (d * 0.5 - 0.6))),
        );
        b.bx(
            Vec3::new(0.28, 0.24, 0.6),
            Vec3::new(0.0, 1.1, end * (d * 0.5 + 0.2)),
            Mat::DarkMetal,
        );
    }
    b.crate_box(Vec3::new(0.6, f + 0.06, 2.0), 0.8, Some("workshop"));
    b.crate_box(Vec3::new(-0.6, f + 0.06, -3.0), 0.7, Some("food_common"));
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
    // Where the terrain was settled for it (the lowest corner of a rough
    // footprint, lifted as far as its foundation allows).
    // Rolling stock pitches to follow the grade under its bogies; a wreck
    // pitches and rolls to the ground under its four wheels.
    let pitch = zone.hm.pitch(&p.prefab, p.at, p.rot);
    let roll = zone.hm.roll(&p.prefab, p.at, p.rot);
    let rot = Quat::from_rotation_y(-p.rot.to_radians())
        * Quat::from_rotation_x(-pitch)
        * Quat::from_rotation_z(roll);
    let y = zone.hm.footing(&p.prefab, p.at, p.rot);
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
        if let Some((collider, surfaces)) = body(&geo) {
            commands.entity(root).insert((
                RigidBody::Static,
                collider,
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                surfaces,
            ));
        }
        for d in &geo.doors {
            spawn_door(&mut commands, root, *d);
        }
        for s in &geo.loot {
            loot.0.push(LootSpot {
                at: loot_spot_at(&zone, &t, s.at),
                table: s.table,
            });
        }
        for l in &geo.ladders {
            commands.spawn(
                crate::interact::hand_climb::Ladder {
                    bottom: t.transform_point(l.bottom),
                    top: t.transform_point(l.top),
                    width: l.width,
                    out: t.rotation * l.out,
                    over: l.over.map(|o| t.transform_point(o)),
                }
                .bundle(),
            );
        }
    }
    info!("{} building loot spots", loot.0.len());
    commands.insert_resource(loot);
}

/// How far a loot spot keeps from a note or terminal on the same table,
/// centre to centre: what's left there doesn't lie across the paper.
const LOOT_CLEAR: f32 = 0.55;

/// A loot spot moved along the table, away from a note or terminal that
/// lies on it (notes sit 0.3 m off a table's middle, where its loot is).
/// Where a prefab's loot spot (`local`, in the prefab) lies in the world:
/// clear of notes, and never under the ground. A prefab set on its lowest
/// corner over a hill (the slag heaps' anomaly) had its loot spawned inside
/// the hill, falling through it (`docs/SOAK.md`). Underground sites' spots
/// are under the ground on purpose.
pub fn loot_spot_at(zone: &Zone, t: &Transform, local: Vec3) -> Vec3 {
    let mut at = clear_of_notes(&zone.map, t.transform_point(local));
    if crate::world::underground::floor_under(zone, at).is_none() {
        at.y = at.y.max(zone.ground(at.x, at.z) + 0.05);
    }
    at
}

fn clear_of_notes(map: &extraction_core::map::MapDef, at: Vec3) -> Vec3 {
    use extraction_core::map::MarkerKind;
    let Some(m) = map.markers.iter().find(|m| {
        matches!(
            m.kind,
            MarkerKind::Note { .. } | MarkerKind::Terminal { .. }
        ) && Vec2::new(m.at[0] - at.x, m.at[1] - at.z).length() < LOOT_CLEAR
    }) else {
        return at;
    };
    let away = Vec2::new(at.x - m.at[0], at.z - m.at[1]);
    let dir = away.try_normalize().unwrap_or(Vec2::X);
    let to = Vec2::new(m.at[0], m.at[1]) + dir * LOOT_CLEAR;
    Vec3::new(to.x, at.y, to.y)
}

/// A building's compound collider and its pieces' materials (also how a
/// breach rebuilds them: `destruction::breach`). None if nothing collides.
pub fn body(geo: &Geo) -> Option<(Collider, super::surfaces::Surfaces)> {
    let mut colliders = Vec::new();
    let mut surfaces = Vec::new();
    // Crates are their own things (`destruction::wood`): they break.
    let crates = crate::destruction::wood::crate_pieces(geo);
    for (piece, _) in geo.pieces.iter().zip(&crates).filter(|(_, c)| !**c) {
        let half = match piece.shape {
            Shape::Box(s) => s * 0.5,
            Shape::Prism(s) => Vec3::new(s.x, s.y * 0.5, s.z) * 0.5,
            Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
        };
        // Visual-only pieces count too: floorboards over a plinth.
        surfaces.push((half, piece.at, piece.mat));
        if !piece.collide {
            continue;
        }
        let c = match piece.shape {
            Shape::Box(s) => Collider::cuboid(s.x, s.y, s.z),
            Shape::Prism(s) => Collider::cuboid(s.x, s.y * 0.5, s.z),
            Shape::Cylinder(r, h) => Collider::cylinder(r, h),
        };
        colliders.push((piece.at.translation, piece.at.rotation, c));
    }
    (!colliders.is_empty()).then(|| {
        (
            Collider::compound(colliders),
            super::surfaces::Surfaces(surfaces),
        )
    })
}

/// A hinged door in a building (also how the Surge rehangs one that was
/// broken down: `destruction::wood`).
pub fn spawn_door(commands: &mut Commands, root: Entity, d: DoorDef) -> Entity {
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
            DoorLeaf(d),
            Transform::from_translation(d.hinge).with_rotation(Quat::from_rotation_y(d.yaw)),
            Visibility::default(),
            RigidBody::Kinematic,
            ChildOf(root),
        ))
        .with_child((
            Collider::cuboid(d.width, d.height, 0.05),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
            Transform::from_xyz(d.width * 0.5, d.height * 0.5 + 0.01, 0.0),
        ))
        .id()
}

/// The door's definition, kept for visuals.
#[derive(Component, Clone, Copy, Debug)]
pub struct DoorLeaf(pub DoorDef);

/// Pressing a door toggles it; it swings over a second.
pub fn swing_doors(
    mut events: MessageReader<crate::interact::InteractEvent>,
    mut doors: Query<(
        &mut Door,
        &mut Transform,
        &DoorLeaf,
        Has<crate::robots::doors::Wedged>,
        Option<&super::underground::PowerLocked>,
    )>,
    time: Res<Time>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for ev in events.read() {
        if let crate::interact::InteractEvent::Pressed { item, .. } = ev {
            if let Ok((mut d, _, _, wedged, locked)) = doors.get_mut(*item) {
                if wedged {
                    raid.say("It won't budge: there's a wedge under it.");
                } else if locked.is_some_and(|l| l.stage == super::underground::SEALED) {
                    raid.say("Bolted by the shelter's programme. It won't open again.");
                } else if locked.is_some() {
                    raid.say("Locked fast: an electric bolt, and no power to it.");
                } else {
                    d.open = !d.open;
                }
            }
        }
    }
    for (mut d, mut t, leaf, _, _) in &mut doors {
        let target = if d.open { 1.0 } else { 0.0 };
        d.angle += (target - d.angle).clamp(-time.delta_secs() * 1.6, time.delta_secs() * 1.6);
        t.rotation = Quat::from_rotation_y(leaf.0.yaw + d.angle * 1.75);
    }
}

/// A building waiting for its props' models before it builds its meshes
/// (frames waited: it gives up on a model that never arrives).
#[derive(Component)]
pub struct PropsPending(pub(crate) u32);

/// Build a building's meshes again (its walls changed: a breach, or the
/// Surge mending one). Its merged meshes go; `dress_buildings` makes new.
pub fn redress(commands: &mut Commands, e: Entity) {
    commands.entity(e).insert(PropsPending(0));
}

/// Client: lights for buildings, and door leaves (their boxes until the
/// door model is in). The meshes come once the props' models have loaded
/// (`dress_buildings`).
pub fn building_visuals(
    mut commands: Commands,
    buildings: Query<(Entity, &Building), Added<Building>>,
    doors: Query<(Entity, &DoorLeaf), Added<DoorLeaf>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<crate::render::materials::MaterialLibrary>,
    server: Res<AssetServer>,
) {
    use crate::render::world_props::{Blockout, SwapIn};
    for (e, b) in &buildings {
        let Some(geo) = prefab(&b.prefab) else {
            continue;
        };
        commands.entity(e).insert(PropsPending(0));
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
        // The door models are 0.91 x 2.03; stretched to other leaves.
        let model = if leaf.mat == Mat::PaintedMetal {
            "door_metal"
        } else {
            "door_leaf"
        };
        commands.entity(e).insert(SwapIn(vec![(
            model,
            Transform::from_scale(Vec3::new(leaf.width / 0.91, leaf.height / 2.03, 1.0)),
        )]));
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
            Blockout,
        ));
        // A ledged and braced door: two ledges and a diagonal on the
        // inside face, a knob either side.
        let timber = library.get(Mat::Timber, &server, &mut materials);
        for y in [0.3, leaf.height - 0.3] {
            commands.spawn((
                Mesh3d(meshes.add(crate::render::materials::metric_box(Vec3::new(
                    leaf.width - 0.1,
                    0.12,
                    0.03,
                )))),
                MeshMaterial3d(timber.clone()),
                Transform::from_xyz(leaf.width * 0.5, y, 0.04),
                ChildOf(e),
                Blockout,
            ));
        }
        let (a, b) = (
            Vec2::new(0.12, 0.36),
            Vec2::new(leaf.width - 0.12, leaf.height - 0.36),
        );
        let brace = b - a;
        commands.spawn((
            Mesh3d(meshes.add(crate::render::materials::metric_box(Vec3::new(
                brace.length(),
                0.1,
                0.03,
            )))),
            MeshMaterial3d(timber),
            Transform::from_xyz((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, 0.04)
                .with_rotation(Quat::from_rotation_z(brace.y.atan2(brace.x))),
            ChildOf(e),
            Blockout,
        ));
        let knob = library.get(Mat::DarkMetal, &server, &mut materials);
        for z in [-0.05, 0.08] {
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(0.035))),
                MeshMaterial3d(knob.clone()),
                Transform::from_xyz(leaf.width - 0.1, 1.0, z),
                ChildOf(e),
                Blockout,
            ));
        }
    }
}

/// A box whose UVs run on from its neighbours': in metres, measured from
/// the building's origin in the box's own orientation, so the boxes of a
/// wall either side of a window (one rotation, different places) carry
/// the brick courses across rather than each starting the texture afresh.
fn aligned_box(size: Vec3, at: Transform) -> Mesh {
    use bevy::mesh::VertexAttributeValues;
    let mut mesh = crate::render::materials::metric_box(size);
    let offset = at.rotation.inverse() * at.translation;
    let pos: Vec<[f32; 3]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(p)) => p.clone(),
        _ => return mesh,
    };
    let nrm: Vec<[f32; 3]> = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(n)) => n.clone(),
        _ => return mesh,
    };
    // The same (u, v) axes per face as `metric_box`.
    let uv: Vec<[f32; 2]> = pos
        .iter()
        .zip(&nrm)
        .map(|(p, n)| {
            let r = Vec3::from(*p) + offset;
            let n = Vec3::from(*n);
            let (u, v) = if n.x > 0.5 {
                (Vec3::NEG_Z, Vec3::NEG_Y)
            } else if n.x < -0.5 {
                (Vec3::Z, Vec3::NEG_Y)
            } else if n.y > 0.5 {
                (Vec3::X, Vec3::Z)
            } else if n.y < -0.5 {
                (Vec3::X, Vec3::NEG_Z)
            } else if n.z > 0.5 {
                (Vec3::X, Vec3::NEG_Y)
            } else {
                (Vec3::NEG_X, Vec3::NEG_Y)
            };
            [r.dot(u), r.dot(v)]
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh
}

/// What shades a building's own surfaces (`ao`): its solid pieces
/// (furniture's boxes too, so the floor darkens round their feet), not its
/// glass.
pub fn occluders(geo: &Geo) -> super::ao::Occluders {
    super::ao::Occluders::new(geo.pieces.iter().filter(|p| p.mat != Mat::Glass).map(|p| {
        let half = match p.shape {
            Shape::Box(s) => s * 0.5,
            Shape::Prism(s) => Vec3::new(s.x * 0.5, s.y * 0.25, s.z * 0.5),
            Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
        };
        let at = match p.shape {
            // A gable's box is its lower half.
            Shape::Prism(s) => p.at * Transform::from_xyz(0.0, s.y * 0.25, 0.0),
            _ => p.at,
        };
        (half, at)
    }))
}

/// A prefab's pieces shaded by its own solid pieces, baked once in the
/// background: a big site's thousand pieces take half a second, a house's
/// a few dozen milliseconds, every house alike (`render::spikes`).
pub enum Bake {
    Baking(bevy::tasks::Task<std::sync::Arc<Vec<Option<Mesh>>>>),
    Ready(std::sync::Arc<Vec<Option<Mesh>>>),
}

/// A piece's mesh in building space, unshaded.
fn piece_mesh(piece: &Piece) -> Mesh {
    let mut mesh = match piece.shape {
        Shape::Box(s) => aligned_box(s, piece.at),
        Shape::Prism(s) => crate::render::materials::gable_prism(s.x, s.y, s.z),
        Shape::Cylinder(r, h) => Cylinder::new(r, h).mesh().resolution(16).build(),
    };
    let _ = mesh.generate_tangents();
    mesh.transformed_by(piece.at)
}

/// Each piece as `dress_buildings` draws it when it isn't furniture's
/// stand-in: a box shaded face by face, anything else shaded at its
/// vertices (not under the ground: a site's rooms go unshaded).
fn bake_pieces(geo: &Geo, site: bool) -> Vec<Option<Mesh>> {
    let occluders = occluders(geo);
    geo.pieces
        .iter()
        .map(|piece| match piece.shape {
            // Glass keeps its own alpha (the rest carries the sky).
            Shape::Box(s) => Some(super::ao::shaded_box(
                s,
                piece.at,
                &occluders,
                piece.mat != Mat::Glass,
            )),
            _ if site && piece.at.translation.y < super::underground::BELOW_Y => None,
            _ => {
                let mut mesh = piece_mesh(piece);
                occluders.bake(&mut mesh);
                Some(mesh)
            }
        })
        .collect()
}

/// Client: a building's meshes, one per material (a house is a handful of
/// draws, not fifty), once its props' models have loaded or failed: each
/// prop whose models are in is merged in place of its boxes.
pub fn dress_buildings(
    mut commands: Commands,
    mut pending: Query<(Entity, &Building, &Transform, &mut PropsPending)>,
    zone: Option<Res<super::Zone>>,
    ivy: Option<Res<super::overgrowth::IvyAssets>>,
    stain_mats: Option<Res<super::decay::StainAssets>>,
    epilogue: Option<Res<crate::raid::epilogue::Epilogue>>,
    mut props: crate::render::world_props::PropModels,
    mut cache: ResMut<crate::render::world_props::PropMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stand_in_mat: Res<crate::render::lod::StandInMaterial>,
    mut bakes: Local<std::collections::HashMap<String, Bake>>,
    breaches: Option<Res<crate::destruction::breach::Breaches>>,
) {
    use crate::render::world_props::Resolved;
    // A few buildings a frame, each once its prefab's shading is baked
    // (in the background, `Bake`), so a valley's arrival doesn't hitch.
    let mut built = 0;
    for (e, b, xf, mut wait) in &mut pending {
        if built >= 3 {
            break;
        }
        let Some(mut geo) = prefab(&b.prefab) else {
            commands.entity(e).remove::<PropsPending>();
            continue;
        };
        // Holes torn in its walls (`destruction::breach`): a building
        // with holes has its own shading, not its prefab's.
        let holes = breaches
            .as_ref()
            .map_or(0, |h| crate::destruction::breach::apply(&mut geo, xf, &h.0));
        // Old snow on its roof and sills, where it lies all winter
        // (`decay::snow_load`). Not down an underground site: nothing
        // there is open to the sky.
        let snowed = extraction_core::underground::layout(&b.prefab).is_none()
            && zone.as_ref().is_some_and(|z| {
                z.map.snow > 0.0
                    && super::snow::snow_line(&z.map).is_some_and(|l| xf.translation.y > l)
            });
        if snowed {
            let load = super::decay::snow_load(&geo, &occluders(&geo));
            geo.pieces.extend(load);
        }
        let bake_key = if holes == 0 && !snowed {
            b.prefab.clone()
        } else if holes == 0 {
            format!("{} snowed", b.prefab)
        } else {
            format!(
                "{} at {:.1},{:.1} with {holes} holes",
                b.prefab, xf.translation.x, xf.translation.z
            )
        };
        // Years empty: some of the panes smashed (all of a ruin's), the
        // same as the logic's panes see them (`decay::weathered_sashes`).
        let broken = super::decay::weathered(&geo, &b.prefab, xf);
        let mut sash = broken.iter();
        for d in &mut geo.props {
            for (name, _) in &mut d.models {
                if *name == "window_sash" && sash.next() == Some(&true) {
                    *name = "window_sash_broken";
                }
            }
        }
        // What the people who lived here left (`clutter`), with the
        // furniture.
        if let Some(zone) = &zone {
            let at = Vec2::new(xf.translation.x, xf.translation.z);
            let tag = zone
                .map
                .placements
                .iter()
                .find(|p| p.prefab == b.prefab && Vec2::from(p.at).distance(at) < 0.5)
                .map_or("", |p| p.tag.as_str());
            let ending = epilogue.as_ref().is_some_and(|e| e.done);
            let models = super::clutter::dress(&zone.map, &b.prefab, tag, xf, ending);
            if !models.is_empty() {
                let n = geo.pieces.len();
                geo.props.push(PropDef {
                    models,
                    pieces: n..n,
                    inside: true,
                    shadow: false,
                    lods: None,
                });
            }
            // Its yard (`clutter::yard`), out in the sun.
            let hm = &zone.hm;
            let ground = |p: Vec2| {
                let p = [p.x, p.y];
                hm.water_level(p).is_none().then(|| hm.height(p))
            };
            let models = super::clutter::yard(&zone.map, &b.prefab, tag, xf, &ground);
            if !models.is_empty() {
                let n = geo.pieces.len();
                geo.props.push(PropDef {
                    models,
                    pieces: n..n,
                    inside: false,
                    shadow: true,
                    lods: None,
                });
            }
        }
        wait.0 += 1;
        let bake = bakes.entry(bake_key).or_insert_with(|| {
            let geo = geo.clone();
            let site = super::underground::ids().any(|id| id == b.prefab);
            Bake::Baking(
                bevy::tasks::AsyncComputeTaskPool::get()
                    .spawn(async move { std::sync::Arc::new(bake_pieces(&geo, site)) }),
            )
        });
        if let Bake::Baking(task) = bake {
            match bevy::tasks::futures::check_ready(task) {
                Some(baked) => *bake = Bake::Ready(baked),
                None => continue,
            }
        }
        let Bake::Ready(baked) = bake else {
            continue;
        };
        let baked = baked.clone();
        let mut loading = false;
        let ready: Vec<bool> = geo
            .props
            .iter()
            .map(|d| {
                match props.resolve_all(
                    d.models
                        .iter()
                        .map(|(n, _)| *n)
                        .chain(d.lods.into_iter().flatten()),
                ) {
                    Resolved::Loading => {
                        loading = true;
                        false
                    }
                    Resolved::Missing => false,
                    Resolved::Ready => true,
                }
            })
            .collect();
        // And their meshes, not just their glTFs (`PropMeshes::get`).
        if !loading {
            loading = geo.props.iter().zip(&ready).any(|(d, r)| {
                *r && d
                    .models
                    .iter()
                    .map(|(n, _)| *n)
                    .chain(d.lods.into_iter().flatten())
                    .any(|n| {
                        cache.get(n, &mut props, &meshes);
                        !cache.has(n)
                    })
            });
        }
        // Models load in a few frames; past ten seconds, build without.
        if loading && wait.0 < 600 {
            continue;
        }
        commands.entity(e).remove::<PropsPending>();
        built += 1;
        // What shades the building's own surfaces: its solid pieces
        // (furniture's boxes too, so the floor darkens round their feet),
        // not its glass.
        let occluders = occluders(&geo);
        // Window glass is the panes' (`destruction::glass`): they draw it,
        // whole or broken; and crates are their own (`destruction::wood`):
        // they break.
        let mut skip = crate::destruction::glass::window_glass(&geo);
        for (s, c) in skip
            .iter_mut()
            .zip(crate::destruction::wood::crate_pieces(&geo))
        {
            *s |= c;
        }
        let panes = skip.clone();
        let crate_props = crate::destruction::wood::crate_props(&geo);
        for (d, _) in geo.props.iter().zip(&ready).filter(|(_, r)| **r) {
            for i in d.pieces.clone() {
                skip[i] = true;
            }
        }
        // Four meshes a material: the shell; its windows, which cast no
        // shadow worth the cost (they're in the wall's own); the
        // furniture's models, near and shadowless (the sun doesn't reach
        // it); the furniture's boxes, further off.
        const OUTER: usize = 0;
        const NEAR: usize = 1;
        const FAR: usize = 2;
        const WINDOWS: usize = 3;
        // A hero landmark's full model and its middle one (see `PropDef::lods`).
        const HERO: usize = 4;
        const HERO_MID: usize = 5;
        let mut far = vec![false; geo.pieces.len()];
        // The shell, one vertex-coloured mesh, for past `lod::FAR_BUILDING`.
        let mut stand_in = crate::render::lod::StandIn::default();
        for (d, _) in geo
            .props
            .iter()
            .zip(&ready)
            .filter(|(d, r)| **r && d.inside)
        {
            for i in d.pieces.clone() {
                far[i] = true;
            }
        }
        // An underground site's rooms: their own group, drawn only when
        // the eye can see them (`underground::Below`).
        const BELOW: usize = 6;
        let site = super::underground::ids().any(|id| id == b.prefab);
        let below = |at: &Transform| site && at.translation.y < super::underground::BELOW_Y;
        let mut merged: [Vec<(Handle<StandardMaterial>, Mesh)>; 7] = Default::default();
        let mut add = |group: usize, mat: Handle<StandardMaterial>, mesh: Mesh| {
            let into = &mut merged[group];
            let fits = match into.iter_mut().find(|(m, _)| *m == mat) {
                Some((_, m)) => m.merge(&mesh).is_ok(),
                None => false,
            };
            if !fits {
                into.push((mat, mesh));
            }
        };
        for (i, piece) in geo.pieces.iter().enumerate() {
            if (skip[i] && !far[i]) || panes[i] {
                continue;
            }
            let mat = props.lib.get(piece.mat, &props.server, &mut materials);
            let is_below = below(&piece.at);
            if let (false, Shape::Box(s)) = (far[i], piece.shape) {
                // Shaded, and cut where the shading changes (the far
                // stand-in has the plain box).
                if !is_below {
                    if let Some(m) = materials.get(&mat) {
                        let plain = aligned_box(s, piece.at).transformed_by(piece.at);
                        stand_in.add(&plain, crate::render::lod::far_colour(m));
                    }
                }
                let group = if is_below { BELOW } else { OUTER };
                if let Some(mesh) = &baked[i] {
                    add(group, mat, mesh.clone());
                }
                continue;
            }
            let mut mesh = piece_mesh(piece);
            if !far[i] && !is_below {
                if let Some(m) = materials.get(&mat) {
                    stand_in.add(&mesh, crate::render::lod::far_colour(m));
                }
                if let Some(shaded) = &baked[i] {
                    mesh = shaded.clone();
                }
            } else {
                super::ao::ensure_colours(&mut mesh);
            }
            add(
                if is_below {
                    BELOW
                } else if far[i] {
                    FAR
                } else {
                    OUTER
                },
                mat,
                mesh,
            );
        }
        for ((d, _), _) in geo
            .props
            .iter()
            .zip(&ready)
            .zip(&crate_props)
            .filter(|((_, r), c)| **r && !**c)
        {
            if let Some([mid, far]) = d.lods {
                let (near, at) = d.models[0];
                for (name, group) in [(near, Some(HERO)), (mid, Some(HERO_MID)), (far, None)] {
                    let prims = cache.get(name, &mut props, &meshes).to_vec();
                    for (mat, mesh) in prims {
                        let mat = props.material(&mat, &mut materials);
                        let mesh = mesh.transformed_by(at);
                        match group {
                            Some(g) => add(g, mat, mesh),
                            None => {
                                if let Some(m) = materials.get(&mat) {
                                    stand_in.add(&mesh, crate::render::lod::far_colour(m));
                                }
                            }
                        }
                    }
                }
                continue;
            }
            for (name, at) in &d.models {
                let prims = cache.get(name, &mut props, &meshes).to_vec();
                for (mat, mesh) in prims {
                    if crate::destruction::glass::drawn_by_panes(name, &mat) {
                        continue;
                    }
                    let mat = props.material(&mat, &mut materials);
                    if !d.inside {
                        if let Some(m) = materials.get(&mat) {
                            stand_in.add(
                                &mesh.clone().transformed_by(*at),
                                crate::render::lod::far_colour(m),
                            );
                        }
                    }
                    add(
                        if below(at) {
                            BELOW
                        } else if d.inside {
                            NEAR
                        } else if d.shadow {
                            OUTER
                        } else {
                            WINDOWS
                        },
                        mat,
                        mesh.transformed_by(*at),
                    );
                }
            }
        }
        // Past this the furniture is boxes (and inside a dark house, hardly
        // seen at all); bounds-centred, as a merged mesh sits at the
        // building's origin.
        let range = |near: bool| {
            let switch = 25.0..35.0;
            if near {
                bevy::camera::visibility::VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: switch,
                    use_aabb: true,
                }
            } else {
                bevy::camera::visibility::VisibilityRange {
                    start_margin: switch,
                    end_margin: crate::render::lod::near_range().end_margin,
                    use_aabb: true,
                }
            }
        };
        let [mut outer, near, mut far, windows, hero, hero_mid, under] = merged;
        // Moss and lichen in the shell's colours, stains on its walls
        // (`decay`).
        let decay = !site && super::overgrowth::enabled();
        let bseed =
            super::overgrowth::seed(Vec2::new(xf.translation.x, xf.translation.z), &b.prefab);
        let bnorth = xf.rotation.inverse() * Vec3::NEG_Z;
        if decay {
            // The timber houses' boards in their faded paint, each its own
            // (not a ruin's: soot, or the water, took it).
            if matches!(b.prefab.as_str(), "house_plank" | "farmhouse") {
                let boards = props.lib.get(Mat::Planks, &props.server, &mut materials);
                let paint = super::decay::paint(bseed);
                for (mat, m) in &mut outer {
                    if *mat == boards {
                        super::decay::tint(m, paint);
                    }
                }
            }
            for (_, m) in &mut outer {
                super::decay::moss(m, bnorth, bseed);
            }
            // A third of the old houses' roofs bowed (their ridges along
            // z), near and far alike.
            if matches!(
                b.prefab.as_str(),
                "house_plank" | "farmhouse" | "house_brick" | "cabin_log" | "shed"
            ) {
                let bow = super::decay::sagging(bseed);
                let roofs = [Mat::Shingles, Mat::Corrugated]
                    .map(|m| props.lib.get(m, &props.server, &mut materials));
                let roof = super::decay::roof_bounds(
                    outer
                        .iter()
                        .filter(|(mat, _)| roofs.contains(mat))
                        .map(|(_, m)| m),
                );
                if let (Some(roof), true) = (roof, bow > 0.0) {
                    for (_, m) in outer.iter_mut().chain(far.iter_mut()) {
                        super::decay::sag(m, roof, bow);
                    }
                }
            }
            if let Some(sm) = &stain_mats {
                let decals =
                    super::decay::stains(&geo, &occluders, bseed, b.prefab.ends_with("_burnt"));
                for (k, kind) in super::decay::STAINS.iter().enumerate() {
                    if let Some(m) = super::decay::decal_mesh(&decals, *kind) {
                        commands.spawn((
                            Mesh3d(meshes.add(m)),
                            MeshMaterial3d(sm.0[k].clone()),
                            bevy::light::NotShadowCaster,
                            bevy::camera::visibility::VisibilityRange {
                                start_margin: 0.0..0.0,
                                end_margin: super::overgrowth::LEAVES_TO - 5.0
                                    ..super::overgrowth::LEAVES_TO + 5.0,
                                use_aabb: true,
                            },
                            ChildOf(e),
                        ));
                    }
                }
            }
        }
        // Years of ivy (`overgrowth`): stems near, leaves to the middle
        // distance, a few big cards past that. None where the water's only
        // just gone down.
        // Nor above the snow line, where nothing grows.
        let drowned = b.prefab.ends_with("_drowned");
        let warm = zone
            .as_ref()
            .is_none_or(|z| super::overgrowth::warm(&z.map, xf.translation.y));
        if let (Some(ivy), false, false, true) =
            (&ivy, site, drowned || !warm, super::overgrowth::enabled())
        {
            let at = Vec2::new(xf.translation.x, xf.translation.z);
            let north = xf.rotation.inverse() * Vec3::NEG_Z;
            let gseed = super::overgrowth::seed(at, &b.prefab);
            let mut growth = super::overgrowth::grow_on(&b.prefab, &geo, &occluders, gseed, north);
            // Saplings through a ruin's floor, bushes at the walls' feet.
            let ruin = extraction_core::map::prefab_base(&b.prefab) != b.prefab;
            let wild = super::overgrowth::reclaim(&geo, &occluders, gseed, ruin);
            growth.vines.extend(wild.vines);
            growth.leaves.extend(wild.leaves);
            let span = |a: f32, b: f32| bevy::camera::visibility::VisibilityRange {
                start_margin: if a > 0.0 { a - 5.0..a + 5.0 } else { 0.0..0.0 },
                end_margin: b - 5.0..b + 5.0,
                use_aabb: true,
            };
            if let Some(m) = super::overgrowth::vine_mesh(&growth) {
                commands.spawn((
                    Mesh3d(meshes.add(m)),
                    MeshMaterial3d(ivy.vines.clone()),
                    bevy::light::NotShadowCaster,
                    span(0.0, super::overgrowth::VINES_TO),
                    ChildOf(e),
                ));
            }
            for (every, scale, range) in [
                (1, 1.0, span(0.0, super::overgrowth::LEAVES_TO)),
                (
                    3,
                    1.6,
                    span(
                        super::overgrowth::LEAVES_TO,
                        crate::render::lod::FAR_BUILDING,
                    ),
                ),
            ] {
                if let Some(m) = super::overgrowth::leaf_mesh(&growth, every, scale) {
                    commands.spawn((
                        Mesh3d(meshes.add(m)),
                        MeshMaterial3d(ivy.leaves.clone()),
                        bevy::light::NotShadowCaster,
                        range,
                        ChildOf(e),
                    ));
                }
            }
        }
        for (mat, mesh) in under {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(mat),
                bevy::light::NotShadowCaster,
                super::underground::Below,
                ChildOf(e),
            ));
        }
        let mid = LANDMARK_MID - 5.0..LANDMARK_MID + 5.0;
        for (group, range) in [
            (
                hero,
                bevy::camera::visibility::VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: mid.clone(),
                    use_aabb: true,
                },
            ),
            (
                hero_mid,
                bevy::camera::visibility::VisibilityRange {
                    start_margin: mid.clone(),
                    end_margin: crate::render::lod::near_range().end_margin,
                    use_aabb: true,
                },
            ),
        ] {
            for (mat, mesh) in group {
                commands.spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(mat),
                    range.clone(),
                    ChildOf(e),
                ));
            }
        }
        for (mat, mesh) in outer {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(mat),
                crate::render::lod::near_range(),
                ChildOf(e),
            ));
        }
        for (mat, mesh) in windows {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(mat),
                bevy::light::NotShadowCaster,
                crate::render::lod::near_range(),
                ChildOf(e),
            ));
        }
        if let Some(mesh) = stand_in.build() {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(stand_in_mat.0.clone()),
                bevy::light::NotShadowCaster,
                crate::render::lod::far_range(),
                ChildOf(e),
            ));
        }
        for (group, near) in [(near, true), (far, false)] {
            for (mat, mesh) in group {
                commands.spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(mat),
                    bevy::light::NotShadowCaster,
                    range(near),
                    ChildOf(e),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {

    /// A table's loot keeps clear of the note on it (the pump station's
    /// log, read at its table, with the tools there too).
    #[test]
    fn loot_keeps_off_the_notes() {
        use extraction_core::map::MarkerKind;
        for name in extraction_core::map::VALLEYS.map(|v| v.0) {
            let zone = Zone::load(name);
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(zone.clone())
                .add_systems(Startup, spawn_buildings);
            app.update();
            let loot = app.world().resource::<BuildingLoot>();
            assert!(loot.0.len() > 20, "{name}: {} loot spots", loot.0.len());
            for m in &zone.map.markers {
                if !matches!(
                    m.kind,
                    MarkerKind::Note { .. } | MarkerKind::Terminal { .. }
                ) {
                    continue;
                }
                for s in &loot.0 {
                    let d = Vec2::new(m.at[0] - s.at.x, m.at[1] - s.at.z).length();
                    assert!(
                        d > LOOT_CLEAR - 0.01,
                        "{name}: {} under loot ({d:.2} m)",
                        m.name
                    );
                }
            }
        }
    }

    use super::*;

    /// Every model a recipe dresses with is built (`art/props`), and
    /// replaces only pieces of its own: a renamed or missing model would
    /// leave the furniture as boxes without anyone noticing.
    #[test]
    fn every_prop_model_exists() {
        let root = crate::asset_root_path();
        for id in PREFABS.into_iter().chain(super::super::underground::ids()) {
            let geo = prefab(id).unwrap();
            for d in &geo.props {
                assert!(
                    d.pieces.end <= geo.pieces.len(),
                    "{id}: a prop claims pieces past the end"
                );
                let lods = d.lods.into_iter().flatten();
                for name in d.models.iter().map(|(n, _)| *n).chain(lods) {
                    let p = root.join(crate::render::world_props::path(name));
                    assert!(p.exists(), "{id}: no model {}", p.display());
                }
            }
        }
        for name in [
            "pew",
            "lever_frame",
            "hay_bale",
            "door_metal",
            "road_block",
            "gate_post",
            "barrier_arm",
            "chair",
            "locker",
            "barrel",
            "pallet",
            "door_leaf",
            "signpost",
            "fence_post",
            "supply_crate",
            "stash_stand",
            "camp_table",
            "camp_board",
            "camp_post",
            "lantern",
        ] {
            let p = root.join(crate::render::world_props::path(name));
            assert!(p.exists(), "no model {}", p.display());
        }
    }

    /// Breakable things are sound, face out of their building, and are
    /// where they should be: a timber house's windows with their shutters,
    /// a wrecked bus with its door and panels, a garden fence with its
    /// four panels.
    #[test]
    fn breakables_are_where_they_should_be() {
        for id in PREFABS {
            let g = prefab(id).unwrap();
            for b in &g.breakables {
                let t = b.at.translation;
                assert!(
                    b.size.min_element() > 0.0 && t.is_finite() && b.at.rotation.is_finite(),
                    "{id}: {b:?}"
                );
                // Its outer face away from the middle (a fence faces -Z; a
                // bus's parts face out of the bus, not the depot).
                let out = b.at.rotation * Vec3::NEG_Z;
                if b.kind == BreakKind::Shutter {
                    assert!(
                        Vec2::new(t.x, t.z).dot(Vec2::new(out.x, out.z)) > 0.0,
                        "{id}: {:?} faces in",
                        b.kind
                    );
                }
            }
        }
        let count = |id: &str, k: BreakKind| {
            prefab(id)
                .unwrap()
                .breakables
                .iter()
                .filter(|b| b.kind == k)
                .count()
        };
        let windows = |id: &str| {
            prefab(id)
                .unwrap()
                .props
                .iter()
                .flat_map(|d| d.models.iter())
                .filter(|(m, _)| *m == "window_casing")
                .count()
        };
        for id in ["house_plank", "farmhouse", "cabin_log"] {
            assert!(windows(id) > 0);
            assert!(
                count(id, BreakKind::Shutter) >= windows(id) * 2 - 2,
                "{id}: {} shutters for {} windows",
                count(id, BreakKind::Shutter),
                windows(id)
            );
        }
        assert_eq!(count("house_brick", BreakKind::Shutter), 0);
        assert_eq!(count("bus_depot", BreakKind::BusDoor), 1);
        assert_eq!(count("bus_depot", BreakKind::BusPanel), 4);
        assert_eq!(count("fence_garden", BreakKind::FencePanel), 4);
    }

    /// Every piece of every recipe has a real, positive size and a finite
    /// pose (a negative box gave the colliders a NaN mass).
    #[test]
    fn every_piece_is_sound() {
        for id in PREFABS.into_iter().chain(super::super::underground::ids()) {
            for (i, p) in prefab(id).unwrap().pieces.iter().enumerate() {
                let ok = match p.shape {
                    Shape::Box(s) | Shape::Prism(s) => s.is_finite() && s.min_element() > 0.0,
                    Shape::Cylinder(r, h) => r > 0.0 && h > 0.0,
                } && p.at.translation.is_finite()
                    && p.at.rotation.is_finite();
                assert!(ok, "{id} piece {i}: {:?} at {:?}", p.shape, p.at);
            }
        }
    }

    /// The map's copy of every prefab's doors (`map::prefab_entrances`,
    /// for the lint that keeps their steps and the space in front clear)
    /// is the recipes' own. If this fails, regenerate the table from
    /// `Geo::entrances`.
    #[test]
    fn entrances_match_the_map() {
        for id in PREFABS {
            let built = prefab(id).unwrap().entrances;
            let listed = extraction_core::map::prefab_entrances(id);
            assert_eq!(built.len(), listed.len(), "{id}: doors built vs listed");
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

    /// Furniture keeps its boxes when dressed: the colliders and loot
    /// heights the models were built round.
    #[test]
    fn dressing_keeps_the_boxes() {
        let mut b = Builder::default();
        b.table(Vec3::ZERO, 0.0, Some("house"));
        let g = b.finish();
        assert_eq!(g.pieces.len(), 5);
        assert_eq!(g.props.len(), 1);
        assert_eq!(g.props[0].pieces, 0..5);
        assert!((g.loot[0].at.y - 0.82).abs() < 1e-6);
    }
}
