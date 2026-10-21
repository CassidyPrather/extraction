//! Camera paths for the cinematic mode: a centripetal Catmull-Rom spline
//! through key points, walked at an even speed by arc length, with eased
//! starts and stops; and the `.cine` text format the shots are written in.
//!
//! Pure maths and parsing (no world): the shots' ground heights are filled
//! in by the caller through a `ground(x, z)` function.

use bevy::math::Vec3;

/// A height in a `.cine` file: `~h` is h metres above the ground under the
/// point, a bare number is absolute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Height {
    Above(f32),
    Abs(f32),
}

impl Height {
    pub fn resolve(self, ground: f32) -> f32 {
        match self {
            Height::Above(h) => ground + h,
            Height::Abs(y) => y,
        }
    }
}

/// A point as written: x, height, z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: Height,
    pub z: f32,
}

impl Point {
    pub fn resolve(self, ground: &dyn Fn(f32, f32) -> f32) -> Vec3 {
        Vec3::new(self.x, self.y.resolve(ground(self.x, self.z)), self.z)
    }

    /// `x,y,z` with `~` before y for "above the ground".
    pub fn parse(s: &str) -> Option<Self> {
        let v: Vec<&str> = s.split(',').map(str::trim).collect();
        if v.len() != 3 {
            return None;
        }
        let y = match v[1].strip_prefix('~') {
            Some(h) => Height::Above(h.parse().ok()?),
            None => Height::Abs(v[1].parse().ok()?),
        };
        Some(Self {
            x: v[0].parse().ok()?,
            y,
            z: v[2].parse().ok()?,
        })
    }
}

/// One shot as read from a file: its settings and either key points or a
/// generator (see `cinematic::generate`) to make them.
#[derive(Clone, Debug, Default)]
pub struct ShotSpec {
    pub name: String,
    /// Settings as `key=value`, in the order given.
    pub opts: Vec<(String, String)>,
    /// Camera key points and, where given, what each looks at.
    pub keys: Vec<(Point, Option<Point>)>,
}

impl ShotSpec {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.opts
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn num(&self, key: &str, default: f32) -> f32 {
        self.get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
}

/// Reads a `.cine` file:
///
/// ```text
/// # comment
/// shot gate_rise dur=10 ease=0.3 fov=60 hour=dusk weather=clearing
///   at 40,~2,262    look 40,~3,230
///   at 40,~30,270   look 20,~2,150
/// shot church_orbit gen=orbit target=church radius=35 height=14 turns=0.3 dur=12
/// ```
///
/// A `shot` line starts a shot and carries its settings; `at` lines under
/// it are its key points (`look` optional: a key without one keeps the
/// previous key's, and a shot with none looks along its path).
pub fn parse(text: &str) -> Result<Vec<ShotSpec>, String> {
    let mut shots: Vec<ShotSpec> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut words = line.split_whitespace();
        let err = |what: &str| format!("line {}: {what}: {raw:?}", n + 1);
        match words.next() {
            Some("shot") => {
                let name = words.next().ok_or_else(|| err("shot needs a name"))?;
                let mut opts = Vec::new();
                for w in words {
                    let (k, v) = w.split_once('=').ok_or_else(|| err("expected key=value"))?;
                    opts.push((k.to_string(), v.to_string()));
                }
                shots.push(ShotSpec {
                    name: name.to_string(),
                    opts,
                    keys: Vec::new(),
                });
            }
            Some("at") => {
                let shot = shots
                    .last_mut()
                    .ok_or_else(|| err("`at` before any shot"))?;
                let at = words
                    .next()
                    .and_then(Point::parse)
                    .ok_or_else(|| err("bad point"))?;
                let look = match words.next() {
                    Some("look") => Some(
                        words
                            .next()
                            .and_then(Point::parse)
                            .ok_or_else(|| err("bad look point"))?,
                    ),
                    None => None,
                    Some(_) => return Err(err("expected `look`")),
                };
                shot.keys.push((at, look));
            }
            Some(_) => return Err(err("expected `shot` or `at`")),
            None => {}
        }
    }
    Ok(shots)
}

/// Centripetal Catmull-Rom through `p`, with arc-length tables so it can be
/// walked at an even speed.
#[derive(Clone, Debug)]
pub struct Spline {
    pub points: Vec<Vec3>,
    /// (arc length, spline parameter) samples, increasing.
    table: Vec<(f32, f32)>,
}

const SAMPLES_PER_SEGMENT: usize = 64;

