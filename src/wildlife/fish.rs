//! Fish in the creeks: a few trout holding in the deeper water, heads into
//! the current, a flick of the tail now and then. Come to the bank (or
//! wade in) and they're gone, darting off downstream into deeper water;
//! they drift back a while later. Fish there and nobody's been by.
//!
//! Shoals live only round the local player: the water is cut into
//! [`CELL`]-metre cells, and a cell with enough depth near you gets a shoal
//! or not by a roll of the seed and the cell (so the same pool always has
//! its fish). Out of [`FORGET`] they're dropped. Fish never leave water
//! [`MIN_DEPTH`] deep, and keep between the bed and the surface.

use bevy::prelude::*;
use extraction_core::noise::Rng;

use super::ThreatKind;
use super::Threats;
use crate::world::Zone;

pub const CELL: f32 = 14.0;
/// Shoals are kept within this of you...
pub const NEAR: f32 = 45.0;
/// ...and dropped beyond this.
pub const FORGET: f32 = 70.0;
/// Fish keep to water at least this deep (m).
pub const MIN_DEPTH: f32 = 0.3;
/// A cell with deep enough water has a shoal this often.
pub const CHANCE: f32 = 0.55;
/// They dart off from someone on the bank this close...
pub const SCARE: f32 = 6.0;
/// ...or wading this close, or moving fast this close.
pub const SCARE_WADING: f32 = 11.0;
/// They dart this fast, for this long, and come back after this long.
pub const DART: f32 = 3.5;
pub const DART_FOR: f32 = 0.9;
pub const RETURN_AFTER: f32 = 12.0;

#[derive(Clone, Debug)]
pub struct Fish {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Heading, radians about Y (atan2(z, x)).
    pub yaw: f32,
    /// Where it holds.
    pub home: Vec3,
    /// Seconds left darting (> 0), or hiding (< 0, counting up to 0).
    pub dart: f32,
    /// For the tail's beat.
    pub phase: f32,
    pub size: f32,
}

#[derive(Component, Clone, Debug)]
pub struct Shoal {
    pub cell: (i32, i32),
    pub fish: Vec<Fish>,
    /// Into the current: the way they face.
    pub upstream: f32,
    rng: Rng,
}

/// How deep the water is at `p` (0 on dry land).
pub fn depth(zone: &Zone, p: Vec2) -> f32 {
    zone.hm
        .water_level([p.x, p.y])
        .map_or(0.0, |w| w - zone.ground(p.x, p.y))
}

/// Where in a cell a shoal holds, if it has one: the deepest of a few
/// points in it, if deep enough and the roll says so.
pub fn shoal_at(zone: &Zone, seed: u64, cell: (i32, i32)) -> Option<Vec2> {
    let mut rng = Rng::new(
        seed ^ 0xF15A
            ^ (cell.0 as u64).wrapping_mul(0x9E37_79B9)
            ^ (cell.1 as u64).wrapping_mul(0xC2B2_AE3D),
    );
    if !rng.chance(CHANCE) {
        return None;
    }
    let base = Vec2::new(cell.0 as f32, cell.1 as f32) * CELL;
    (0..12)
        .map(|_| base + Vec2::new(rng.f32(), rng.f32()) * CELL)
        .map(|p| (depth(zone, p), p))
        .filter(|(d, _)| *d >= MIN_DEPTH + 0.15)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
}

/// The way water runs past `p`: along the nearest stretch of the nearest
/// stream's line, toward its first point.
fn upstream(zone: &Zone, p: Vec2) -> f32 {
    let mut best = (f32::MAX, 0.0f32);
    for w in &zone.map.water {
        for s in w.points.windows(2) {
            let (a, b) = (Vec2::from(s[0]), Vec2::from(s[1]));
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let d = p.distance(a + ab * t);
            if d < best.0 {
                let up = -ab;
                best = (d, up.y.atan2(up.x));
            }
        }
    }
    best.1
}

fn new_shoal(zone: &Zone, seed: u64, cell: (i32, i32), at: Vec2) -> Shoal {
    let mut rng = Rng::new(seed ^ 0x5A0A ^ (cell.0 as u64) << 20 ^ cell.1 as u64);
    let up = upstream(zone, at);
    let n = 2 + rng.below(4);
    let fish = (0..n)
        .filter_map(|_| {
            let p = at + Vec2::new(rng.range(-1.5, 1.5), rng.range(-1.5, 1.5));
            let d = depth(zone, p);
            (d >= MIN_DEPTH).then(|| {
                let home = hold(zone, p, rng.range(0.35, 0.65));
                Fish {
                    pos: home,
                    vel: Vec3::ZERO,
                    yaw: up + rng.range(-0.3, 0.3),
                    home,
                    dart: 0.0,
                    phase: rng.f32(),
                    size: rng.range(0.7, 1.2),
                }
            })
        })
        .collect();
    Shoal {
        cell,
        fish,
        upstream: up,
        rng,
    }
}

