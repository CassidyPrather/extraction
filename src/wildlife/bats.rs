//! Bats, at dusk and at night: a few small groups hunting in loose,
//! erratic loops over the water, along the edges of the woods and round
//! the buildings, 3 to 15 m up. By day they roost in the buildings (not
//! shown).
//!
//! What they tell you: bats veer off a light (a torch's beam, a lamp:
//! [`LAMP_CLEAR`], [`TORCH_REACH`]) and scatter from a shot; and they go
//! quiet and leave when a machine comes within [`MACHINE`] of where they
//! hunt, coming back only a minute after it's gone ([`QUIET_FOR`]). Bats
//! over the creek and none tonight by the pumphouse: something's there.
//!
//! Like the fish, groups live only round the local player: the valley is
//! cut into [`CELL`]-metre cells, and a cell with water, a wood's edge or a
//! building in it gets a group or not by a roll of the seed and the cell
//! (the same cell always has its bats). Out of [`FORGET`] they're dropped.
//! A bat is never inside or under anything built, nor below [`MIN_UP`]
//! over the ground or the water.

use bevy::prelude::*;
use extraction_core::noise::Rng;

use super::ThreatKind;
use super::Threats;
use super::perches::Obstacles;
use crate::world::Zone;
use crate::world::weather::{Conditions, Hour};

pub const CELL: f32 = 40.0;
/// Groups are kept within this of you...
pub const NEAR: f32 = 90.0;
/// ...and dropped beyond this.
pub const FORGET: f32 = 130.0;
/// A cell with somewhere to hunt has a group this often.
pub const CHANCE: f32 = 0.5;
/// Bats in a group.
pub const GROUP: (usize, usize) = (3, 8);
/// How fast a bat flies (m/s), each its own.
pub const SPEED: (f32, f32) = (6.0, 9.0);
/// Never lower than this over the ground or the water (m)...
pub const MIN_UP: f32 = 2.0;
/// ...and hunting this high.
pub const BAND: (f32, f32) = (3.0, 15.0);
/// How sharply a bat turns (rad/s): a sudden swerve every so often.
pub const TURN: f32 = 9.0;
pub const SWERVE_EVERY: (f32, f32) = (0.25, 1.0);
/// They keep this far from a lamp...
pub const LAMP_CLEAR: f32 = 9.0;
/// ...and out of a torch's beam this long and this wide (rad, half angle).
pub const TORCH_REACH: f32 = 30.0;
pub const TORCH_CONE: f32 = 0.35;
/// A shot this close (`flock::FLUSH_NOISE` of its reach) scatters them...
pub const SCATTER_FOR: f32 = 3.0;
/// ...and a machine this close sends them off for good: gone after
/// [`LEAVE_FOR`], back [`QUIET_FOR`] after it's gone.
pub const MACHINE: f32 = 40.0;
pub const LEAVE_FOR: f32 = 5.0;
pub const QUIET_FOR: f32 = 60.0;
/// Wingbeats a second.
pub const BEAT: f32 = 10.0;
/// A bat's body: kept this far out of anything built.
const PAD: f32 = 0.3;

/// Where a group hunts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Haunt {
    Water,
    /// Along a wood's edge, outside it.
    Edge,
    /// Round a building, outside it.
    Building,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColonyState {
    Hunting,
    /// Off from a shot, for `left` seconds.
    Scattered {
        left: f32,
        from: Vec3,
    },
    /// Off from a machine; gone after `left`.
    Leaving {
        left: f32,
        from: Vec3,
    },
    /// Away; back once nothing's been near for `quiet` seconds.
    Gone {
        quiet: f32,
    },
}

#[derive(Clone, Debug)]
pub struct Bat {
    pub pos: Vec3,
    pub vel: Vec3,
    pub speed: f32,
    /// Where it's making for.
    to: Vec3,
    /// Seconds to its next swerve.
    swerve: f32,
    /// Through the wingbeat, 0..1 (two poses: `phase < 0.5` is up).
    pub phase: f32,
}

/// One group of bats and where it hunts.
#[derive(Component, Clone, Debug)]
pub struct Colony {
    pub cell: (i32, i32),
    pub haunt: Haunt,
    /// The middle of its beat, on the ground (or the water).
    pub centre: Vec3,
    /// Its beat: between `inner` and `reach` of the centre (round a
    /// building, outside its walls).
    pub inner: f32,
    pub reach: f32,
    pub bats: Vec<Bat>,
    pub state: ColonyState,
    rng: Rng,
}