impl Spline {
    pub fn new(points: Vec<Vec3>) -> Self {
        let mut s = Self {
            points,
            table: Vec::new(),
        };
        let segs = s.segments();
        let mut len = 0.0;
        let mut last = s.at_param(0.0);
        s.table.push((0.0, 0.0));
        for i in 1..=segs * SAMPLES_PER_SEGMENT {
            let u = i as f32 / SAMPLES_PER_SEGMENT as f32;
            let p = s.at_param(u);
            len += p.distance(last);
            last = p;
            s.table.push((len, u));
        }
        s
    }

    pub fn segments(&self) -> usize {
        self.points.len().saturating_sub(1).max(1)
    }

    pub fn length(&self) -> f32 {
        self.table.last().map_or(0.0, |t| t.0)
    }

    /// The point at parameter `u` (0 at the first key, 1 at the second...).
    pub fn at_param(&self, u: f32) -> Vec3 {
        let n = self.points.len();
        match n {
            0 => return Vec3::ZERO,
            1 => return self.points[0],
            _ => {}
        }
        let u = u.clamp(0.0, (n - 1) as f32);
        let i = (u.floor() as usize).min(n - 2);
        let t = u - i as f32;
        let p1 = self.points[i];
        let p2 = self.points[i + 1];
        // Mirror the ends so the first and last segments have tangents.
        let p0 = if i > 0 {
            self.points[i - 1]
        } else {
            p1 * 2.0 - p2
        };
        let p3 = if i + 2 < n {
            self.points[i + 2]
        } else {
            p2 * 2.0 - p1
        };
        centripetal(p0, p1, p2, p3, t)
    }

    /// The parameter at arc length `s`.
    pub fn param_at_length(&self, s: f32) -> f32 {
        let t = &self.table;
        if t.len() < 2 {
            return 0.0;
        }
        let s = s.clamp(0.0, self.length());
        let i = t.partition_point(|e| e.0 < s).clamp(1, t.len() - 1);
        let (s0, u0) = t[i - 1];
        let (s1, u1) = t[i];
        if s1 - s0 <= f32::EPSILON {
            u1
        } else {
            u0 + (u1 - u0) * (s - s0) / (s1 - s0)
        }
    }
}

/// One centripetal Catmull-Rom segment (alpha 0.5): no cusps or loops
/// where key points bunch up, unlike the uniform kind.
fn centripetal(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let knot = |a: Vec3, b: Vec3| a.distance(b).sqrt().max(1e-4);
    let t0 = 0.0;
    let t1 = t0 + knot(p0, p1);
    let t2 = t1 + knot(p1, p2);
    let t3 = t2 + knot(p2, p3);
    let t = t1 + (t2 - t1) * t;
    let lerp = |a: Vec3, b: Vec3, ta: f32, tb: f32| {
        a * ((tb - t) / (tb - ta)) + b * ((t - ta) / (tb - ta))
    };
    let a1 = lerp(p0, p1, t0, t1);
    let a2 = lerp(p1, p2, t1, t2);
    let a3 = lerp(p2, p3, t2, t3);
    let b1 = lerp(a1, a2, t0, t2);
    let b2 = lerp(a2, a3, t1, t3);
    lerp(b1, b2, t1, t2)
}

/// Eased progress: speeds up from rest over the first `ramp` of the time,
/// holds an even speed, and slows to rest over the last `ramp`; sine ramps,
/// so the acceleration never jumps. `ramp` 0 is linear, 0.5 all ramp.
pub fn ease(x: f32, ramp: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let r = ramp.clamp(0.0, 0.5);
    if r <= 1e-4 {
        return x;
    }
    use std::f32::consts::PI;
    // Speed: 0.5(1 - cos(pi x / r)) on the ramps, 1 between.
    let ramp_area = |x: f32| 0.5 * (x - r / PI * (PI * x / r).sin());
    let total = 1.0 - r;
    let s = if x < r {
        ramp_area(x)
    } else if x <= 1.0 - r {
        r * 0.5 + (x - r)
    } else {
        total - ramp_area(1.0 - x)
    };
    s / total
}

/// A shot ready to fly: where the camera is and what it looks at, at any
/// fraction of the shot's time.
#[derive(Clone, Debug)]
pub struct Flight {
    pub path: Spline,
    /// What the camera looks at, per key; `None`: along the path.
    pub look: Option<Spline>,
    pub ramp: f32,
    /// Down-tilt when looking along the path, degrees.
    pub pitch: f32,
}

