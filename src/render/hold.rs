//! A glove on a thing held free (by no grip of its own: food, tools,
//! valuables, ammo, a loose magazine): drawn closed on it.
//!
//! A free hold keeps the item where the hand took it (`interact`), and the
//! glove used to curl by the item's smallest size wherever it was. Its
//! fingers closed through the item or on air beside it, and a fist hung
//! off the side of a tin. Now the glove is seated on the item's box (the
//! item's collider: its model is fitted to it, `models`'
//! `every_item_model_fits_its_box`). The glove keeps the hand's turn and
//! slides at most a few centimetres: palm onto the item, then each finger
//! curls until it touches. Of the places it could slide to, the nearest
//! one where the item is held is taken, as [`held`] judges it: nothing
//! sinks in further than [`SINK`], the thumb and two fingers touch within
//! [`TOUCH`], and the palm is on it or the thumb closes against a finger.

use bevy::math::Affine3A;
use bevy::prelude::*;

use super::props::{FINGER_BONES, finger_root, glove_frame, joint_rotation_spread, rest_spread};
use crate::rig::Side;

/// The deepest a glove may sink into what it holds, metres (the glove's
/// leather gives a little).
pub const SINK: f32 = 0.004;
/// A finger within this of the item touches it, metres.
pub const TOUCH: f32 = 0.003;

/// The glove's finger radius at each joint and at the tip
/// (`art/hands/gloves.py`'s `RADII`).
const RADII: [[f32; 4]; 5] = [
    [0.0094, 0.0087, 0.0080, 0.0073],
    [0.0096, 0.0089, 0.0082, 0.0075],
    [0.0092, 0.0085, 0.0078, 0.0072],
    [0.0084, 0.0077, 0.0071, 0.0066],
    [0.0122, 0.0102, 0.0086, 0.0],
];

/// One bone of a finger in the hand's frame: its ends and radius.
#[derive(Clone, Copy, Debug)]
struct Bone {
    a: Vec3,
    b: Vec3,
    r: f32,
}

/// A finger's bones curled `c` (0 open, 1 closed), in the hand's frame:
/// the chain `props::spawn_hand` builds, at the hand's resting spread.
fn bones(side: Side, finger: usize, c: f32) -> impl Iterator<Item = Bone> {
    let frame = glove_frame(side).to_matrix();
    let mut at = Mat4::from_translation(finger_root(side, finger));
    let spread = rest_spread(finger);
    let mut out = Vec::with_capacity(3);
    for (joint, len) in FINGER_BONES[finger].iter().enumerate() {
        if *len <= 0.0 {
            break;
        }
        at *= Mat4::from_quat(joint_rotation_spread(side, finger, joint, c, spread));
        let a = at.transform_point3(Vec3::ZERO);
        let b = at.transform_point3(Vec3::new(0.0, 0.0, -len));
        let r =
            (RADII[finger][joint] + RADII[finger][joint + 1].max(RADII[finger][joint] * 0.8)) * 0.5;
        out.push(Bone {
            a: frame.transform_point3(a),
            b: frame.transform_point3(b),
            r,
        });
        at *= Mat4::from_translation(Vec3::new(0.0, 0.0, -len));
    }
    out.into_iter()
}

/// The thumb's way (its first bone's, on the unrolled glove) curled `c`.
fn thumb_way(side: Side, c: f32) -> Vec3 {
    joint_rotation_spread(side, 4, 0, c, 0.0) * Vec3::NEG_Z
}