/// Lights bats keep away from this tick: torches (where, which way) and
/// lamps.
#[derive(Clone, Copy, Default)]
pub struct Lights<'a> {
    pub torches: &'a [(Vec3, Vec3)],
    pub lamps: &'a [Vec3],
}

/// What a group's tick needs to know.
pub struct Ctx<'a> {
    pub zone: &'a Zone,
    pub obstacles: &'a Obstacles,
    pub lights: Lights<'a>,
    /// Loud noises this tick: where, and how far they scare.
    pub loud: &'a [(Vec3, f32)],
    /// Machines (drones, walkers, the airship).
    pub machines: &'a [Vec3],
}

/// Whether bats are out: at dusk and at night, not in heavy rain.
pub fn out(c: Option<&Conditions>) -> bool {
    c.is_some_and(|c| matches!(c.hour, Hour::Dusk | Hour::Night) && c.rain < 0.6)
}

/// The ground, or the water's surface over it.
pub fn surface(zone: &Zone, p: Vec2) -> f32 {
    let g = zone.ground(p.x, p.y);
    zone.hm.water_level([p.x, p.y]).map_or(g, |w| w.max(g))
}

/// Somewhere a bat may be: up off the ground and the water, and neither
/// inside nor under anything built.
pub fn clear(zone: &Zone, obstacles: &Obstacles, p: Vec3) -> bool {
    p.y >= surface(zone, p.xz()) + MIN_UP && !obstacles.inside(p, PAD) && !obstacles.roofed(p)
}

/// Clear all the way from `a` to `b` (nothing thin skipped over).
fn path_clear(zone: &Zone, obstacles: &Obstacles, a: Vec3, b: Vec3) -> bool {
    let n = (a.distance(b) / 0.15).ceil().max(1.0) as usize;
    (1..=n).all(|i| clear(zone, obstacles, a.lerp(b, i as f32 / n as f32)))
}

fn in_wood(zone: &Zone, p: Vec2) -> bool {
    zone.map
        .forests
        .iter()
        .any(|f| inside_polygon(&f.polygon, p))
}

fn inside_polygon(poly: &[[f32; 2]], p: Vec2) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (Vec2::from(poly[i]), Vec2::from(poly[(i + n - 1) % n]));
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// How a light pushes a bat at `p`: away from lamps near it and out of
/// the side of a torch's beam, stronger the deeper in it is. Zero out of
/// every light's reach.
pub fn shy(lights: Lights, p: Vec3) -> Vec3 {
    let mut push = Vec3::ZERO;
    for &l in lights.lamps {
        let d = p - l;
        let r = d.length();
        if r < LAMP_CLEAR * 1.5 && r > 1e-3 {
            push += d / r * (1.0 - r / (LAMP_CLEAR * 1.5));
        }
    }
    for &(at, dir) in lights.torches {
        let t = (p - at).dot(dir);
        if !(0.0..TORCH_REACH).contains(&t) {
            continue;
        }
        let off = p - (at + dir * t);
        let r = off.length();
        let wide = beam_width(t);
        if r < wide {
            let side = if r > 1e-3 {
                off / r
            } else {
                dir.cross(Vec3::Y).normalize_or(Vec3::X)
            };
            push += side * (1.0 - r / wide) * 1.5;
        }
    }
    push
}

/// Is `p` in a light (a lamp's reach, a torch's beam)?
pub fn lit(lights: Lights, p: Vec3) -> bool {
    lights.lamps.iter().any(|l| l.distance(p) < LAMP_CLEAR)
        || lights.torches.iter().any(|&(at, dir)| {
            let t = (p - at).dot(dir);
            (0.0..TORCH_REACH).contains(&t) && p.distance(at + dir * t) < beam_width(t)
        })
}

/// How far off a torch's beam's axis, `t` metres along it, a bat keeps:
/// the cone, and a margin.
fn beam_width(t: f32) -> f32 {
    t * TORCH_CONE.tan() * 1.6 + 1.5
}

