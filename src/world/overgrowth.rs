//! Overgrowth: ivy and creepers grown over the Zone's buildings, years of
//! them. Visual only; `buildings::dress_buildings` draws what [`grow`]
//! returns with the building's other near meshes.
//!
//! Space colonisation on each climbable face of a building's pieces (its
//! walls' outsides, posts, pylon legs): attraction points scattered over
//! the face up to the height this building's ivy has reached, vines grown
//! from roots at the face's foot towards them, a step at a time, each
//! node pulled by the points nearest it, points reached removed. Faces
//! are favoured where ivy thrives: shaded (low [`ao`] light), open to the
//! sky (rain reaches them), facing north. Everything comes from a seed
//! (the building's place), so a building grows the same every time.

use bevy::prelude::*;

use super::ao::Occluders;
use super::buildings::{Geo, Shape};
use crate::render::materials::Mat;

/// One vine: its nodes from the root up, and each node's radius.
#[derive(Clone, Debug, PartialEq)]
pub struct Vine {
    pub points: Vec<Vec3>,
    pub radius: Vec<f32>,
    /// The face it grows on (its outward normal), for lying flat to it.
    pub normal: Vec3,
}

/// A leaf card: where, facing which way, how big, how far along its vine
/// (the tips sway the most).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaf {
    pub at: Vec3,
    pub normal: Vec3,
    pub up: Vec3,
    pub size: f32,
    pub sway: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Growth {
    pub vines: Vec<Vine>,
    pub leaves: Vec<Leaf>,
}

/// Whether the Zone's decay is drawn (ivy, moss, stains, breakage):
/// `EXTRACTION_NO_DECAY=1` shows the buildings as built, for comparisons.
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("EXTRACTION_NO_DECAY").map_or(true, |v| v != "1"))
}

/// A building's seed, from where it stands: the same building grows the
/// same ivy every time.
pub fn seed(at: Vec2, prefab: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in prefab.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    let q = |v: f32| (v * 10.0).round() as i64 as u64;
    (h ^ q(at.x).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ q(at.y).wrapping_mul(0xc2b2_ae3d_27d4_eb4f))
        .max(1)
}

/// Materials ivy climbs: walls, posts, masts and pylons (not glass, cloth,
/// or the robots' shells).
fn climbable(m: Mat) -> bool {
    matches!(
        m,
        Mat::Planks
            | Mat::LogWall
            | Mat::Brick
            | Mat::Plaster
            | Mat::Concrete
            | Mat::RustyMetal
            | Mat::PaintedMetal
            | Mat::Corrugated
            | Mat::Timber
            | Mat::Charred
            | Mat::SootBrick
            | Mat::SootPlaster
            | Mat::DressedStone
    )
}

/// A face of a box piece: centre, outward normal, its two in-plane axes
/// (`v` the one nearest straight up) and half sizes along them.
#[derive(Clone, Copy, Debug)]
struct Face {
    c: Vec3,
    n: Vec3,
    u: Vec3,
    v: Vec3,
    hu: f32,
    hv: f32,
}

impl Face {
    fn point(&self, a: f32, b: f32) -> Vec3 {
        self.c + self.u * a + self.v * b
    }

    /// The face's foot: lowest point along `v` (v points up the face).
    fn foot(&self) -> f32 {
        (self.c - self.v * self.hv).y
    }
}

fn faces(size: Vec3, at: &Transform) -> [Face; 4] {
    let h = size * 0.5;
    let r = at.rotation;
    let c = at.translation;
    // The box's four sides around its local Y (its "up" axis).
    let mk = |n: Vec3, u: Vec3, hn: f32, hu: f32| Face {
        c: c + r * (n * hn),
        n: r * n,
        u: r * u,
        v: r * Vec3::Y,
        hu,
        hv: h.y,
    };
    [
        mk(Vec3::X, Vec3::Z, h.x, h.z),
        mk(Vec3::NEG_X, Vec3::Z, h.x, h.z),
        mk(Vec3::Z, Vec3::X, h.z, h.x),
        mk(Vec3::NEG_Z, Vec3::X, h.z, h.x),
    ]
}

