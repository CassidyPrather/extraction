//! Small 2D geometry helpers for map authoring: splines, polylines,
//! polygons. Map space is metres, `x` east and `z` south (Bevy's `-Z` is
//! north), so a 2D point here is `[x, z]`.

pub type P2 = [f32; 2];

pub fn sub(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn add(a: P2, b: P2) -> P2 {
    [a[0] + b[0], a[1] + b[1]]
}

pub fn scale(a: P2, s: f32) -> P2 {
    [a[0] * s, a[1] * s]
}

pub fn dot(a: P2, b: P2) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

pub fn len(a: P2) -> f32 {
    dot(a, a).sqrt()
}

pub fn dist(a: P2, b: P2) -> f32 {
    len(sub(a, b))
}

pub fn lerp(a: P2, b: P2, t: f32) -> P2 {
    add(a, scale(sub(b, a), t))
}

/// Hermite smoothstep on `0..=1`.
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Samples a centripetal-ish Catmull-Rom spline through `points` into a
/// polyline with roughly `step` metres between samples. Endpoints are
/// included; two-point inputs come back as a straight line.
pub fn catmull_rom(points: &[P2], step: f32) -> Vec<P2> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let mut out = Vec::new();
    let n = points.len();
    for i in 0..n - 1 {
        let p0 = points[i.saturating_sub(1)];
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = points[(i + 2).min(n - 1)];
        let seg = dist(p1, p2);
        let steps = ((seg / step.max(0.01)).ceil() as usize).max(1);
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * ((2.0 * b)
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push([f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]);
        }
    }
    out.push(points[n - 1]);
    out
}

/// Closest point on a polyline to `p`: returns `(distance, arc_length_at_closest, segment_index)`.
pub fn polyline_closest(line: &[P2], p: P2) -> (f32, f32, usize) {
    let mut best = (f32::INFINITY, 0.0, 0);
    let mut arc = 0.0;
    for i in 0..line.len().saturating_sub(1) {
        let a = line[i];
        let b = line[i + 1];
        let ab = sub(b, a);
        let l2 = dot(ab, ab);
        let t = if l2 > 0.0 {
            (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = add(a, scale(ab, t));
        let d = dist(p, q);
        let seg_len = l2.sqrt();
        if d < best.0 {
            best = (d, arc + t * seg_len, i);
        }
        arc += seg_len;
    }
    if line.len() == 1 {
        best = (dist(p, line[0]), 0.0, 0);
    }
    best
}

/// A polyline whose segments are kept in a grid of buckets, so that asking
/// many points how near they are only looks at the segments around each:
/// `closest` gives what `polyline_closest` gives (to the bit) for any point
/// within `reach` of the line, and may give `None` for one farther. A creek
/// or ridge across a valley asked about by every cell of its heightmap was
/// most of the terrain's build time.
pub struct NearLine<'a> {
    line: &'a [P2],
    /// Arc length at each point, summed as `polyline_closest` sums it.
    arc: Vec<f32>,
    lo: P2,
    cols: usize,
    rows: usize,
    buckets: Vec<Vec<u32>>,
}

impl<'a> NearLine<'a> {
    const BUCKET: f32 = 8.0;

    pub fn new(line: &'a [P2], reach: f32) -> Self {
        let mut arc = vec![0.0; line.len()];
        for i in 1..line.len() {
            let ab = sub(line[i], line[i - 1]);
            arc[i] = arc[i - 1] + dot(ab, ab).sqrt();
        }
        // A metre over, so a point at the reach's edge isn't lost to rounding.
        let pad = reach.max(0.0) + 1.0;
        let (lo, hi) = bounds(line);
        let lo = [lo[0] - pad, lo[1] - pad];
        let cells = |a: f32, b: f32| (((b - a) / Self::BUCKET).floor() as usize + 1).max(1);
        let (cols, rows) = if line.is_empty() {
            (0, 0)
        } else {
            (cells(lo[0], hi[0] + pad), cells(lo[1], hi[1] + pad))
        };
        let mut buckets = vec![Vec::new(); cols * rows];
        for i in 0..line.len().saturating_sub(1) {
            let (a, b) = bounds(&line[i..=i + 1]);
            let at = |v: f32, o: f32, n: usize| {
                (((v - o) / Self::BUCKET).floor().max(0.0) as usize).min(n - 1)
            };
            for r in at(a[1] - pad, lo[1], rows)..=at(b[1] + pad, lo[1], rows) {
                for c in at(a[0] - pad, lo[0], cols)..=at(b[0] + pad, lo[0], cols) {
                    buckets[r * cols + c].push(i as u32);
                }
            }
        }
        Self {
            line,
            arc,
            lo,
            cols,
            rows,
            buckets,
        }
    }

    /// `polyline_closest(line, p)`, or `None` when no segment is near.
    pub fn closest(&self, p: P2) -> Option<(f32, f32, usize)> {
        if self.line.len() < 2 {
            return (!self.line.is_empty()).then(|| polyline_closest(self.line, p));
        }
        let c = ((p[0] - self.lo[0]) / Self::BUCKET).floor();
        let r = ((p[1] - self.lo[1]) / Self::BUCKET).floor();
        if c < 0.0 || r < 0.0 || c as usize >= self.cols || r as usize >= self.rows {
            return None;
        }
        let near = &self.buckets[r as usize * self.cols + c as usize];
        let mut best = None::<(f32, f32, usize)>;
        // In order, as `polyline_closest` goes: the first of equals wins.
        for &i in near {
            let i = i as usize;
            let a = self.line[i];
            let ab = sub(self.line[i + 1], a);
            let l2 = dot(ab, ab);
            let t = if l2 > 0.0 {
                (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = dist(p, add(a, scale(ab, t)));
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, self.arc[i] + t * l2.sqrt(), i));
            }
        }
        best
    }
}

