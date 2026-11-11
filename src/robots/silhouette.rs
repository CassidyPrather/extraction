//! A gauge of the machines' outlines: what each is as a solid black shape
//! against flat sky, the way a newcomer first meets it across a field
//! (`docs/craft/ROBOT_READING.md`: "body shape first").
//!
//! Each machine of the line-up (`dev`'s `--gallery robots:`), dressed and
//! standing as the game draws it, is cast as a mask from the line-ups'
//! six views: facing you, side-on and facing away, at 30 m and at 60 m.
//! A view's pixels are the headset's (14.3 a degree), and a pixel is the
//! machine's if more than half of it is covered: a spar or a mast thinner
//! than half a pixel at that range is not in its outline, as it is not in
//! the eye. Each pair of machines is then held together view by view: both
//! outlines scaled to one height (by their bodies: rows a hairline wide
//! don't count, [`Mask::body`]), standing on one floor, softened a little
//! ([`SOFTEN`]: the eye takes in a shape at range, not its pixels) and
//! fitted, one slid and scaled against the other ([`FIT_SCALES`],
//! [`FIT_SLIDE`]) for the most they overlap (IoU). Two machines over
//! [`CEILING`] in any one view, or on the mean of the views, are twins in
//! outline whatever their paint or detail says. A machine is scored only
//! as far off as it's met ([`reach`]: the crawler, indoors, at 30 m). (The walker and the
//! grappler, a walker with a launcher on top, scored 0.81; the next pair,
//! the walker and the crawler, its small indoor cousin, 0.55.)
//!
//! The test `no_two_machines_share_an_outline` holds every pair to it and,
//! with `EXTRACTION_SILHOUETTE_OUT=DIR`, writes the sheet (black on white,
//! a row a machine) and the table there.

use bevy::prelude::*;

/// The line-up's machines the gauge scores, in their line-up order.
pub const FAMILY: [&str; 7] = [
    "walker", "drone", "grappler", "sentry", "hound", "heavy", "crawler",
];
/// The views: how far each machine is turned from facing you (degrees),
/// and how far off it stands (m).
pub const TURNS: [(&str, f32); 3] = [
    ("facing you", 0.0),
    ("side-on", 90.0),
    ("facing away", 180.0),
];
pub const DISTANCES: [f32; 2] = [30.0, 60.0];

/// The farthest a machine is met (m): where it's seen, it's scored. The
/// crawler is the indoor machine, met down corridors and round corners,
/// never across a field: at 60 m its legs are under half a pixel and a
/// low flat body is all that's left, a drone's outline, at a range it
/// never stands at.
pub fn reach(name: &str) -> f32 {
    match name {
        "crawler" => 30.0,
        _ => 60.0,
    }
}
/// The headset's pixels a degree (`ROBOT_READING.md`).
pub const PX_PER_DEG: f32 = 14.3;
/// A mask's height once scaled (px).
pub const HEIGHT: usize = 40;
/// A scaled mask's width (px): room for the flattest machine.
pub const WIDTH: usize = HEIGHT * 8;
/// The most two machines' outlines may overlap, the mean of the views.
pub const CEILING: f32 = 0.7;
/// Sub-samples a pixel, each way.
const SUB: usize = 4;

/// A mask: `w` x `h`, row by row from the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub on: Vec<bool>,
}

impl Mask {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            on: vec![false; w * h],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        self.on[y * self.w + x]
    }

    pub fn count(&self) -> usize {
        self.on.iter().filter(|b| **b).count()
    }

    /// The rows and columns its body spans, (x0, y0, x1, y1), exclusive:
    /// those at least two pixels across, so a hairline (a mast, an aerial
    /// a pixel wide) doesn't set its size.
    pub fn body(&self) -> Option<(usize, usize, usize, usize)> {
        let rows: Vec<usize> = (0..self.h)
            .filter(|y| (0..self.w).filter(|x| self.get(*x, *y)).count() >= 2)
            .collect();
        let cols: Vec<usize> = (0..self.w)
            .filter(|x| (0..self.h).filter(|y| self.get(*x, *y)).count() >= 2)
            .collect();
        Some((
            *cols.first()?,
            *rows.first()?,
            cols.last()? + 1,
            rows.last()? + 1,
        ))
    }

    /// The rows and columns its pixels span: (x0, y0, x1, y1), exclusive.
    pub fn bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.get(x, y) {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                    y1 = y1.max(y + 1);
                }
            }
        }
        (x0 < x1).then_some((x0, y0, x1, y1))
    }
}