/// How much ivy a building has: 0 none, 1 smothered. From its seed, so
/// some buildings are bare and a few are green to the eaves.
/// How overgrown the building `prefab` standing at `at` (map xz) is, 0
/// (bare) to 1 (smothered): the vigour its ivy and weeds grow with, the
/// same on the server and every client. 0 everywhere with decay off.
/// For Audio's rustle in the leaves.
pub fn amount(at: Vec2, prefab: &str) -> f32 {
    if enabled() {
        vigour(seed(at, prefab))
    } else {
        0.0
    }
}

fn vigour(seed: u64) -> f32 {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x51ed);
    let r = rng.f32();
    if r < 0.12 {
        0.0
    } else {
        ((r - 0.12) / 0.88).powf(0.7)
    }
}

/// The ivy on a building: its pieces, its occlusion and sky (`occ`), its
/// seed, and `north` (building-local) for the shaded side.
pub fn grow(geo: &Geo, occ: &Occluders, seed: u64, north: Vec3) -> Growth {
    let mut out = Growth::default();
    let v = vigour(seed);
    if v <= 0.0 {
        return out;
    }
    let mut rng = fastrand::Rng::with_seed(seed);
    // The faces ivy can climb: upright-ish sides of climbable solid boxes,
    // standing on the ground, not buried in another piece.
    let mut cands: Vec<(Face, f32)> = Vec::new();
    for p in &geo.pieces {
        let Shape::Box(s) = p.shape else { continue };
        if !p.collide || !climbable(p.mat) {
            continue;
        }
        for f in faces(s, &p.at) {
            if f.n.y.abs() > 0.5 || f.v.y < 0.6 || f.hu < 0.05 {
                continue;
            }
            // Its foot near the ground (the plinth's top at most).
            if f.foot() > 0.8 || f.c.y + f.hv * f.v.y < 0.6 {
                continue;
            }
            let probe = f.c + f.n * 0.06;
            if occ.inside(probe) || occ.inside(f.point(0.0, -f.hv * 0.8) + f.n * 0.06) {
                continue;
            }
            // Ivy likes shade, rain and the north.
            let low = f.point(0.0, -f.hv * 0.5) + f.n * 0.03;
            let shade = 1.0 - occ.light(low, f.n);
            let sky = occ.sky(f.point(0.0, f.hv * 0.9) + f.n * 0.05, Vec3::Y);
            let north_w = f.n.dot(north).max(0.0);
            let w = (0.35 + shade * 1.2 + sky * 0.4 + north_w * 0.9) * f.hu.min(3.0);
            cands.push((f, w));
        }
    }
    if cands.is_empty() {
        return out;
    }
    // How many patches, how high this building's ivy has reached.
    let total: f32 = cands.iter().map(|(_, w)| w).sum();
    let patches = (2.0 + v * 9.0).round() as usize;
    let reach = 1.2 + v * 4.5;
    for _ in 0..patches {
        // Pick a face by weight.
        let mut r = rng.f32() * total;
        let mut pick = 0;
        for (i, (_, w)) in cands.iter().enumerate() {
            r -= w;
            if r <= 0.0 {
                pick = i;
                break;
            }
        }
        let f = cands[pick].0;
        let (nv, nl) = (out.vines.len(), out.leaves.len());
        colonise(&f, occ, reach, v, &mut rng, &mut out);
        // Within the budget: a patch that would overrun it isn't grown.
        if triangles(&out) > BUDGET {
            out.vines.truncate(nv);
            out.leaves.truncate(nl);
            break;
        }
    }
    out
}