impl Flight {
    pub fn new(keys: Vec<(Vec3, Option<Vec3>)>, ramp: f32, pitch: f32) -> Self {
        let any_look = keys.iter().any(|k| k.1.is_some());
        let look = any_look.then(|| {
            // Keys without a look keep the one before (the first given one
            // for leading keys).
            let first = keys.iter().find_map(|k| k.1).unwrap_or(Vec3::ZERO);
            let mut last = first;
            Spline::new(
                keys.iter()
                    .map(|k| {
                        last = k.1.unwrap_or(last);
                        last
                    })
                    .collect(),
            )
        });
        Self {
            path: Spline::new(keys.into_iter().map(|k| k.0).collect()),
            look,
            ramp,
            pitch,
        }
    }

    /// Eye and look-at point at fraction `x` of the shot's time.
    pub fn at(&self, x: f32) -> (Vec3, Vec3) {
        let s = ease(x, self.ramp) * self.path.length();
        let u = self.path.param_at_length(s);
        let eye = self.path.at_param(u);
        let target = match &self.look {
            Some(look) => look.at_param(u),
            None => {
                let ahead = self.path.at_param(self.path.param_at_length(s + 2.0));
                let back = self.path.at_param(self.path.param_at_length(s - 2.0));
                let dir = (ahead - back).normalize_or(Vec3::NEG_Z);
                let flat = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::NEG_Z);
                let pitch = self.pitch.to_radians();
                eye + (flat * pitch.cos() - Vec3::Y * pitch.sin()) * 20.0
            }
        };
        (eye, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spline_passes_through_its_keys() {
        let pts = vec![
            Vec3::ZERO,
            Vec3::new(10.0, 2.0, 0.0),
            Vec3::new(10.0, 2.0, 30.0),
            Vec3::new(-5.0, 0.0, 31.0),
        ];
        let s = Spline::new(pts.clone());
        for (i, p) in pts.iter().enumerate() {
            assert!(s.at_param(i as f32).distance(*p) < 1e-3, "key {i}");
        }
        // Arc length: at least the chords, not wildly more.
        let chords: f32 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
        assert!(s.length() >= chords - 1e-3 && s.length() < chords * 1.3);
    }

    #[test]
    fn even_speed_by_arc_length() {
        // Keys bunched at one end: walking by length still moves evenly.
        let s = Spline::new(vec![
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(40.0, 0.0, 0.0),
        ]);
        let n = 40;
        let steps: Vec<f32> = (0..n)
            .map(|i| {
                let a = s.at_param(s.param_at_length(s.length() * i as f32 / n as f32));
                let b = s.at_param(s.param_at_length(s.length() * (i + 1) as f32 / n as f32));
                a.distance(b)
            })
            .collect();
        let mean = s.length() / n as f32;
        for st in steps {
            assert!((st - mean).abs() < mean * 0.05, "{st} vs {mean}");
        }
    }

    #[test]
    fn easing_starts_and_stops_at_rest() {
        for ramp in [0.0, 0.15, 0.3, 0.5] {
            assert!(ease(0.0, ramp).abs() < 1e-6);
            assert!((ease(1.0, ramp) - 1.0).abs() < 1e-5);
            let mut last = 0.0;
            for i in 1..=100 {
                let e = ease(i as f32 / 100.0, ramp);
                assert!(e >= last - 1e-6, "monotonic");
                last = e;
            }
        }
        // At rest at both ends, even speed in the middle.
        let d = 1e-3;
        assert!(ease(d, 0.3) / d < 0.01);
        assert!((1.0 - ease(1.0 - d, 0.3)) / d < 0.01);
        let mid = (ease(0.5 + d, 0.3) - ease(0.5 - d, 0.3)) / (2.0 * d);
        assert!((mid - 1.0 / 0.7).abs() < 1e-2, "{mid}");
    }

    #[test]
    fn parses_shots() {
        let text = "# x\nshot a dur=8 hour=dusk\n at 1,~2,3 look 4,5,6\n  at 7,8,9\n\
                    shot b gen=orbit target=church\n";
        let s = parse(text).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].get("hour"), Some("dusk"));
        assert_eq!(s[0].num("dur", 1.0), 8.0);
        assert_eq!(s[0].keys.len(), 2);
        assert_eq!(s[0].keys[0].0.y, Height::Above(2.0));
        assert_eq!(s[0].keys[1].1, None);
        assert_eq!(s[1].get("gen"), Some("orbit"));
        assert!(parse("at 1,2,3").is_err());
        assert!(parse("shot x\n at 1,2").is_err());
    }

    #[test]
    fn looking_along_the_path_tilts_down() {
        let f = Flight::new(
            vec![(Vec3::ZERO, None), (Vec3::new(0.0, 0.0, -50.0), None)],
            0.2,
            10.0,
        );
        let (eye, at) = f.at(0.5);
        let dir = (at - eye).normalize();
        assert!(dir.z < -0.9 && dir.y < -0.1 && dir.y > -0.25, "{dir}");
    }
}