/// Total arc length of a polyline.
pub fn polyline_length(line: &[P2]) -> f32 {
    line.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Point and unit tangent at arc length `s` along a polyline.
pub fn polyline_at(line: &[P2], s: f32) -> (P2, P2) {
    let mut remaining = s.max(0.0);
    for w in line.windows(2) {
        let l = dist(w[0], w[1]);
        if remaining <= l || l == 0.0 {
            let t = if l > 0.0 { remaining / l } else { 0.0 };
            let dir = if l > 0.0 {
                scale(sub(w[1], w[0]), 1.0 / l)
            } else {
                [1.0, 0.0]
            };
            return (lerp(w[0], w[1], t), dir);
        }
        remaining -= l;
    }
    let n = line.len();
    if n >= 2 {
        let d = sub(line[n - 1], line[n - 2]);
        let l = len(d).max(1e-6);
        (line[n - 1], scale(d, 1.0 / l))
    } else {
        (line.first().copied().unwrap_or([0.0, 0.0]), [1.0, 0.0])
    }
}

/// Even-odd point-in-polygon test.
pub fn point_in_polygon(poly: &[P2], p: P2) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0];
            if p[0] < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Distance from `p` to a polygon's boundary (positive outside and inside).
pub fn polygon_edge_distance(poly: &[P2], p: P2) -> f32 {
    let mut closed = poly.to_vec();
    if let Some(first) = poly.first() {
        closed.push(*first);
    }
    polyline_closest(&closed, p).0
}

/// Axis-aligned bounds of a point set: `(min, max)`.
pub fn bounds(points: &[P2]) -> (P2, P2) {
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for p in points {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spline_passes_through_its_points() {
        let pts = [[0.0, 0.0], [10.0, 5.0], [20.0, 0.0]];
        let line = catmull_rom(&pts, 1.0);
        assert_eq!(line.first(), Some(&pts[0]));
        assert_eq!(line.last(), Some(&pts[2]));
        assert!(line.iter().any(|p| dist(*p, pts[1]) < 1e-4));
    }

    #[test]
    fn closest_point_on_a_line() {
        let line = [[0.0, 0.0], [10.0, 0.0]];
        let (d, s, _) = polyline_closest(&line, [4.0, 3.0]);
        assert!((d - 3.0).abs() < 1e-5);
        assert!((s - 4.0).abs() < 1e-5);
    }

    #[test]
    fn polygon_containment() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert!(point_in_polygon(&sq, [5.0, 5.0]));
        assert!(!point_in_polygon(&sq, [15.0, 5.0]));
        assert!((polygon_edge_distance(&sq, [5.0, 5.0]) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn walking_a_polyline() {
        let line = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]];
        assert_eq!(polyline_length(&line), 20.0);
        let (p, dir) = polyline_at(&line, 15.0);
        assert!(dist(p, [10.0, 5.0]) < 1e-5);
        assert!(dist(dir, [0.0, 1.0]) < 1e-5);
    }

    /// A bucketed line answers as the whole line does, to the bit, for
    /// every point within its reach (and says nothing wrong beyond it).
    #[test]
    fn near_line_answers_as_the_whole_line() {
        let mut rng = crate::noise::Rng::new(7);
        let points: Vec<P2> = (0..12)
            .map(|_| [rng.range(-300.0, 300.0), rng.range(-300.0, 300.0)])
            .collect();
        let line = catmull_rom(&points, 1.0);
        for reach in [0.0, 3.5, 16.0, 40.0] {
            let near = NearLine::new(&line, reach);
            for _ in 0..20_000 {
                let p = [rng.range(-360.0, 360.0), rng.range(-360.0, 360.0)];
                let whole = polyline_closest(&line, p);
                match near.closest(p) {
                    Some(got) if whole.0 <= reach => {
                        assert_eq!(got.0.to_bits(), whole.0.to_bits());
                        assert_eq!(got.1.to_bits(), whole.1.to_bits());
                        assert_eq!(got.2, whole.2);
                    }
                    Some(got) => assert!(got.0 >= whole.0 && got.0 > reach),
                    None => assert!(whole.0 > reach, "{p:?} at {} lost", whole.0),
                }
            }
        }
    }
}