/// Grow one patch of ivy up a face.
fn colonise(
    f: &Face,
    occ: &Occluders,
    reach: f32,
    vigour: f32,
    rng: &mut fastrand::Rng,
    out: &mut Growth,
) {
    // In the face's own coordinates: a along u (-hu..hu), b along v from
    // the foot (0..2 hv).
    let height = (2.0 * f.hv).min(reach * rng.f32().mul_add(0.5, 0.75));
    let width = (2.0 * f.hu).min(1.0 + vigour * 3.2);
    let a0 = rng
        .f32()
        .mul_add(2.0 * f.hu - width, -f.hu + width * 0.5)
        .clamp(-f.hu, f.hu)
        - 0.0;
    let to3 = |a: f32, b: f32| f.point(a, b - f.hv) + f.n * 0.015;
    // Attraction points, thinning towards the top.
    let mut pts: Vec<Vec2> = (0..(30.0 + 90.0 * vigour) as usize)
        .map(|_| {
            let t = rng.f32();
            Vec2::new(
                a0 + (rng.f32() - 0.5) * width * (0.6 + 0.4 * t),
                height * (1.0 - (1.0 - rng.f32()).powf(0.7)),
            )
        })
        // Not where something's in the way (a window's sill, a pipe).
        .filter(|p| !occ.inside(to3(p.x, p.y) + f.n * 0.04))
        .collect();
    // Nodes: (position, parent, descendants).
    const STEP: f32 = 0.13;
    const INFLUENCE: f32 = 0.9;
    const KILL: f32 = 0.2;
    let roots = 1 + (rng.f32() * 2.5 * vigour) as usize;
    let mut nodes: Vec<(Vec2, Option<usize>)> = (0..roots)
        .map(|k| {
            (
                Vec2::new(a0 + (k as f32 - (roots - 1) as f32 * 0.5) * 0.35, 0.02),
                None,
            )
        })
        .collect();
    for _ in 0..90 {
        if pts.is_empty() {
            break;
        }
        // Each point pulls its nearest node, if within reach.
        let mut pull = vec![Vec2::ZERO; nodes.len()];
        let mut count = vec![0u32; nodes.len()];
        for p in &pts {
            let mut best = (f32::MAX, usize::MAX);
            for (i, (q, _)) in nodes.iter().enumerate() {
                let d = p.distance_squared(*q);
                if d < best.0 {
                    best = (d, i);
                }
            }
            if best.0 < INFLUENCE * INFLUENCE {
                pull[best.1] += (*p - nodes[best.1].0).normalize_or_zero();
                count[best.1] += 1;
            }
        }
        let mut grew = false;
        for i in 0..nodes.len() {
            if count[i] == 0 {
                continue;
            }
            // Ivy climbs: a little extra up.
            let dir = (pull[i].normalize_or_zero() + Vec2::new(0.0, 0.35)).normalize_or_zero();
            let p = nodes[i].0 + dir * STEP;
            if p.x.abs() > f.hu - 0.02 || p.y > 2.0 * f.hv - 0.02 || p.y < 0.0 {
                continue;
            }
            if occ.inside(to3(p.x, p.y) + f.n * 0.04) {
                continue;
            }
            nodes.push((p, Some(i)));
            grew = true;
        }
        pts.retain(|p| {
            !nodes
                .iter()
                .any(|(q, _)| p.distance_squared(*q) < KILL * KILL)
        });
        if !grew {
            break;
        }
    }
    if nodes.len() <= roots {
        return;
    }
    // Thickness: older stems (more below them) are thicker.
    let mut kids = vec![0u32; nodes.len()];
    let mut leaf = vec![true; nodes.len()];
    for i in (0..nodes.len()).rev() {
        if let Some(p) = nodes[i].1 {
            kids[p] += kids[i] + 1;
            leaf[p] = false;
        }
    }
    // Vines: from each tip back to a branch point (or the root).
    let mut used = vec![false; nodes.len()];
    let mut tips: Vec<usize> = (0..nodes.len()).filter(|&i| leaf[i]).collect();
    tips.sort_by_key(|&i| std::cmp::Reverse(depth(&nodes, i)));
    for tip in tips {
        let mut chain = vec![tip];
        let mut i = tip;
        while let Some(p) = nodes[i].1 {
            chain.push(p);
            if used[p] {
                break;
            }
            i = p;
        }
        for &k in &chain[..chain.len() - 1] {
            used[k] = true;
        }
        chain.reverse();
        if chain.len() < 2 {
            continue;
        }
        let radius = chain
            .iter()
            .map(|&k| (0.006 + 0.0025 * (kids[k] as f32).sqrt()).min(0.03))
            .collect();
        out.vines.push(Vine {
            points: chain
                .iter()
                .map(|&k| to3(nodes[k].0.x, nodes[k].0.y))
                .collect(),
            radius,
            normal: f.n,
        });
    }
    // Leaves along the stems, thicker towards the tips.
    for (i, (p, parent)) in nodes.iter().enumerate() {
        if parent.is_none() {
            continue;
        }
        let n = if leaf[i] {
            3
        } else {
            1 + (rng.f32() < 0.6 * vigour) as usize
        };
        for _ in 0..n {
            let off = Vec2::new(rng.f32() - 0.5, rng.f32() - 0.5) * 0.14;
            let q = *p + off;
            if q.x.abs() > f.hu || q.y < 0.05 || q.y > 2.0 * f.hv {
                continue;
            }
            let tilt = (rng.f32() - 0.5) * 1.2;
            let up = (f.v * tilt.cos() + f.u * tilt.sin()).normalize();
            out.leaves.push(Leaf {
                at: to3(q.x, q.y) + f.n * (0.02 + rng.f32() * 0.04),
                normal: (f.n + up * -0.35 + f.u * (rng.f32() - 0.5) * 0.4).normalize(),
                up,
                size: 0.11 + rng.f32() * 0.09,
                sway: (q.y / height).clamp(0.0, 1.0) * if leaf[i] { 1.0 } else { 0.6 },
            });
        }
    }
}