/// The thumb's bones in the hand's frame, its first bone pointing `dir`
/// on the unrolled glove and its last joint bent `bend` radians: as
/// `props::pose_hands` draws a thumb aimed (a tracked one, or one on a
/// catch).
fn thumb_bones(side: Side, dir: Vec3, bend: f32) -> impl Iterator<Item = Bone> {
    let frame = glove_frame(side).to_matrix();
    let root = Mat4::from_translation(finger_root(side, 4))
        * Mat4::from_quat(
            Quat::from_rotation_arc(Vec3::NEG_Z, dir.normalize())
                * Quat::from_rotation_z(0.6 * side.sign()),
        );
    let [l0, l1, _] = FINGER_BONES[4];
    let mid = root * Mat4::from_translation(Vec3::new(0.0, 0.0, -l0));
    let tip = mid * Mat4::from_quat(Quat::from_rotation_x(-bend));
    let r = |j: usize| (RADII[4][j] + RADII[4][j + 1]) * 0.5;
    [
        Bone {
            a: frame.transform_point3(root.transform_point3(Vec3::ZERO)),
            b: frame.transform_point3(mid.transform_point3(Vec3::ZERO)),
            r: r(0),
        },
        Bone {
            a: frame.transform_point3(tip.transform_point3(Vec3::ZERO)),
            b: frame.transform_point3(tip.transform_point3(Vec3::new(0.0, 0.0, -l1))),
            r: r(1),
        },
    ]
    .into_iter()
}

/// Points over the palm's underside and edges, in the hand's frame (the
/// glove's palm, `art/hands/gloves.py`: about 85 mm across, 25 mm
/// thick, from the knuckles to the wrist).
fn palm_points(side: Side) -> impl Iterator<Item = Vec3> {
    let frame = glove_frame(side).to_matrix();
    let xs = [-0.038, -0.019, 0.0, 0.019, 0.038];
    let zs = [-0.05, -0.025, 0.0, 0.025, 0.05];
    let ys = [-0.013, 0.0];
    xs.into_iter().flat_map(move |x| {
        zs.into_iter().flat_map(move |z| {
            ys.into_iter()
                .map(move |y| frame.transform_point3(Vec3::new(x, y, z)))
        })
    })
}

/// The thumb's root, the ball of the thumb on the palm's thumb side: as
/// much the palm as the thumb (`art/hands/gloves.py`'s thenar pad, its
/// root radius), lower than the rest of the palm and never moving with
/// the thumb. In the hand's frame, and its radius.
fn thenar(side: Side) -> (Vec3, f32) {
    (
        glove_frame(side).transform_point(finger_root(side, 4)),
        RADII[4][0],
    )
}

/// Which way the palm faces, in the hand's frame.
pub fn palm_normal(side: Side) -> Vec3 {
    glove_frame(side).rotation * Vec3::NEG_Y
}

/// Signed distance from `p` to a box of half-size `h` round the origin.
fn sdf(p: Vec3, h: Vec3) -> f32 {
    let q = p.abs() - h;
    q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
}

/// The box's outward face normal nearest `p`.
fn normal(p: Vec3, h: Vec3) -> Vec3 {
    let q = p.abs() - h;
    let out = q.max(Vec3::ZERO);
    if out != Vec3::ZERO {
        return (out * p.signum()).normalize();
    }
    if q.x >= q.y && q.x >= q.z {
        Vec3::X * p.x.signum()
    } else if q.y >= q.z {
        Vec3::Y * p.y.signum()
    } else {
        Vec3::Z * p.z.signum()
    }
}

/// How near a bone (in the box's frame) comes to the box's surface,
/// negative inside, and where on it. The box's signed distance is convex,
/// so along a segment a ternary search finds its least.
fn bone_gap(a: Vec3, b: Vec3, r: f32, h: Vec3) -> (f32, Vec3) {
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..12 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        if sdf(a.lerp(b, m1), h) < sdf(a.lerp(b, m2), h) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    let p = a.lerp(b, (lo + hi) * 0.5);
    (sdf(p, h) - r, p)
}

/// A hand's parts, index to little, the thumb, then the palm.
pub const PARTS: [&str; 6] = ["index", "middle", "ring", "little", "thumb", "palm"];