/// Somewhere on a group's beat to make for: clear, out of the light, out
/// of the trees, in the band over the ground (over a roof's top if it's
/// over one).
#[allow(clippy::too_many_arguments)]
fn spot(
    zone: &Zone,
    obstacles: &Obstacles,
    lights: Lights,
    rng: &mut Rng,
    centre: Vec3,
    inner: f32,
    reach: f32,
) -> Option<Vec3> {
    for _ in 0..10 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = inner + (reach - inner) * rng.f32().sqrt();
        let xz = centre.xz() + Vec2::from_angle(a) * r;
        let s = surface(zone, xz);
        let mut y = s + rng.range(BAND.0, BAND.1 - 3.0);
        if let Some(top) = obstacles.top_near(Vec3::new(xz.x, y, xz.y), 1.5) {
            y = y.max(top + 2.5);
        }
        let p = Vec3::new(xz.x, y, xz.y);
        if y <= s + BAND.1
            && clear(zone, obstacles, p)
            && !obstacles.inside(p, 1.5)
            && !in_wood(zone, xz)
            && !lit(lights, p)
        {
            return Some(p);
        }
    }
    None
}

/// Where in a cell bats hunt, if anywhere: its deepest water, or the
/// nearest stretch of a wood's edge, or a building.
fn haunt_in(zone: &Zone, rng: &mut Rng, cell: (i32, i32)) -> Option<(Haunt, Vec3, f32, f32)> {
    let base = Vec2::new(cell.0 as f32, cell.1 as f32) * CELL;
    let mid = base + Vec2::splat(CELL * 0.5);
    let water = (0..16)
        .map(|_| base + Vec2::new(rng.f32(), rng.f32()) * CELL)
        .filter_map(|p| {
            let d = zone.hm.water_level([p.x, p.y])? - zone.ground(p.x, p.y);
            (d > 0.2).then_some((d, p))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, p)) = water {
        return Some((
            Haunt::Water,
            Vec3::new(p.x, surface(zone, p), p.y),
            0.0,
            13.0,
        ));
    }
    // The nearest point of a wood's edge to the cell's middle, if it's in
    // the cell: out from it, along it.
    let mut edge: Option<(f32, Vec2, Vec2)> = None;
    for f in &zone.map.forests {
        let n = f.polygon.len();
        for i in 0..n {
            let (a, b) = (Vec2::from(f.polygon[i]), Vec2::from(f.polygon[(i + 1) % n]));
            let ab = b - a;
            let t = ((mid - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let q = a + ab * t;
            let d = q.distance(mid);
            if d < CELL * 0.5 && edge.is_none_or(|e| d < e.0) {
                let side = ab.perp().normalize_or_zero();
                let out = if inside_polygon(&f.polygon, q + side) {
                    -side
                } else {
                    side
                };
                edge = Some((d, q, out));
            }
        }
    }
    if let Some((_, q, out)) = edge {
        let c = q + out * 9.0;
        return Some((Haunt::Edge, zone.on_ground(c.x, c.y), 0.0, 8.0));
    }
    let lo = base;
    let hi = base + Vec2::splat(CELL);
    let building = zone.map.placements.iter().find(|p| {
        let at = Vec2::from(p.at);
        at.cmpge(lo).all()
            && at.cmplt(hi).all()
            && !crate::world::underground::ids().any(|id| id == p.prefab)
    })?;
    let (mid, r, _) = super::perches::footprint(zone, building)?;
    if r > 40.0 {
        return None;
    }
    Some((
        Haunt::Building,
        zone.on_ground(mid.x, mid.y),
        r + 2.5,
        r + 10.0,
    ))
}

/// A cell's group of bats, if it has one: the same for a seed every time.
pub fn colony_at(
    zone: &Zone,
    obstacles: &Obstacles,
    seed: u64,
    cell: (i32, i32),
) -> Option<Colony> {
    let mut rng = Rng::new(
        seed ^ 0xBA75
            ^ (cell.0 as u64).wrapping_mul(0x9E37_79B9)
            ^ (cell.1 as u64).wrapping_mul(0xC2B2_AE3D),
    );
    if !rng.chance(CHANCE) {
        return None;
    }
    let (haunt, centre, inner, reach) = haunt_in(zone, &mut rng, cell)?;
    let mut c = Colony {
        cell,
        haunt,
        centre,
        inner,
        reach,
        bats: Vec::new(),
        state: ColonyState::Hunting,
        rng,
    };
    c.bats = c.place(zone, obstacles);
    (c.bats.len() >= GROUP.0).then_some(c)
}

impl Colony {
    /// Bats on the beat, flying.
    fn place(&mut self, zone: &Zone, obstacles: &Obstacles) -> Vec<Bat> {
        let n = GROUP.0 + self.rng.below(GROUP.1 - GROUP.0 + 1);
        let (c, i, r) = (self.centre, self.inner, self.reach);
        let none = Lights::default();
        let mut bats = Vec::new();
        for _ in 0..n * 3 {
            if bats.len() >= n {
                break;
            }
            let Some(pos) = spot(zone, obstacles, none, &mut self.rng, c, i, r) else {
                continue;
            };
            let to = spot(zone, obstacles, none, &mut self.rng, c, i, r).unwrap_or(pos);
            let speed = self.rng.range(SPEED.0, SPEED.1);
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            bats.push(Bat {
                pos,
                vel: Vec3::new(a.cos(), 0.0, a.sin()) * speed,
                speed,
                to,
                swerve: self.rng.range(SWERVE_EVERY.0, SWERVE_EVERY.1),
                phase: self.rng.f32(),
            });
        }
        bats
    }

    /// Out and about (not gone away).
    pub fn flying(&self) -> bool {
        !matches!(self.state, ColonyState::Gone { .. })
    }

    /// One tick: a machine near sends them off, a shot scatters them; and
    /// every bat's flight.
    pub fn tick(&mut self, ctx: &Ctx, dt: f32) {
        let near = |p: Vec3| p.xz().distance(self.centre.xz()) < MACHINE + self.reach;
        let machine = ctx.machines.iter().copied().find(|&m| near(m));
        match self.state {
            ColonyState::Gone { quiet } => {
                let quiet = if machine.is_some() {
                    QUIET_FOR
                } else {
                    quiet - dt
                };
                self.state = if quiet <= 0.0 {
                    self.bats = self.place(ctx.zone, ctx.obstacles);
                    ColonyState::Hunting
                } else {
                    ColonyState::Gone { quiet }
                };
                return;
            }
            ColonyState::Leaving { left, from } => {
                let left = left - dt;
                self.state = if left <= 0.0 {
                    ColonyState::Gone { quiet: QUIET_FOR }
                } else {
                    ColonyState::Leaving { left, from }
                };
            }
            _ if machine.is_some() => {
                let from = machine.unwrap_or_default();
                self.state = ColonyState::Leaving {
                    left: LEAVE_FOR,
                    from,
                };
                self.away(ctx, from, 60.0);
            }
            state => {
                let shot = ctx.loud.iter().copied().find(|&(at, r)| {
                    self.bats.iter().any(|b| b.pos.distance(at) < r) || at.distance(self.centre) < r
                });
                if let Some((at, _)) = shot {
                    self.state = ColonyState::Scattered {
                        left: SCATTER_FOR,
                        from: at,
                    };
                    self.away(ctx, at, 30.0);
                } else if let ColonyState::Scattered { left, from } = state {
                    let left = left - dt;
                    self.state = if left <= 0.0 {
                        ColonyState::Hunting
                    } else {
                        ColonyState::Scattered { left, from }
                    };
                }
            }
        }
        let hunting = self.state == ColonyState::Hunting;
        let (c, i, r) = (self.centre, self.inner, self.reach);
        for b in &mut self.bats {
            b.swerve -= dt;
            b.phase = (b.phase + BEAT * dt).fract();
            if hunting && (b.swerve <= 0.0 || b.pos.distance(b.to) < 2.0 || lit(ctx.lights, b.to)) {
                if let Some(p) = spot(ctx.zone, ctx.obstacles, ctx.lights, &mut self.rng, c, i, r) {
                    b.to = p;
                }
                b.swerve = self.rng.range(SWERVE_EVERY.0, SWERVE_EVERY.1);
                b.speed = self.rng.range(SPEED.0, SPEED.1);
            }
            let speed = if hunting { b.speed } else { SPEED.1 };
            fly(b, ctx, speed, dt, &mut self.rng);
        }
    }

    /// Every bat off away from `from`, `far` metres, and up.
    fn away(&mut self, ctx: &Ctx, from: Vec3, far: f32) {
        for b in &mut self.bats {
            let off = (b.pos - from).xz();
            let a = off.y.atan2(off.x) + self.rng.range(-0.5, 0.5);
            let xz = b.pos.xz() + Vec2::from_angle(a) * far;
            let y = surface(ctx.zone, xz) + BAND.1 - 2.0;
            b.to = Vec3::new(xz.x, y.max(b.pos.y), xz.y);
        }
    }
}

/// A bat's flight for a tick: for where it's making for, off the lights,
/// up off the ground and over what's built; never into anything.
fn fly(b: &mut Bat, ctx: &Ctx, speed: f32, dt: f32, rng: &mut Rng) {
    let (zone, obstacles) = (ctx.zone, ctx.obstacles);
    let dir = b.vel.normalize_or(Vec3::X);
    let mut want = (b.to - b.pos).normalize_or(dir) + shy(ctx.lights, b.pos) * 4.0;
    let s = surface(zone, b.pos.xz());
    if b.pos.y < s + BAND.0 {
        want.y += (s + BAND.0 - b.pos.y) * 0.8;
    } else if b.pos.y > s + BAND.1 {
        want.y -= (b.pos.y - s - BAND.1) * 0.5;
    }
    // Something built ahead: up over it.
    let ahead = b.pos + dir * speed * 0.5;
    if obstacles.inside(ahead, 1.0) || obstacles.roofed(ahead) {
        want.y += 1.5;
    }
    let want = want.normalize_or(dir);
    let angle = dir.angle_between(want);
    let new = if angle > 1e-4 {
        let k = (TURN * dt / angle).min(1.0);
        Quat::IDENTITY.slerp(Quat::from_rotation_arc(dir, want), k) * dir
    } else {
        dir
    };
    b.vel = new.normalize_or(dir) * speed;
    let step = b.vel * dt;
    if path_clear(zone, obstacles, b.pos, b.pos + step) {
        b.pos += step;
    } else if path_clear(zone, obstacles, b.pos, b.pos + Vec3::Y * speed * dt) {
        // Straight up, clear of it.
        b.pos += Vec3::Y * speed * dt;
        b.vel = Vec3::new(b.vel.x, b.vel.y.abs(), b.vel.z).normalize_or(Vec3::Y) * speed;
    } else {
        // Back the way it came, off somewhere else.
        b.vel = -b.vel;
        let a = rng.range(-0.6, 0.6);
        b.to = b.pos + Quat::from_rotation_y(a) * b.vel.normalize_or(Vec3::X) * 10.0;
        b.swerve = rng.range(SWERVE_EVERY.0, SWERVE_EVERY.1);
    }
}

/// The lamps in the buildings (`buildings::Geo::lights`), in the world.
pub fn lamps(zone: &Zone) -> Vec<Vec3> {
    use crate::world::buildings::{placement_transform, prefab};
    zone.map
        .placements
        .iter()
        .filter(|p| !crate::world::underground::ids().any(|id| id == p.prefab))
        .filter_map(|p| Some((prefab(&p.prefab)?, placement_transform(zone, p))))
        .flat_map(|(geo, xf)| {
            geo.lights
                .iter()
                .map(|l| xf.transform_point(*l))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Groups in and out round the local player, at dusk and at night, and
/// their ticks.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bats(
    mut commands: Commands,
    mut colonies: Query<(Entity, &mut Colony)>,
    me: Query<&GlobalTransform, With<crate::rig::LocalPlayer>>,
    threats: Res<Threats>,
    zone: Res<Zone>,
    obstacles: Option<Res<Obstacles>>,
    opts: Res<crate::LaunchOptions>,
    conditions: Option<Res<Conditions>>,
    torches: Query<(&crate::gadgets::Device, &GlobalTransform), With<crate::gadgets::Powered>>,
    point_lights: Query<(&GlobalTransform, &PointLight)>,
    mut noises: MessageReader<crate::robots::Noise>,
    mut every: Local<f32>,
    mut built_lamps: Local<Option<Vec<Vec3>>>,
    mut nothing: Local<std::collections::HashSet<(i32, i32)>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let loud: Vec<(Vec3, f32)> = noises
        .read()
        .filter(|n| n.radius >= super::flock::LOUD)
        .map(|n| (n.at, n.radius * super::flock::FLUSH_NOISE))
        .collect();
    let (Some(obstacles), Ok(me)) = (obstacles, me.single()) else {
        return;
    };
    let at = me.translation();
    let off = std::env::var("EXTRACTION_WILDLIFE").is_ok_and(|v| v == "off");
    if off || !out(conditions.as_deref()) {
        for (e, _) in &colonies {
            commands.entity(e).despawn();
        }
        return;
    }
    *every -= dt;
    if *every <= 0.0 {
        *every = 1.0;
        let have: Vec<(i32, i32)> = colonies.iter().map(|(_, c)| c.cell).collect();
        for (e, c) in &colonies {
            let mid = (Vec2::new(c.cell.0 as f32, c.cell.1 as f32) + 0.5) * CELL;
            if mid.distance(at.xz()) > FORGET {
                commands.entity(e).despawn();
            }
        }
        let r = (NEAR / CELL).ceil() as i32;
        let (cx, cz) = ((at.x / CELL).floor() as i32, (at.z / CELL).floor() as i32);
        for i in -r..=r {
            for j in -r..=r {
                let cell = (cx + i, cz + j);
                let mid = (Vec2::new(cell.0 as f32, cell.1 as f32) + 0.5) * CELL;
                if have.contains(&cell) || nothing.contains(&cell) || mid.distance(at.xz()) > NEAR {
                    continue;
                }
                match colony_at(&zone, &obstacles, opts.seed, cell) {
                    // At the origin: its bats (`bat_look`) are its
                    // children, placed in world space.
                    Some(c) => {
                        commands.spawn((
                            Name::new("Bats"),
                            Transform::IDENTITY,
                            Visibility::default(),
                            c,
                        ));
                    }
                    None => {
                        nothing.insert(cell);
                    }
                }
            }
        }
    }
    let mut near_lamps = built_lamps.get_or_insert_with(|| lamps(&zone)).clone();
    // Any other lamp lit near you (the camp's, a lantern): the client's.
    near_lamps.extend(
        point_lights
            .iter()
            .filter(|(_, l)| l.intensity > 1000.0)
            .map(|(g, _)| g.translation())
            .filter(|p| p.distance(at) < FORGET),
    );
    let beams: Vec<(Vec3, Vec3)> = torches
        .iter()
        .filter(|(d, _)| d.kind == crate::gadgets::DeviceKind::Light)
        .map(|(_, g)| (g.translation(), g.forward().as_vec3()))
        .collect();
    let machines: Vec<Vec3> = threats
        .0
        .iter()
        .filter(|t| !matches!(t.kind, ThreatKind::Player { .. }))
        .map(|t| t.at)
        .collect();
    let ctx = Ctx {
        zone: &zone,
        obstacles: &obstacles,
        lights: Lights {
            torches: &beams,
            lamps: &near_lamps,
        },
        loud: &loud,
        machines: &machines,
    };
    for (_, mut c) in &mut colonies {
        c.tick(&ctx, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;

    fn valley(map: &str) -> (Zone, Obstacles) {
        let zone = Zone::load(map);
        let (o, _) = super::super::perches::survey(&zone);
        (zone, o)
    }

    /// Every group in the valley, for a seed.
    fn all(zone: &Zone, obstacles: &Obstacles, seed: u64) -> Vec<Colony> {
        let half = (zone.map.size * 0.5 / CELL).ceil() as i32;
        let mut out = Vec::new();
        for i in -half..half {
            for j in -half..half {
                out.extend(colony_at(zone, obstacles, seed, (i, j)));
            }
        }
        out
    }

    fn assert_clear(map: &str, zone: &Zone, obstacles: &Obstacles, c: &Colony) {
        if !c.flying() {
            return;
        }
        for b in &c.bats {
            let s = surface(zone, b.pos.xz());
            assert!(
                b.pos.y >= s + MIN_UP - 1e-3,
                "{map}: a bat {:.2} m up at {:.1}",
                b.pos.y - s,
                b.pos
            );
            assert!(
                !obstacles.inside(b.pos, 0.0) && !obstacles.roofed(b.pos),
                "{map}: a bat in something built at {:.1} ({:?})",
                b.pos,
                c.haunt
            );
            assert!(b.vel.length() <= SPEED.1 + 1e-3);
        }
    }

    /// Out at dusk and at night (not in a downpour), never by day.
    #[test]
    fn bats_only_at_dusk_and_night() {
        for (hour, want) in [
            (Hour::Dawn, false),
            (Hour::Day, false),
            (Hour::Dusk, true),
            (Hour::Night, true),
        ] {
            let mut c = Conditions::compose(crate::world::weather::WeatherKind::Clearing, hour, 0);
            c.rain = 0.0;
            assert_eq!(out(Some(&c)), want, "{hour:?}");
            c.rain = 0.9;
            assert!(!out(Some(&c)), "{hour:?} in a downpour");
        }
        assert!(!out(None));
    }

    /// In the whole game: by the creek in Marrow Creek there are bats at
    /// dusk and at night, and none by day.
    #[test]
    fn bats_in_the_game_only_after_dark() {
        use crate::rig::LocalPlayer;
        let (zone, obstacles) = valley("marrow_creek");
        let spot = all(&zone, &obstacles, 1)
            .into_iter()
            .find(|c| c.haunt == Haunt::Water)
            .expect("bats on the creek")
            .centre;
        for (hour, want) in [("day", false), ("dusk", true), ("night", true)] {
            let mut app = crate::headless_app(
                crate::LaunchOptions {
                    robots: false,
                    seed: 1,
                    hour: Some(hour.into()),
                    weather: Some("clearing".into()),
                    ..Default::default()
                },
                false,
            );
            app.finish();
            app.cleanup();
            for _ in 0..3 {
                app.update();
            }
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            for _ in 0..200 {
                app.world_mut()
                    .get_mut::<Transform>(player)
                    .unwrap()
                    .translation = spot + Vec3::new(6.0, 0.05, 0.0);
                app.update();
            }
            let bats: usize = app
                .world_mut()
                .query::<&Colony>()
                .iter(app.world())
                .map(|c| c.bats.len())
                .sum();
            assert_eq!(bats > 0, want, "{hour}: {bats} bats");
        }
    }

    /// How many of a valley's cells would have bats over water, if they had
    /// bats.
    fn wet_cells(zone: &Zone) -> usize {
        let half = (zone.map.size * 0.5 / CELL).ceil() as i32;
        let mut n = 0;
        for i in -half..half {
            for j in -half..half {
                let mut rng = Rng::new(0xBA75 ^ (i as u64) << 20 ^ j as u64);
                n += haunt_in(zone, &mut rng, (i, j)).is_some_and(|h| h.0 == Haunt::Water) as usize;
            }
        }
        n
    }

    /// Every valley has bats to hunt somewhere, over its water where it has
    /// much; four minutes of someone walking about with a torch on them and
    /// shooting now and then, and a machine coming by, never puts a bat inside or under anything built
    /// or below 2 m over the ground or the water; they scatter from the
    /// shots and leave for the machine, and come back.
    #[test]
    fn bats_never_go_inside_anything_or_underground() {
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            let (zone, obstacles) = valley(map);
            let lamps = lamps(&zone);
            let every = all(&zone, &obstacles, 3);
            assert!(every.len() >= 4, "{map}: {} groups", every.len());
            // Over the water wherever there's much of it: not over Rime
            // Fell's one small tarn, two cells, which no group may draw.
            let wet = wet_cells(&zone);
            let haunts: &[Haunt] = if wet >= 8 {
                &[Haunt::Water, Haunt::Building]
            } else {
                assert_eq!(map, "rime_fell", "{map}: {wet} cells over water");
                &[Haunt::Building]
            };
            for &haunt in haunts {
                assert!(
                    every.iter().any(|c| c.haunt == haunt),
                    "{map}: no bats hunting {haunt:?}"
                );
            }
            let step = (every.len() / 10).max(1);
            let mut groups: Vec<Colony> = every.into_iter().step_by(step).take(10).collect();
            let mut rng = Rng::new(9);
            let (mut scattered, mut left, mut back) = (0, 0, 0);
            let steps = (240.0 / DT) as usize;
            for k in 0..steps {
                let t = k as f32 * DT;
                // Round the groups one after another, 12 m off, torch on
                // their beat.
                let g = &groups[(k / 600) % groups.len()];
                let a = t * 0.15;
                let me = zone.on_ground(g.centre.x + a.cos() * 12.0, g.centre.z + a.sin() * 12.0)
                    + Vec3::Y * 1.5;
                let dir = (g.centre + Vec3::Y * 7.0 - me).normalize();
                let loud = if rng.chance(DT / 25.0) {
                    vec![(me, 220.0 * super::super::flock::FLUSH_NOISE)]
                } else {
                    vec![]
                };
                // A walker past the first group for half a minute.
                let machines = if (60.0..90.0).contains(&t) {
                    vec![groups[0].centre + Vec3::X * 20.0]
                } else {
                    vec![]
                };
                let torches = [(me, dir)];
                let ctx = Ctx {
                    zone: &zone,
                    obstacles: &obstacles,
                    lights: Lights {
                        torches: &torches,
                        lamps: &lamps,
                    },
                    loud: &loud,
                    machines: &machines,
                };
                for c in &mut groups {
                    let before = c.state;
                    c.tick(&ctx, DT);
                    match (before, c.state) {
                        (ColonyState::Hunting, ColonyState::Scattered { .. }) => scattered += 1,
                        (ColonyState::Leaving { .. }, ColonyState::Gone { .. }) => left += 1,
                        (ColonyState::Gone { .. }, ColonyState::Hunting) => back += 1,
                        _ => {}
                    }
                    assert_clear(map, &zone, &obstacles, c);
                }
            }
            assert!(scattered >= 3, "{map}: scattered {scattered} times");
            assert!(left >= 1 && back >= 1, "{map}: left {left}, back {back}");
        }
    }

    /// How far a bat is from a torch's beam (the ray, out to its reach).
    fn off_beam(p: Vec3, at: Vec3, dir: Vec3) -> f32 {
        let t = (p - at).dot(dir).clamp(0.0, TORCH_REACH);
        p.distance(at + dir * t)
    }

    /// Point a torch across a group's beat and the bats keep further from
    /// its beam than they fly from the same line in the dark.
    #[test]
    fn bats_keep_out_of_a_torch_beam() {
        let (zone, obstacles) = valley("marrow_creek");
        for c in all(&zone, &obstacles, 1)
            .into_iter()
            .filter(|c| c.haunt == Haunt::Water)
            .take(3)
        {
            let me = zone.on_ground(c.centre.x + 14.0, c.centre.z) + Vec3::Y * 1.5;
            let dir = (c.centre + Vec3::Y * 7.0 - me).normalize();
            let mean = |torch: bool| {
                let mut c = c.clone();
                let torches = [(me, dir)];
                let ctx = Ctx {
                    zone: &zone,
                    obstacles: &obstacles,
                    lights: Lights {
                        torches: if torch { &torches } else { &[] },
                        lamps: &[],
                    },
                    loud: &[],
                    machines: &[],
                };
                let (mut sum, mut n) = (0.0, 0.0);
                for k in 0..(60.0 / DT) as usize {
                    c.tick(&ctx, DT);
                    if k > 60 {
                        for b in &c.bats {
                            sum += off_beam(b.pos, me, dir);
                            n += 1.0;
                        }
                    }
                }
                sum / n
            };
            let (dark, lit) = (mean(false), mean(true));
            assert!(
                lit > dark + 1.0,
                "{:.0}: {lit:.1} m from the beam with it on, {dark:.1} m in the dark",
                c.centre
            );
        }
    }

    /// The same seed, the same bats and the same flight, bat for bat.
    #[test]
    fn bats_are_deterministic() {
        let (zone, obstacles) = valley("marrow_creek");
        let run = || {
            let mut groups = all(&zone, &obstacles, 5);
            let me = groups[0].centre + Vec3::new(10.0, 1.5, 0.0);
            for k in 0..(30.0 / DT) as usize {
                let loud = if k == 300 { vec![(me, 130.0)] } else { vec![] };
                let torches = [(me, Vec3::NEG_X)];
                let ctx = Ctx {
                    zone: &zone,
                    obstacles: &obstacles,
                    lights: Lights {
                        torches: &torches,
                        lamps: &[],
                    },
                    loud: &loud,
                    machines: &[],
                };
                for c in &mut groups {
                    c.tick(&ctx, DT);
                }
            }
            groups
                .iter()
                .flat_map(|c| c.bats.iter().map(|b| b.pos))
                .collect::<Vec<_>>()
        };
        let a = run();
        assert!(!a.is_empty());
        assert_eq!(a, run());
    }
}