fn depth(nodes: &[(Vec2, Option<usize>)], mut i: usize) -> usize {
    let mut d = 0;
    while let Some(p) = nodes[i].1 {
        d += 1;
        i = p;
    }
    d
}

/// A young tree or a bush, grown into `out`: stems up from `at` (its
/// foot, building-local), leaf cards along their upper parts.
fn sapling(out: &mut Growth, at: Vec3, height: f32, stems: usize, rng: &mut fastrand::Rng) {
    // Within the budget: one that would overrun it isn't grown.
    let (nv, nl) = (out.vines.len(), out.leaves.len());
    grow_sapling(out, at, height, stems, rng);
    if triangles(out) > WILD_BUDGET {
        out.vines.truncate(nv);
        out.leaves.truncate(nl);
    }
}

fn grow_sapling(out: &mut Growth, at: Vec3, height: f32, stems: usize, rng: &mut fastrand::Rng) {
    for s in 0..stems {
        let lean =
            Vec3::new(rng.f32() - 0.5, 0.0, rng.f32() - 0.5) * if stems > 1 { 0.9 } else { 0.25 };
        let h = height * (0.6 + 0.4 * rng.f32()) * if s == 0 { 1.0 } else { 0.8 };
        let n = 6;
        let mut points = Vec::new();
        let mut radius = Vec::new();
        let side = Vec3::new(lean.z, 0.0, -lean.x).normalize_or(Vec3::X);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let bend = lean * (t * t) * h * 0.5;
            points.push(at + Vec3::Y * (t * h) + bend);
            radius.push((0.004 + 0.012 * height * (1.0 - t)).max(0.004));
        }
        // Leaves up the top two thirds, thickest at the tip.
        for i in 2..=n {
            let t = i as f32 / n as f32;
            let c = points[i];
            for _ in 0..(2 + (t * 3.0) as usize) {
                let off = Vec3::new(rng.f32() - 0.5, rng.f32() * 0.3 - 0.1, rng.f32() - 0.5)
                    * (0.15 + 0.35 * t)
                    * height.min(1.6);
                let normal =
                    Vec3::new(rng.f32() - 0.5, 0.6 + rng.f32(), rng.f32() - 0.5).normalize();
                let up = (Vec3::Y - normal * normal.y).normalize_or(Vec3::X);
                out.leaves.push(Leaf {
                    at: c + off,
                    normal,
                    up,
                    size: 0.1 + rng.f32() * 0.08,
                    sway: t,
                });
            }
        }
        out.vines.push(Vine {
            points,
            radius,
            normal: side,
        });
    }
}