/// How a glove sits on a box: each part's least gap to it (negative,
/// sunk in) and the face it's nearest.
#[derive(Clone, Copy, Debug)]
pub struct Grasp {
    pub gap: [f32; 6],
    pub face: [Vec3; 6],
}

impl Grasp {
    /// How deep the deepest part sinks in.
    pub fn sink(&self) -> f32 {
        -self.gap.iter().copied().fold(0.0, f32::min)
    }

    pub fn touches(&self, part: usize) -> bool {
        self.gap[part] <= TOUCH
    }

    /// Fingers (not the thumb) touching.
    pub fn fingers(&self) -> usize {
        (0..4).filter(|&f| self.touches(f)).count()
    }

    /// The thumb closes against a finger: they touch faces turned apart.
    pub fn pinched(&self) -> bool {
        self.touches(4) && (0..4).any(|f| self.touches(f) && self.face[f].dot(self.face[4]) < -0.1)
    }

    /// Why it isn't held, if it isn't: sunk in, fingers off it, or a fist
    /// beside it (neither the palm on it nor the thumb against a finger).
    pub fn fault(&self) -> Option<String> {
        if self.sink() > SINK {
            let part = (0..6)
                .min_by(|a, b| self.gap[*a].total_cmp(&self.gap[*b]))
                .unwrap_or(0);
            return Some(format!(
                "{} sinks {:.1} mm in",
                PARTS[part],
                self.sink() * 1000.0
            ));
        }
        if !self.touches(4) || self.fingers() < 2 {
            let off: Vec<String> = (0..5)
                .filter(|&p| !self.touches(p))
                .map(|p| format!("{} {:.0} mm off", PARTS[p], self.gap[p] * 1000.0))
                .collect();
            return Some(off.join(", "));
        }
        if !self.touches(5) && !self.pinched() {
            return Some(format!(
                "a fist beside it (palm {:.0} mm off, no pinch)",
                self.gap[5] * 1000.0
            ));
        }
        None
    }
}

/// How a glove seated `seat` sits on a box of half-size `half` lying at `item` in the hand's frame.
pub fn grasp(side: Side, half: Vec3, item: Transform, seat: &Seat) -> Grasp {
    let to_box = item.compute_affine().inverse() * seat.glove().compute_affine();
    let mut gap = [f32::MAX; 6];
    let mut face = [Vec3::ZERO; 6];
    for f in 0..5 {
        let parts: Vec<Bone> = if f == 4 {
            thumb_bones(side, seat.thumb.0, seat.thumb.1).collect()
        } else {
            bones(side, f, seat.curl[f]).collect()
        };
        for b in parts {
            let a = to_box.transform_point3(b.a);
            let bb = to_box.transform_point3(b.b);
            let (g, p) = bone_gap(a, bb, b.r, half);
            if g < gap[f] {
                gap[f] = g;
                face[f] = normal(p, half);
            }
        }
    }
    let (ball, r) = thenar(side);
    let ball = (to_box.transform_point3(ball), r);
    for (q, r) in palm_points(side)
        .map(|p| (to_box.transform_point3(p), 0.0))
        .chain([ball])
    {
        let g = sdf(q, half) - r;
        if g < gap[5] {
            gap[5] = g;
            face[5] = normal(q, half);
        }
    }
    Grasp { gap, face }
}

/// Where a glove holding a thing free is drawn and how it closes: moved
/// `shift` in the hand's frame, curled `curl`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seat {
    pub shift: Vec3,
    /// The glove turned from the hand (about its origin, then moved
    /// `shift`): only where, as the hand is turned, the glove can't close
    /// on it at all (a rod end-on to the palm).
    pub turn: Quat,
    pub curl: [f32; 5],
    /// The thumb aimed (its way on the unrolled glove, its last joint's
    /// bend): raised toward the palm's plane to lie on a flat side, where
    /// curled it would swing into it.
    pub thumb: (Vec3, f32),
}

impl Seat {
    /// The glove's pose in the hand's frame.
    pub fn glove(&self) -> Transform {
        Transform::from_translation(self.shift).with_rotation(self.turn)
    }
}