/// Pixels a metre at `dist` m.
pub fn px_per_m(dist: f32) -> f32 {
    PX_PER_DEG * (180.0 / std::f32::consts::PI) / dist
}

/// `tris` (world space) seen level along -Z from `dist` m, the machine
/// turned `turn` radians about the upright through `centre` first: a pixel
/// is on if more than half of it is covered.
pub fn cast(tris: &[[Vec3; 3]], centre: Vec3, turn: f32, dist: f32) -> Mask {
    let q = Quat::from_rotation_y(turn);
    let scale = px_per_m(dist) * SUB as f32;
    let flat: Vec<[Vec2; 3]> = tris
        .iter()
        .map(|t| {
            t.map(|p| {
                let p = q * (p - centre);
                Vec2::new(p.x, -p.y) * scale
            })
        })
        .collect();
    let (lo, hi) = flat
        .iter()
        .flatten()
        .fold((Vec2::MAX, Vec2::MIN), |(lo, hi), p| {
            (lo.min(*p), hi.max(*p))
        });
    if lo.x > hi.x {
        return Mask::new(1, 1);
    }
    // Whole pixels, the sub-samples of a pixel together.
    let lo = (lo / SUB as f32).floor() * SUB as f32 - Vec2::splat(SUB as f32);
    let w = (((hi.x - lo.x) / SUB as f32).ceil() as usize + 2) * SUB;
    let h = (((hi.y - lo.y) / SUB as f32).ceil() as usize + 2) * SUB;
    let mut fine = vec![false; w * h];
    for t in &flat {
        let [a, b, c] = t.map(|p| p - lo);
        let area = (b - a).perp_dot(c - a);
        if area.abs() < 1e-9 {
            continue;
        }
        let x0 = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
        let x1 = (a.x.max(b.x).max(c.x).ceil() as usize).min(w);
        let y0 = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
        let y1 = (a.y.max(b.y).max(c.y).ceil() as usize).min(h);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let e0 = (b - a).perp_dot(p - a) * area.signum();
                let e1 = (c - b).perp_dot(p - b) * area.signum();
                let e2 = (a - c).perp_dot(p - c) * area.signum();
                if e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0 {
                    fine[y * w + x] = true;
                }
            }
        }
    }
    let mut m = Mask::new(w / SUB, h / SUB);
    for y in 0..m.h {
        for x in 0..m.w {
            let mut n = 0;
            for sy in 0..SUB {
                for sx in 0..SUB {
                    n += fine[(y * SUB + sy) * w + x * SUB + sx] as usize;
                }
            }
            m.on[y * m.w + x] = n * 2 > SUB * SUB;
        }
    }
    m
}

/// A normalised mask's height (px): room over [`HEIGHT`] for the larger
/// of a fit's scales ([`FIT_SCALES`]).
pub const CANVAS: usize = HEIGHT * 2;

/// `m` cropped to its body ([`Mask::body`]), scaled to `k` times
/// [`HEIGHT`], standing on the floor of a [`WIDTH`] x [`CANVAS`] mask in
/// its middle (as it stands on the ground).
pub fn normalised(m: &Mask, k: f32) -> Mask {
    let mut out = Mask::new(WIDTH, CANVAS);
    let Some((x0, y0, x1, y1)) = m.body() else {
        return out;
    };
    let s = (y1 - y0) as f32 / (HEIGHT as f32 * k);
    let (bw, bh) = ((x1 - x0) as f32 / s, (y1 - y0) as f32 / s);
    let left = (WIDTH as f32 - bw) * 0.5;
    let top = CANVAS as f32 - bh;
    for y in 0..CANVAS {
        for x in 0..WIDTH {
            let sx = (x as f32 + 0.5 - left) * s;
            let sy = (y as f32 + 0.5 - top) * s;
            if sx < 0.0 || sy < 0.0 || sx >= (x1 - x0) as f32 || sy >= (y1 - y0) as f32 {
                continue;
            }
            out.on[y * WIDTH + x] = m.get(x0 + sx as usize, y0 + sy as usize);
        }
    }
    out
}