/// Nature taking the inside back too: saplings up through a ruin's floor
/// where the roof's open over it, bushes at a house's feet, weeds in its
/// gutters. Clear of its doorways, its loot and its furniture.
pub fn reclaim(geo: &Geo, occ: &Occluders, seed: u64, ruin: bool) -> Growth {
    let mut out = Growth::default();
    let v = vigour(seed);
    if v <= 0.0 {
        return out;
    }
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x7ee5);
    // Where the building is: its pieces' bounds.
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in &geo.pieces {
        if let Shape::Box(s) = p.shape {
            if p.collide && s.y > 1.5 {
                lo = lo.min(p.at.translation - s * 0.5);
                hi = hi.max(p.at.translation + s * 0.5);
            }
        }
    }
    if lo.x > hi.x {
        return out;
    }
    let clear_of_ways = |p: Vec3| {
        geo.entrances
            .iter()
            .all(|e| Vec2::new(e.at[0], e.at[1]).distance(p.xz()) > 1.6)
            && geo.loot.iter().all(|l| l.at.xz().distance(p.xz()) > 0.9)
            && geo
                .doors
                .iter()
                .all(|d| d.hinge.xz().distance(p.xz()) > d.width + 0.6)
    };
    // Room for it: nothing solid round its stem.
    let free = |p: Vec3, h: f32| {
        (1..=4).all(|k| {
            let y = p.y + h * k as f32 / 4.0;
            [
                Vec3::ZERO,
                Vec3::X * 0.2,
                Vec3::NEG_X * 0.2,
                Vec3::Z * 0.2,
                Vec3::NEG_Z * 0.2,
            ]
            .iter()
            .all(|o| !occ.inside(Vec3::new(p.x, y, p.z) + *o))
        })
    };
    // Inside a ruin: up through its floor, under open sky.
    if ruin {
        for _ in 0..40 {
            if out.vines.len() >= 3 + (v * 6.0) as usize {
                break;
            }
            let p = Vec3::new(
                lo.x + 0.4 + rng.f32() * (hi.x - lo.x - 0.8),
                0.0,
                lo.z + 0.4 + rng.f32() * (hi.z - lo.z - 0.8),
            );
            // The floor: down from above to the first thing it meets.
            let Some(floor) = (0..30)
                .map(|k| 2.5 - k as f32 * 0.1)
                .find(|y| occ.inside(Vec3::new(p.x, *y, p.z)))
            else {
                continue;
            };
            let foot = Vec3::new(p.x, floor + 0.08, p.z);
            if occ.sky(foot, Vec3::Y) < 0.5 || !clear_of_ways(foot) {
                continue;
            }
            let h = 0.8 + rng.f32() * 1.6;
            if !free(foot, h.min(2.0)) {
                continue;
            }
            let stems = if rng.f32() < 0.5 {
                1
            } else {
                3 + (rng.f32() * 3.0) as usize
            };
            sapling(
                &mut out,
                foot,
                if stems > 1 { h * 0.5 } else { h },
                stems,
                &mut rng,
            );
        }
    }
    // Weeds in the gutters: small clumps along the roofs' lowest edges.
    for p in &geo.pieces {
        let Shape::Box(s) = p.shape else { continue };
        if !matches!(p.mat, Mat::Shingles | Mat::Corrugated) || p.at.rotation.is_near_identity() {
            continue;
        }
        let (r, c, h) = (p.at.rotation, p.at.translation, s * 0.5);
        // The slab's two long edges along local x's ends; the lower one.
        let ends = [r * Vec3::new(h.x, h.y, 0.0), r * Vec3::new(-h.x, h.y, 0.0)];
        let low = if ends[0].y < ends[1].y {
            ends[0]
        } else {
            ends[1]
        };
        let along = r * Vec3::Z;
        let n = (s.z / 1.6) as usize;
        for k in 0..n {
            if rng.f32() > 0.25 * v + 0.05 {
                continue;
            }
            let t = (k as f32 + rng.f32()) / n.max(1) as f32 - 0.5;
            let foot = c + low + along * (t * s.z * 0.9);
            sapling(&mut out, foot, 0.2 + rng.f32() * 0.2, 3, &mut rng);
        }
    }
    // Bushes at the walls' feet outside.
    let bushes = (v * 4.0) as usize;
    for _ in 0..bushes * 4 {
        if out.vines.len() >= 40 {
            break;
        }
        // A point just outside the bounds, on a random side.
        let t = rng.f32();
        let (x, z) = match rng.usize(0..4) {
            0 => (lo.x - 0.35, lo.z + t * (hi.z - lo.z)),
            1 => (hi.x + 0.35, lo.z + t * (hi.z - lo.z)),
            2 => (lo.x + t * (hi.x - lo.x), lo.z - 0.35),
            _ => (lo.x + t * (hi.x - lo.x), hi.z + 0.35),
        };
        let foot = Vec3::new(x, 0.0, z);
        if !clear_of_ways(foot) || !free(foot, 0.8) {
            continue;
        }
        sapling(
            &mut out,
            foot,
            0.45 + rng.f32() * 0.5,
            4 + rng.usize(0..4),
            &mut rng,
        );
    }
    out
}