/// How far the glove may slide to find a hold, metres, and the step.
const SLIDE: f32 = 0.035;
const SLIDE_STEP: f32 = 0.0175;
/// Turned, the glove may slide further: a thing end-on to the palm lies
/// off to one side of the turned fist.
const TURNED_SLIDE: f32 = 0.0525;
/// How far along its facing the palm may move onto (or off) a thing.
const PRESS: f32 = 0.12;
/// A finger that meets nothing as it closes: a relaxed fist.
const MISS_CURL: f32 = 0.7;

/// The curl at which a finger comes to rest on the box, given the glove's
/// `shift`: the first, from open, at which it's on the surface (closing
/// onto it from outside, or, if it starts in it, lifting out onto it, as
/// the thumb, below the palm, may); `MISS_CURL` if it never meets the
/// box, and `None` if it can't help but sink in.
fn close_on<I: Iterator<Item = Bone>>(
    at: impl Fn(f32) -> I,
    half: Vec3,
    to_box: &Affine3A,
    shift: Vec3,
) -> Option<f32> {
    let gap = |c: f32| {
        at(c)
            .map(|b| {
                bone_gap(
                    to_box.transform_point3(b.a + shift),
                    to_box.transform_point3(b.b + shift),
                    b.r,
                    half,
                )
                .0
            })
            .fold(f32::MAX, f32::min)
    };
    let on = |g: f32| (-SINK * 0.5..=TOUCH).contains(&g);
    // Where between two curls the gap crosses nought.
    let cross = |mut lo: f32, mut hi: f32| {
        let out = gap(lo) > 0.0;
        for _ in 0..7 {
            let m = (lo + hi) * 0.5;
            if (gap(m) > 0.0) == out {
                lo = m;
            } else {
                hi = m;
            }
        }
        if out { lo } else { hi }
    };
    let steps = 20;
    let mut prev = (0.0, gap(0.0));
    if on(prev.1) {
        return Some(0.0);
    }
    let mut ever_in = prev.1 < 0.0;
    for k in 1..=steps {
        let c = k as f32 / steps as f32;
        let g = gap(c);
        if on(g) {
            return Some(c);
        }
        if (g > 0.0) != (prev.1 > 0.0) {
            return Some(cross(prev.0, c));
        }
        ever_in |= g < 0.0;
        prev = (c, g);
    }
    (!ever_in).then_some(MISS_CURL)
}

/// The open hand's least gap to the box with the glove moved `shift`:
/// the palm's, and each finger's straight (not the thumb's: open, it hangs
/// below the palm, and would hold it off what it holds).
fn open_gap(side: Side, half: Vec3, to_box: &Affine3A, shift: Vec3) -> f32 {
    let (ball, r) = thenar(side);
    let palm = palm_points(side)
        .map(|p| sdf(to_box.transform_point3(p + shift), half))
        .fold(
            sdf(to_box.transform_point3(ball + shift), half) - r,
            f32::min,
        );
    (0..4)
        .flat_map(|f| bones(side, f, 0.0))
        .map(|b| {
            bone_gap(
                to_box.transform_point3(b.a + shift),
                to_box.transform_point3(b.b + shift),
                b.r,
                half,
            )
            .0
        })
        .fold(palm, f32::min)
}