/// A point at `share` of the way down from the surface to the bed, kept
/// clear of both.
pub fn hold(zone: &Zone, p: Vec2, share: f32) -> Vec3 {
    let bed = zone.ground(p.x, p.y);
    let top = zone.hm.water_level([p.x, p.y]).unwrap_or(bed);
    let y = (top - (top - bed) * share).clamp(bed + 0.08, (top - 0.08).max(bed + 0.08));
    Vec3::new(p.x, y, p.y)
}

impl Shoal {
    /// One tick: scared by `threats` (and whether each is wading), or loud
    /// noises; holding, darting, coming back.
    pub fn tick(&mut self, zone: &Zone, threats: &[(Vec3, f32, bool)], loud: &[Vec3], dt: f32) {
        let up = self.upstream;
        for f in &mut self.fish {
            let scared = threats.iter().any(|&(p, speed, wading)| {
                let r = if wading || speed > 3.0 {
                    SCARE_WADING
                } else {
                    SCARE
                };
                p.xz().distance(f.pos.xz()) < r
            }) || loud.iter().any(|p| p.distance(f.pos) < 40.0);
            if scared && f.dart <= 0.0 {
                // Off, away from the fright, downstream-ish.
                let from = threats
                    .iter()
                    .map(|t| t.0)
                    .chain(loud.iter().copied())
                    .min_by(|a, b| a.distance(f.pos).total_cmp(&b.distance(f.pos)))
                    .unwrap_or(f.pos);
                let away = (f.pos - from)
                    .xz()
                    .normalize_or(Vec2::from_angle(up + std::f32::consts::PI));
                let a = away.y.atan2(away.x) + self.rng.range(-0.6, 0.6);
                f.vel = Vec3::new(a.cos(), 0.0, a.sin()) * DART;
                f.dart = DART_FOR;
            }
            if f.dart > 0.0 {
                f.dart -= dt;
                if f.dart <= 0.0 {
                    f.dart = -RETURN_AFTER;
                    // Wherever it stopped is where it hides, for now.
                }
            } else if f.dart < 0.0 {
                f.dart = (f.dart + dt).min(0.0);
                f.vel *= (1.0 - dt * 3.0).max(0.0);
            } else {
                // Holding: drift back home, slowly, nose upstream.
                let to = f.home - f.pos;
                f.vel = f.vel.lerp(to.clamp_length_max(0.3), (dt * 1.5).min(1.0));
            }
            let next = f.pos + f.vel * dt;
            if depth(zone, next.xz()) >= MIN_DEPTH {
                f.pos = hold(zone, next.xz(), 0.5).with_y(f.pos.y);
                // Keep between bed and surface.
                let lo = zone.ground(f.pos.x, f.pos.z) + 0.08;
                let hi = zone.hm.water_level([f.pos.x, f.pos.z]).unwrap_or(lo) - 0.08;
                f.pos.y = f.pos.y.clamp(lo, hi.max(lo));
            } else {
                f.vel = -f.vel * 0.5;
            }
            let want = if f.vel.xz().length() > 0.5 {
                f.vel.z.atan2(f.vel.x)
            } else {
                up
            };
            let d = crate::robots::anim::wrap_angle(want - f.yaw);
            f.yaw += d.clamp(-6.0 * dt, 6.0 * dt);
            f.phase = (f.phase + dt * (1.0 + f.vel.length() * 3.0)).fract();
        }
    }
}