/// The triangles a growth costs drawn in full: vines as three-sided tubes,
/// leaves as two-triangle cards.
pub fn triangles(g: &Growth) -> usize {
    g.vines
        .iter()
        .map(|v| (v.points.len() - 1) * 6)
        .sum::<usize>()
        + g.leaves.len() * 2
}

/// Most a building's ivy may cost, triangles (`triangles`).
pub const BUDGET: usize = 6000;
/// And its saplings and bushes (`reclaim`), at most.
pub const WILD_BUDGET: usize = 3000;

// --- Drawing --------------------------------------------------------------------------------

/// The ivy's materials, shared by every building: leaf cards cut from the
/// birch atlas's leafiest parts, darkened to ivy and swaying in the wind
/// (`cover::SwayMaterial`); the stems.
#[derive(Resource)]
pub struct IvyAssets {
    pub leaves: Handle<super::cover::SwayMaterial>,
    pub vines: Handle<StandardMaterial>,
}

pub fn setup_ivy(
    mut commands: Commands,
    existing: Option<Res<IvyAssets>>,
    server: Res<AssetServer>,
    mut sway: ResMut<Assets<super::cover::SwayMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if existing.is_some() {
        return;
    }
    let tex: Handle<Image> = server.load("textures/foliage/birch_leaves/albedo.ktx2");
    commands.insert_resource(IvyAssets {
        leaves: sway.add(super::cover::SwayMaterial {
            base: StandardMaterial {
                base_color: Color::srgb(0.42, 0.55, 0.3),
                base_color_texture: Some(tex),
                alpha_mode: super::trees::foliage_alpha(0.45),
                perceptual_roughness: 0.75,
                reflectance: 0.25,
                double_sided: true,
                cull_mode: None,
                diffuse_transmission: 0.25,
                ..default()
            },
            extension: super::cover::Sway::default(),
        }),
        vines: materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.19, 0.14),
            perceptual_roughness: 0.9,
            ..default()
        }),
    });
}

/// The leaves sway in the weather's wind, as the ground cover does
/// (written only when it changes: writing re-extracts every mesh).
pub fn blow_ivy(
    assets: Option<Res<IvyAssets>>,
    mut sway: ResMut<Assets<super::cover::SwayMaterial>>,
    conditions: Option<Res<super::weather::Conditions>>,
    mut last: Local<Option<f32>>,
) {
    let Some(assets) = assets else { return };
    let strength = conditions.map_or(0.4, |c| c.wind()) * 0.6;
    if *last == Some(strength) {
        return;
    }
    *last = Some(strength);
    if let Some(mut m) = sway.get_mut(&assets.leaves) {
        m.extension.wind = Vec4::new(0.8, 0.6, strength, 0.8);
        m.extension.pusher = Vec4::new(1e6, 1e6, 1e6, 0.0);
    }
}

/// The leafiest parts of the birch atlas, `[u0, v0, u1, v1]`.
const LEAF_UV: [[f32; 4]; 3] = [
    [0.36, 0.28, 0.62, 0.52],
    [0.52, 0.42, 0.78, 0.66],
    [0.3, 0.56, 0.56, 0.8],
];