/// Seats a glove on a box of half-size `half` held at `item` in the hand's
/// frame: of the places a few centimetres round the hand, the nearest
/// where, the palm pressed onto it and each finger closed until it
/// touches, the thing is held ([`Grasp::fault`]); failing all, the one
/// with most fingers on it.
pub fn seat(side: Side, half: Vec3, item: Transform) -> Seat {
    let (mut best, held) = seat_as(side, half, item, SLIDE);
    if held {
        return best;
    }
    // Turned to the nearest ways a hand could hold its box (each side of
    // it square to the palm and the fist), nearest first.
    let mut turns: Vec<Quat> = square_turns()
        .map(|m| item.rotation * m.inverse())
        .filter(|t| t.angle_between(Quat::IDENTITY) > 0.05)
        .collect();
    turns.sort_by(|a, b| {
        a.angle_between(Quat::IDENTITY)
            .total_cmp(&b.angle_between(Quat::IDENTITY))
    });
    let mut best_score = score(&grasp(side, half, item, &best));
    for turn in turns.into_iter().take(TURNS) {
        let inside = Transform::from_translation(turn.inverse() * item.translation)
            .with_rotation(turn.inverse() * item.rotation);
        let (s, held) = seat_as(side, half, inside, TURNED_SLIDE);
        let s = Seat {
            shift: turn * s.shift,
            turn,
            ..s
        };
        if held {
            return s;
        }
        let sc = score(&grasp(side, half, item, &s));
        if sc > best_score {
            (best, best_score) = (s, sc);
        }
    }
    best
}

/// How many turns of the glove `seat` tries before it gives up.
const TURNS: usize = 24;

/// A hold that isn't one, ranked: fingers and thumb on it.
fn score(g: &Grasp) -> usize {
    g.fingers() + usize::from(g.touches(4)) * 2
}

/// The 24 turns that lay a box's sides square to the hand's axes.
fn square_turns() -> impl Iterator<Item = Quat> {
    let axes = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ];
    axes.into_iter().flat_map(move |x| {
        axes.into_iter()
            .filter(move |y| x.dot(*y).abs() < 0.5)
            .map(move |y| Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y))))
    })
}

/// `seat` with the glove as the hand is turned, and whether it holds.
fn seat_as(side: Side, half: Vec3, item: Transform, slide: f32) -> (Seat, bool) {
    let to_box = item.compute_affine().inverse();
    let n = palm_normal(side);
    let u = n.any_orthonormal_vector();
    let w = n.cross(u);
    let reach = (slide / SLIDE_STEP).round() as i32;
    let mut spots: Vec<(i32, i32)> = (-reach..=reach)
        .flat_map(|a| (-reach..=reach).map(move |b| (a, b)))
        .collect();
    spots.sort_by_key(|(a, b)| a * a + b * b);
    let mut best: Option<(usize, Seat)> = None;
    for (a, b) in spots {
        let side_shift = (u * a as f32 + w * b as f32) * SLIDE_STEP;
        // The open hand onto it: the furthest along the palm's facing it
        // can go without sinking in (from standing off, in towards it).
        let gap = |s: f32| open_gap(side, half, &to_box, side_shift + n * s);
        // In from standing off to its first touch (straight through, a
        // small thing would be passed by).
        let mut prev = -PRESS;
        if gap(prev) <= 0.0 {
            continue;
        }
        let Some(first) = (1..=48)
            .map(|k| -PRESS + k as f32 * PRESS / 24.0)
            .find(|s| {
                let touch = gap(*s) <= 0.0;
                if !touch {
                    prev = *s;
                }
                touch
            })
        else {
            // Nothing under the hand here at all.
            continue;
        };
        let (mut lo, mut hi) = (prev, first);
        for _ in 0..10 {
            let m = (lo + hi) * 0.5;
            if gap(m) > 0.0 {
                lo = m;
            } else {
                hi = m;
            }
        }
        let shift = side_shift + n * lo;
        let mut curl = [0.0; 5];
        let mut open = true;
        for (f, c) in curl.iter_mut().enumerate().take(4) {
            match close_on(|c| bones(side, f, c), half, &to_box, shift) {
                Some(v) => *c = v,
                None => open = false,
            }
        }
        let Some(thumb) = close_thumb(side, half, &to_box, shift).filter(|_| open) else {
            continue;
        };
        curl[4] = thumb.2;
        let seat = Seat {
            shift,
            turn: Quat::IDENTITY,
            curl,
            thumb: (thumb.0, thumb.1),
        };
        let g = grasp(side, half, item, &seat);
        if g.fault().is_none() {
            return (seat, true);
        }
        let score = score(&g);
        if best.is_none_or(|(s, _)| score > s) {
            best = Some((score, seat));
        }
    }
    let fist = Seat {
        shift: Vec3::ZERO,
        turn: Quat::IDENTITY,
        curl: [MISS_CURL; 5],
        thumb: (thumb_way(side, MISS_CURL), THUMB_BEND * MISS_CURL),
    };
    (best.map_or(fist, |(_, s)| s), false)
}