/// The scales one machine's outline is tried at against another's, and
/// how far across (px) it's slid: two machines are twins if some fit of
/// one on the other overlaps (a turret a little taller is not a new shape,
/// and a pole on top doesn't hide the same body under it).
pub const FIT_SCALES: [f32; 17] = [
    0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9, 0.95, 1.0, 1.05, 1.1, 1.18, 1.25, 1.33, 1.43, 1.54, 1.67,
];
pub const FIT_SLIDE: isize = 2;

/// How far (px, at [`HEIGHT`]) a mask is softened before two are held
/// together: what the eye takes in at range is a shape, not its pixels; a
/// leg a pixel or two wide one pixel over is the same leg.
pub const SOFTEN: usize = 2;

/// `m` as coverage, box-blurred [`SOFTEN`] px each way.
fn soft(m: &Mask) -> Vec<f32> {
    let r = SOFTEN as isize;
    let n = (2 * r + 1) as f32;
    let (w, h) = (m.w as isize, m.h as isize);
    let mut across = vec![0.0f32; m.w * m.h];
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for d in -r..=r {
                let xx = x + d;
                if (0..w).contains(&xx) && m.get(xx as usize, y as usize) {
                    s += 1.0;
                }
            }
            across[(y * w + x) as usize] = s / n;
        }
    }
    let mut out = vec![0.0f32; m.w * m.h];
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for d in -r..=r {
                let yy = y + d;
                if (0..h).contains(&yy) {
                    s += across[(yy * w + x) as usize];
                }
            }
            out[(y * w + x) as usize] = s / n;
        }
    }
    out
}

/// How much two softened masks overlap ([`SOFTEN`]), `q` slid `dx` px
/// across: shared coverage over either's.
fn overlap(p: &[f32], q: &[f32], w: usize, dx: isize) -> f32 {
    let (mut both, mut either) = (0.0f32, 0.0f32);
    for (i, a) in p.iter().enumerate() {
        let x = (i % w) as isize - dx;
        let b = if (0..w as isize).contains(&x) {
            q[i - (i % w) + x as usize]
        } else {
            0.0
        };
        both += a.min(b);
        either += a.max(b);
    }
    if either <= 0.0 { 1.0 } else { both / either }
}

/// How much two masks overlap, softened ([`SOFTEN`]): shared coverage
/// over either's (1 for one mask against itself).
pub fn iou(a: &Mask, b: &Mask) -> f32 {
    overlap(&soft(a), &soft(b), a.w, 0)
}

/// Two views of two machines (as cast), at their best fit: `b` scaled by
/// each of [`FIT_SCALES`] and slid by up to [`FIT_SLIDE`] against `a`.
pub fn fit(a: &Mask, b: &Mask) -> f32 {
    let p = soft(&normalised(a, 1.0));
    let mut best = 0.0f32;
    for k in FIT_SCALES {
        let q = soft(&normalised(b, k));
        for dx in -FIT_SLIDE..=FIT_SLIDE {
            best = best.max(overlap(&p, &q, WIDTH, dx));
        }
    }
    best
}

/// One machine's views as cast, in the order of [`DISTANCES`] then
/// [`TURNS`].
pub struct Views {
    /// `None` past the machine's [`reach`].
    pub cast: Vec<Option<Mask>>,
}

/// Every view of a machine whose drawn triangles at each distance are
/// `tris(dist)` (a far LOD for the far view), standing at `centre`, out
/// to `reach` m.
pub fn views(tris: impl Fn(f32) -> Vec<[Vec3; 3]>, centre: Vec3, reach: f32) -> Views {
    let mut cast_ = Vec::new();
    for d in DISTANCES {
        let t = (d <= reach).then(|| tris(d));
        for (_, turn) in TURNS {
            cast_.push(t.as_ref().map(|t| cast(t, centre, turn.to_radians(), d)));
        }
    }
    Views { cast: cast_ }
}