/// The leaves as one card mesh: every `every`th leaf, scaled `scale`
/// (the far version is fewer, bigger cards).
pub fn leaf_mesh(g: &Growth, every: usize, scale: f32) -> Option<Mesh> {
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    let mut sway = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for (k, l) in g.leaves.iter().enumerate().filter(|(k, _)| k % every == 0) {
        let s = l.size * 2.3 * scale;
        let right = l.normal.cross(l.up).normalize_or_zero();
        let up = right.cross(l.normal).normalize_or_zero();
        let [u0, v0, u1, v1] = LEAF_UV[k % LEAF_UV.len()];
        let base = pos.len() as u32;
        let phase = (l.at.x * 3.1 + l.at.z * 1.7 + l.at.y * 0.9).fract();
        for (a, b, tu, tv) in [
            (-0.5, -0.5, u0, v1),
            (0.5, -0.5, u1, v1),
            (0.5, 0.5, u1, v0),
            (-0.5, 0.5, u0, v0),
        ] {
            pos.push((l.at + right * a * s + up * (b * s)).to_array());
            nrm.push(l.normal.to_array());
            uv.push([tu, tv]);
            let w = l.sway * if b > 0.0 { 1.0 } else { 0.35 };
            sway.push([w * w, phase]);
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    if idx.is_empty() {
        return None;
    }
    Some(
        Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, sway)
        .with_inserted_indices(bevy::mesh::Indices::U32(idx)),
    )
}

/// The stems: three-sided tubes lying on their faces.
pub fn vine_mesh(g: &Growth) -> Option<Mesh> {
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for v in &g.vines {
        let base = pos.len() as u32;
        for (i, (&p, &r)) in v.points.iter().zip(&v.radius).enumerate() {
            let next = v.points.get(i + 1).copied().unwrap_or(p);
            let prev = if i > 0 { v.points[i - 1] } else { p };
            let along = (next - prev).normalize_or(Vec3::Y);
            let side = along.cross(v.normal).normalize_or(Vec3::X);
            let c = p + v.normal * r;
            for k in 0..3 {
                let a = std::f32::consts::TAU * k as f32 / 3.0;
                let n = (v.normal * a.cos() + side * a.sin()).normalize();
                pos.push((c + n * r).to_array());
                nrm.push(n.to_array());
            }
        }
        for i in 0..v.points.len() as u32 - 1 {
            for k in 0..3u32 {
                let (a, b) = (base + i * 3 + k, base + i * 3 + (k + 1) % 3);
                let (c, d) = (a + 3, b + 3);
                idx.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }
    if idx.is_empty() {
        return None;
    }
    let n = pos.len();
    Some(
        Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n])
        .with_inserted_indices(bevy::mesh::Indices::U32(idx)),
    )
}

/// Where the ivy gives way: stems near, the full leaves to the middle
/// distance, a few big cards to where the building's stand-in takes over.
pub const VINES_TO: f32 = 40.0;
pub const LEAVES_TO: f32 = 60.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every placed building's ivy on the three valleys: the same seed
    /// grows the same, and none costs more than [`BUDGET`].
    #[test]
    fn ivy_is_deterministic_and_in_budget() {
        for name in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let map = crate::world::load_map_def(name);
            let (mut total, mut grown) = (0, 0);
            for p in &map.placements {
                let Some(geo) = crate::world::buildings::prefab(&p.prefab) else {
                    continue;
                };
                let occ = crate::world::buildings::occluders(&geo);
                let seed = seed(Vec2::from(p.at), &p.prefab);
                let ruin = extraction_core::map::prefab_base(&p.prefab) != p.prefab;
                let a = grow(&geo, &occ, seed, Vec3::NEG_Z);
                let b = grow(&geo, &occ, seed, Vec3::NEG_Z);
                assert_eq!(a, b, "{name} {}: grew differently", p.prefab);
                let wa = reclaim(&geo, &occ, seed, ruin);
                assert_eq!(
                    wa,
                    reclaim(&geo, &occ, seed, ruin),
                    "{name} {}: saplings",
                    p.prefab
                );
                let t = triangles(&a) + triangles(&wa);
                assert!(
                    t <= BUDGET + WILD_BUDGET,
                    "{name} {} at {:?}: {t} tris",
                    p.prefab,
                    p.at
                );
                total += t;
                grown += (t > 0) as usize;
            }
            println!("{name}: ivy on {grown} buildings, {total} tris");
            assert!(grown > 0, "{name}: no ivy anywhere");
        }
    }
}