/// Shoals in and out round the local player, and their ticks.
#[allow(clippy::too_many_arguments)]
pub(crate) fn shoals(
    mut commands: Commands,
    mut shoals: Query<(Entity, &mut Shoal)>,
    me: Query<&GlobalTransform, With<crate::rig::LocalPlayer>>,
    players: Query<(&GlobalTransform, &crate::player::Body), With<crate::player::Player>>,
    threats: Res<Threats>,
    zone: Res<Zone>,
    opts: Res<crate::LaunchOptions>,
    mut noises: MessageReader<crate::robots::Noise>,
    mut every: Local<f32>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let loud: Vec<Vec3> = noises
        .read()
        .filter(|n| n.radius >= super::flock::LOUD)
        .map(|n| n.at)
        .collect();
    let Ok(me) = me.single() else {
        return;
    };
    let at = me.translation();
    *every -= dt;
    if *every <= 0.0 {
        *every = 1.0;
        let off = std::env::var("EXTRACTION_WILDLIFE").is_ok_and(|v| v == "off");
        let have: Vec<(i32, i32)> = shoals.iter().map(|(_, s)| s.cell).collect();
        for (e, s) in &shoals {
            let c = (Vec2::new(s.cell.0 as f32, s.cell.1 as f32) + 0.5) * CELL;
            if c.distance(at.xz()) > FORGET {
                commands.entity(e).despawn();
            }
        }
        let r = (NEAR / CELL).ceil() as i32;
        let (cx, cz) = ((at.x / CELL).floor() as i32, (at.z / CELL).floor() as i32);
        for i in -r..=r {
            for j in -r..=r {
                let cell = (cx + i, cz + j);
                let c = (Vec2::new(cell.0 as f32, cell.1 as f32) + 0.5) * CELL;
                if off || have.contains(&cell) || c.distance(at.xz()) > NEAR {
                    continue;
                }
                if let Some(p) = shoal_at(&zone, opts.seed, cell) {
                    let s = new_shoal(&zone, opts.seed, cell, p);
                    if !s.fish.is_empty() {
                        // At the origin: its fish (`small_look`) are its
                        // children, placed in world space.
                        commands.spawn((Name::new("Shoal"), Transform::IDENTITY, s));
                    }
                }
            }
        }
    }
    // People (and whether they're wading), and machines' feet in the water.
    let mut t: Vec<(Vec3, f32, bool)> = players
        .iter()
        .map(|(g, b)| {
            (
                g.translation(),
                Vec2::new(b.velocity.x, b.velocity.z).length(),
                b.wading > 0.1,
            )
        })
        .collect();
    t.extend(
        threats
            .0
            .iter()
            .filter(|t| t.kind == ThreatKind::Walker)
            .map(|t| (t.at, t.speed, true)),
    );
    for (_, mut s) in &mut shoals {
        s.tick(&zone, &t, &loud, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every valley with water has fish somewhere in it; they stay in deep
    /// enough water between the bed and the surface while you splash about
    /// and shoot, and dart off when you come to the bank.
    #[test]
    fn fish_keep_to_the_water_and_flee_the_bank() {
        for map in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let zone = Zone::load(map);
            let half = (zone.map.size * 0.5 / CELL) as i32;
            let mut shoals = Vec::new();
            for i in -half..half {
                for j in -half..half {
                    if let Some(p) = shoal_at(&zone, 1, (i, j)) {
                        shoals.push(new_shoal(&zone, 1, (i, j), p));
                    }
                }
            }
            shoals.retain(|s| !s.fish.is_empty());
            assert!(shoals.len() >= 4, "{map}: {} shoals", shoals.len());
            let mut darted = 0;
            for s in shoals.iter_mut().take(12) {
                let home = s.fish[0].home;
                for step in 0..600 {
                    // Walk up to the bank and stand there, wade in, shoot.
                    let me = home + Vec3::new(8.0 - (step as f32 * 0.02).min(5.0), 0.0, 0.0);
                    let loud = if step == 400 { vec![me] } else { vec![] };
                    let wading = step > 300;
                    let before: Vec<f32> = s.fish.iter().map(|f| f.dart).collect();
                    s.tick(&zone, &[(me, 1.0, wading)], &loud, 1.0 / 30.0);
                    for (f, b) in s.fish.iter().zip(before) {
                        if f.dart > 0.0 && b <= 0.0 {
                            darted += 1;
                        }
                        let d = depth(&zone, f.pos.xz());
                        assert!(d >= MIN_DEPTH - 1e-3, "{map}: a fish in {d:.2} m of water");
                        let bed = zone.ground(f.pos.x, f.pos.z);
                        let top = zone.hm.water_level([f.pos.x, f.pos.z]).unwrap();
                        assert!(
                            f.pos.y > bed && f.pos.y < top,
                            "{map}: a fish out of the water"
                        );
                    }
                }
            }
            assert!(darted >= 6, "{map}: fish darted {darted} times");
        }
    }
}