/// Two machines' overlap view by view ([`fit`]), where both are met.
pub fn per_view(a: &Views, b: &Views) -> Vec<Option<f32>> {
    a.cast
        .iter()
        .zip(&b.cast)
        .map(|(p, q)| Some(fit(p.as_ref()?, q.as_ref()?)))
        .collect()
}

/// Two machines' overlap, the mean of the views both are met in.
pub fn score(a: &Views, b: &Views) -> f32 {
    let v: Vec<f32> = per_view(a, b).into_iter().flatten().collect();
    v.iter().sum::<f32>() / v.len().max(1) as f32
}

/// Every pair over [`CEILING`] on the mean of its views, or in any one
/// view (a player meets a machine head-on or going away as often as
/// side-on: one view twinned is a twin): (a, b, mean, worst view).
pub fn twins<'a>(family: &'a [(String, Views)]) -> Vec<(&'a str, &'a str, f32, f32)> {
    let mut out = Vec::new();
    for (i, (a, va)) in family.iter().enumerate() {
        for (b, vb) in &family[i + 1..] {
            let v: Vec<f32> = per_view(va, vb).into_iter().flatten().collect();
            let mean = v.iter().sum::<f32>() / v.len().max(1) as f32;
            let worst = v.iter().copied().fold(0.0, f32::max);
            if mean > CEILING || worst > CEILING {
                out.push((a.as_str(), b.as_str(), mean, worst));
            }
        }
    }
    out
}

/// The sheet: a row a machine, its six views as cast (true size at each
/// distance), black on white, as a PGM.
pub fn sheet(family: &[(String, Views)]) -> Vec<u8> {
    let cell_w = family
        .iter()
        .flat_map(|(_, v)| v.cast.iter().flatten().map(|m| m.w))
        .max()
        .unwrap_or(1)
        + 8;
    let cell_h = family
        .iter()
        .flat_map(|(_, v)| v.cast.iter().flatten().map(|m| m.h))
        .max()
        .unwrap_or(1)
        + 8;
    let cols = DISTANCES.len() * TURNS.len();
    let (w, h) = (cell_w * cols, cell_h * family.len());
    let mut px = vec![255u8; w * h];
    for (r, (_, v)) in family.iter().enumerate() {
        for (c, m) in v.cast.iter().enumerate() {
            let Some(m) = m else { continue };
            // Standing on the cell's floor, in its middle.
            let ox = c * cell_w + (cell_w - m.w) / 2;
            let oy = r * cell_h + cell_h - 4 - m.h;
            for y in 0..m.h {
                for x in 0..m.w {
                    if m.get(x, y) {
                        px[(oy + y) * w + ox + x] = 0;
                    }
                }
            }
        }
    }
    let mut out = format!("P5\n{w} {h}\n255\n").into_bytes();
    out.extend(px);
    out
}

/// The normalised masks, a row a machine (what the score compares), as a
/// PGM.
pub fn norm_sheet(family: &[(String, Views)]) -> Vec<u8> {
    let cols = DISTANCES.len() * TURNS.len();
    let (w, h) = ((WIDTH + 4) * cols, (CANVAS + 4) * family.len());
    let mut px = vec![255u8; w * h];
    for (r, (_, v)) in family.iter().enumerate() {
        for (c, m) in v.cast.iter().enumerate() {
            let Some(m) = m.as_ref().map(|m| normalised(m, 1.0)) else {
                continue;
            };
            for y in 0..m.h {
                for x in 0..m.w {
                    if m.get(x, y) {
                        px[(r * (CANVAS + 4) + y) * w + c * (WIDTH + 4) + x] = 0;
                    }
                }
            }
        }
    }
    let mut out = format!("P5\n{w} {h}\n255\n").into_bytes();
    out.extend(px);
    out
}