/// A thing of half-size `half` as a hand takes it up: its thinnest side
/// to the palm, its length along the fist, its near side at the hand.
pub fn taken(side: Side, half: Vec3) -> Transform {
    let n = palm_normal(side);
    let mut axes = [(Vec3::X, half.x), (Vec3::Y, half.y), (Vec3::Z, half.z)];
    axes.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (thin, long) = (axes[0], axes[2]);
    // Its thinnest axis onto the palm's facing, its longest onto the
    // fist's (the hand's Y).
    let to = Mat3::from_cols(n, Vec3::Y, n.cross(Vec3::Y));
    let from = Mat3::from_cols(thin.0, long.0, thin.0.cross(long.0));
    let turn = Quat::from_mat3(&(to * from.transpose()));
    Transform::from_translation(n * thin.1).with_rotation(turn)
}

/// The thumb's last joint's bend, curled right in (`props::THUMB_BEND`).
const THUMB_BEND: f32 = 0.9;
/// How far the thumb may be raised toward the palm's plane to lie on a
/// flat side (added to its way's height on the unrolled glove).
const THUMB_LIFTS: [f32; 5] = [0.0, 0.12, 0.24, 0.36, 0.5];

/// The thumb closed onto the box: its way, bend and curl, raised no more
/// than it must be to rest on it (`close_on` for each lift in turn);
/// `None` if every way sinks in.
fn close_thumb(side: Side, half: Vec3, to_box: &Affine3A, shift: Vec3) -> Option<(Vec3, f32, f32)> {
    let way = |c: f32, lift: f32| (thumb_way(side, c) + Vec3::Y * lift).normalize();
    let mut missed = None;
    for lift in THUMB_LIFTS {
        let at = |c: f32| thumb_bones(side, way(c, lift), THUMB_BEND * c);
        match close_on(at, half, to_box, shift) {
            Some(c) if c != MISS_CURL => return Some((way(c, lift), THUMB_BEND * c, c)),
            Some(c) => {
                missed.get_or_insert((way(c, lift), THUMB_BEND * c, c));
            }
            None => {}
        }
    }
    missed
}

#[cfg(test)]
mod tests {
    use super::*;
    use extraction_core::items::{BUILTIN_CATALOG, ItemCatalog, ItemDef};

    /// Kinds a hand holds free, one-handed: not guns (their grips), not
    /// knives (a grip of their own), not what's carried in both arms.
    fn held_free(def: &ItemDef) -> bool {
        def.firearm.is_none() && def.melee.is_none() && !crate::interact::carry::is_bulky(def)
    }

    /// The kinds the first raid can drop (`from_raid` 0 on some table).
    fn first_raid(catalog: &ItemCatalog) -> Vec<String> {
        let text = std::fs::read_to_string(crate::asset_root_path().join("data/loot.ron"))
            .expect("loot.ron");
        let loot = extraction_core::loot::LootTables::from_ron(&text, catalog).expect("loot");
        let names: Vec<String> = loot.names().cloned().collect();
        let mut ids: Vec<String> = catalog
            .iter()
            .map(|(_, d)| d.id.clone())
            .filter(|id| names.iter().any(|n| loot.first_raid(n, id) == Some(0)))
            .collect();
        ids.sort();
        ids
    }