/// The table: every pair's score, the worst first.
pub fn table(family: &[(String, Views)]) -> String {
    let mut rows = Vec::new();
    for (i, (a, va)) in family.iter().enumerate() {
        for (b, vb) in &family[i + 1..] {
            let v = per_view(va, vb);
            let per: Vec<String> = v
                .iter()
                .map(|x| x.map_or("-".into(), |x| format!("{x:.2}")))
                .collect();
            rows.push((score(va, vb), format!("{a}\t{b}"), per.join(" ")));
        }
    }
    rows.sort_by(|x, y| y.0.total_cmp(&x.0));
    let mut out = String::from("a\tb\tmean\tviews (30 m: you side away, 60 m: you side away)\n");
    for (s, ab, per) in rows {
        out += &format!("{ab}\t{s:.3}\t{per}\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::visibility::VisibilityRange;
    use bevy::mesh::{Indices, VertexAttributeValues};

    fn square(lo: Vec2, hi: Vec2) -> Vec<[Vec3; 3]> {
        let (a, b, c, d) = (
            Vec3::new(lo.x, lo.y, 0.0),
            Vec3::new(hi.x, lo.y, 0.0),
            Vec3::new(hi.x, hi.y, 0.0),
            Vec3::new(lo.x, hi.y, 0.0),
        );
        vec![[a, b, c], [a, c, d]]
    }

    #[test]
    fn a_shape_is_its_own_twin_and_a_hairline_is_no_outline() {
        let block = square(Vec2::new(-0.5, 0.0), Vec2::new(0.5, 2.0));
        let a = views(|_| block.clone(), Vec3::ZERO, 60.0);
        assert!((score(&a, &a) - 1.0).abs() < 1e-6);
        // The same block twice the size: the same outline.
        let big = square(Vec2::new(-1.0, 0.0), Vec2::new(1.0, 4.0));
        let b = views(|_| big.clone(), Vec3::ZERO, 60.0);
        assert!(score(&a, &b) > 0.9, "{}", score(&a, &b));
        // A mast 1 cm across on top of it is not in its outline at 30 m.
        let mut masted = block.clone();
        masted.extend(square(Vec2::new(-0.005, 2.0), Vec2::new(0.005, 3.0)));
        let m = cast(&masted, Vec3::ZERO, 0.0, 30.0);
        let plain = cast(&block, Vec3::ZERO, 0.0, 30.0);
        assert!(
            m.count() <= plain.count() + 2,
            "{} {}",
            m.count(),
            plain.count()
        );
        // A wide slab is not the block.
        let slab = square(Vec2::new(-2.0, 0.0), Vec2::new(2.0, 1.0));
        let c = views(|_| slab.clone(), Vec3::ZERO, 60.0);
        assert!(score(&a, &c) < 0.5, "{}", score(&a, &c));
    }

    /// The triangles a robot draws at `dist` m, world space: every mesh of
    /// it and its parts that's visible and opaque, at the LOD that range
    /// shows (`heavy_look`'s three).
    fn drawn(world: &World, robot: Entity, dist: f32) -> Vec<[Vec3; 3]> {
        let meshes = world.resource::<Assets<Mesh>>();
        let mats = world.resource::<Assets<StandardMaterial>>();
        let mut roots = vec![robot];
        for (e, p) in world
            .iter_entities()
            .filter_map(|e| Some((e.id(), e.get::<super::super::RobotPart>()?)))
        {
            if p.robot == robot {
                roots.push(e);
            }
        }
        // And what's drawn for it or a part of it apart from them (a
        // walker's leg segments, a heavy's limbs: `wear::DrawnFor`).
        let ours: std::collections::HashSet<Entity> = roots.iter().copied().collect();
        for (e, d) in world
            .iter_entities()
            .filter_map(|e| Some((e.id(), e.get::<super::super::wear::DrawnFor>()?)))
        {
            if ours.contains(&d.0) {
                roots.push(e);
            }
        }
        // (A sentry's mast and housing: `sentry::OfSentry`.)
        for (e, d) in world
            .iter_entities()
            .filter_map(|e| Some((e.id(), e.get::<super::super::sentry::OfSentry>()?)))
        {
            if d.0 == robot {
                roots.push(e);
            }
        }
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        let mut stack = roots;
        while let Some(e) = stack.pop() {
            if !seen.insert(e) {
                continue;
            }
            if let Some(c) = world.get::<Children>(e) {
                stack.extend(c.iter());
            }
            let Some(mesh) = world.get::<Mesh3d>(e) else {
                continue;
            };
            if world
                .get::<InheritedVisibility>(e)
                .is_some_and(|v| !v.get())
            {
                continue;
            }
            if let Some(r) = world.get::<VisibilityRange>(e) {
                let from = (r.start_margin.start + r.start_margin.end) * 0.5;
                let to = (r.end_margin.start + r.end_margin.end) * 0.5;
                if dist < from || dist >= to {
                    continue;
                }
            }
            if let Some(m) = world
                .get::<MeshMaterial3d<StandardMaterial>>(e)
                .and_then(|m| mats.get(&m.0))
                && matches!(
                    m.alpha_mode,
                    AlphaMode::Add | AlphaMode::Blend | AlphaMode::Premultiplied
                )
            {
                continue;
            }
            let (Some(mesh), Some(gt)) = (meshes.get(&mesh.0), world.get::<GlobalTransform>(e))
            else {
                continue;
            };
            let Some(VertexAttributeValues::Float32x3(ps)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                continue;
            };
            let ps: Vec<Vec3> = ps
                .iter()
                .map(|p| gt.transform_point(Vec3::from_array(*p)))
                .collect();
            let idx: Vec<usize> = match mesh.indices() {
                Some(Indices::U16(i)) => i.iter().map(|i| *i as usize).collect(),
                Some(Indices::U32(i)) => i.iter().map(|i| *i as usize).collect(),
                None => (0..ps.len()).collect(),
            };
            for t in idx.chunks_exact(3) {
                out.push([ps[t[0]], ps[t[1]], ps[t[2]]]);
            }
        }
        out
    }

    /// The family's line-up in the client (no window, no GPU), dressed
    /// and standing, cast from every view.
    fn family() -> Vec<(String, Views)> {
        let opts = crate::LaunchOptions {
            map: "training_yard".into(),
            map_explicit: true,
            new_game: true,
            robots: false,
            gallery: Some(format!("robots:{},0,30", FAMILY.join("+"))),
            camera: Some((Vec3::new(0.0, 0.0, 30.0), 0.0, -1.0)),
            ..Default::default()
        };
        let mut app = crate::soak::drive::client_app(opts);
        for _ in 0..600 {
            app.update();
        }
        let world = app.world_mut();
        let mut robots: Vec<(Entity, Vec3)> = world
            .query::<(Entity, &super::super::Robot, &GlobalTransform)>()
            .iter(world)
            .map(|(e, _, gt)| (e, gt.translation()))
            .collect();
        // The line runs left to right across the camera (+X).
        robots.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
        assert_eq!(
            robots.len(),
            FAMILY.len(),
            "the line-up stood {} machines",
            robots.len()
        );
        let world = app.world();
        FAMILY
            .iter()
            .zip(robots)
            .map(|(name, (e, at))| {
                let all = drawn(world, e, 30.0);
                assert!(!all.is_empty(), "{name} draws nothing");
                // Turned about its own middle (its outline's, in plan).
                let (lo, hi) = all
                    .iter()
                    .flatten()
                    .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| {
                        (lo.min(*p), hi.max(*p))
                    });
                let centre = Vec3::new((lo.x + hi.x) * 0.5, at.y, (lo.z + hi.z) * 0.5);
                (
                    name.to_string(),
                    views(|d| drawn(world, e, d), centre, reach(name)),
                )
            })
            .collect()
    }

    /// No two machines of the family share an outline: each pair's masks
    /// overlap no more than [`CEILING`] (`EXTRACTION_SILHOUETTE_OUT=DIR`
    /// for the sheet and the table).
    #[test]
    fn no_two_machines_share_an_outline() {
        let family = family();
        let table = table(&family);
        println!("{table}");
        if let Some(dir) = std::env::var_os("EXTRACTION_SILHOUETTE_OUT") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("silhouettes.pgm"), sheet(&family)).unwrap();
            std::fs::write(dir.join("silhouettes.tsv"), &table).unwrap();
            std::fs::write(dir.join("normalised.pgm"), norm_sheet(&family)).unwrap();
        }
        let twins = twins(&family);
        assert!(twins.is_empty(), "machines twinned in outline: {twins:?}");
    }
}