    /// The ways a hand takes hold of a thing: the item turned every way
    /// a box can lie in the hand, and some ways between.
    fn turns() -> Vec<Quat> {
        let mut out = Vec::new();
        let faces = [
            Quat::IDENTITY,
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            Quat::from_rotation_x(std::f32::consts::PI),
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
        ];
        for f in faces {
            for k in 0..4 {
                out.push(f * Quat::from_rotation_y(k as f32 * std::f32::consts::FRAC_PI_2));
            }
        }
        for (x, y, z) in [
            (0.4, 0.3, 0.0),
            (-0.5, 0.0, 0.35),
            (0.0, 0.7, -0.4),
            (0.8, -0.6, 0.2),
            (-0.3, 1.1, 0.6),
            (1.2, 0.2, -0.9),
        ] {
            out.push(Quat::from_euler(EulerRot::XYZ, x, y, z));
        }
        out
    }

    /// Where an item taken at `turn` lies in the hand: in front of the
    /// palm, its near side at the hand (as `interact` brings a free hold
    /// in, `PALM_REACH`), at its middle or towards one end.
    fn takes(side: Side, half: Vec3, turn: Quat) -> Vec<Transform> {
        let n = palm_normal(side);
        let rot = Mat3::from_quat(turn);
        let reach = (rot.x_axis * half.x).dot(n).abs()
            + (rot.y_axis * half.y).dot(n).abs()
            + (rot.z_axis * half.z).dot(n).abs();
        let long = [
            rot.x_axis * half.x,
            rot.y_axis * half.y,
            rot.z_axis * half.z,
        ]
        .into_iter()
        .map(|a| a - n * a.dot(n))
        .max_by(|a, b| a.length().total_cmp(&b.length()))
        .unwrap_or(Vec3::ZERO);
        let mut out = Vec::new();
        for off in [0.0, 0.03] {
            for along in [0.0, 0.6] {
                let at = n * (reach + off) + long * along;
                out.push(Transform::from_translation(at).with_rotation(turn));
            }
        }
        out
    }

    /// How the glove holds a box: seated on it.
    fn glove_on(side: Side, half: Vec3, item: Transform) -> Grasp {
        grasp(side, half, item, &seat(side, half, item))
    }

    /// Every kind a hand holds free is held: taken every way, in either
    /// hand, the glove sinks no more than `SINK` into its box, the thumb
    /// and two fingers touch it, and the palm is on it or the thumb closes
    /// against a finger. A new kind in the table is checked by being there.
    #[test]
    #[ignore = "in progress: the seat isn't on the glove yet"]
    fn every_free_held_kind_is_held_by_the_glove() {
        let catalog = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        let first = first_raid(&catalog);
        assert!(first.len() > 8, "first raid's loot: {first:?}");
        let mut bad = Vec::new();
        let mut checked = 0;
        let mut kinds = 0;
        for (_, def) in catalog.iter() {
            if !held_free(def) {
                continue;
            }
            kinds += 1;
            let half = Vec3::from(def.size_m()) * 0.5;
            let mut worst: Option<String> = None;
            let mut failed = 0;
            let mut tries = 0;
            for side in [Side::Right, Side::Left] {
                for turn in turns() {
                    for item in takes(side, half, turn) {
                        tries += 1;
                        checked += 1;
                        if let Some(why) = glove_on(side, half, item).fault() {
                            failed += 1;
                            worst.get_or_insert(why);
                        }
                    }
                }
            }
            if let Some(why) = worst {
                let tag = if first.contains(&def.id) {
                    " (first raid)"
                } else {
                    ""
                };
                bad.push(format!(
                    "{}{tag}: {failed} of {tries} holds fail, e.g. {why}",
                    def.id
                ));
            }
        }
        assert!(kinds > 100, "only {kinds} kinds held free");
        assert!(
            bad.is_empty(),
            "{} of {kinds} kinds not held ({checked} holds):\n{}",
            bad.len(),
            bad.join("\n")
        );
    }
}
