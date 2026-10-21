//! Window panes: every one can break.
//!
//! A building's windows are `window_sash` models (`art/props/openings.py`),
//! four panes to a sash: two casements, a vent pane and a fixed pane over
//! them. Each pane is a logic entity here ([`Pane`], a child of its
//! building) with no collider: glass costs a round nothing to go through
//! (`firearm_terminal`), and robots see through it. Instead, whatever
//! crosses a pane breaks it: a round or a fragment (swept along its path
//! before it flies, clipped where it would stop), something thrown or
//! swung or a fist (anything moving past [`BREAK_SPEED`]), a robot
//! walking into it, a grenade's blast nearby.
//!
//! A pane breaks round where it was struck (`fracture::shatter`): the
//! falling pieces are thrown as [`debris`](super::debris), the teeth stay
//! in the frame (drawn by [`super::visuals`]), and the crash is heard.

use avian3d::prelude::*;
use bevy::math::Affine3A;
use bevy::prelude::*;
use extraction_core::fracture;
use extraction_core::noise::Rng;
use std::collections::HashMap;

use super::debris::{self, DebrisPool, Kind, Piece};
use super::{Broke, Broken, Stuff, key};
use crate::world::Layer;
use crate::world::buildings::{Building, Geo};

/// The window model whose panes these are.
pub const SASH: &str = "window_sash";
/// Glass thickness, metres.
pub const THICK: f32 = 0.004;
/// Moving at least this fast (m/s), a thing breaks a pane it crosses.
pub const BREAK_SPEED: f32 = 2.5;
/// A grenade breaks every pane within this many metres that it can see.
pub const BLAST_REACH: f32 = 7.0;

/// One pane of glass: its half extents in its plane (x across, y up, z
/// its normal), where it is in the world, and how it broke.
#[derive(Component, Clone, Debug)]
pub struct Pane {
    pub half: Vec2,
    pub building: Entity,
    /// Its world pose (buildings don't move), and that pose's inverse.
    pub world: Transform,
    pub inv: Affine3A,
    pub broken: Option<Break>,
}

/// How a pane broke: its fracture's seed and where it was struck (in its
/// own plane), all the visuals need to redraw the teeth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Break {
    pub seed: u64,
    pub impact: Vec2,
    /// Broken by the years, not by anyone: the sash's own model
    /// (`window_sash_broken`) has the shards, so the pane draws none, and
    /// the Surge doesn't mend it.
    pub weathered: bool,
    /// Which side of the pane its glass fell (+1 along its local z, the
    /// room side of an outside wall; -1 out).
    pub fell: f32,
}

impl Pane {
    pub fn centre(&self) -> Vec3 {
        self.world.translation
    }

    pub fn area(&self) -> f32 {
        4.0 * self.half.x * self.half.y
    }

    /// Where a segment crosses the pane (world), padded by `pad` round its
    /// edges, if it does.
    pub fn crossing(&self, a: Vec3, b: Vec3, pad: f32) -> Option<Vec3> {
        let (la, lb) = (self.inv.transform_point3(a), self.inv.transform_point3(b));
        let t = if (la.z > 0.0) != (lb.z > 0.0) {
            la.z / (la.z - lb.z)
        } else if la.z.abs() < pad {
            0.0
        } else if lb.z.abs() < pad {
            1.0
        } else {
            return None;
        };
        let p = la.lerp(lb, t);
        (p.x.abs() <= self.half.x + pad && p.y.abs() <= self.half.y + pad)
            .then(|| self.world.transform_point(p.with_z(0.0)))
    }

    /// The point on the pane nearest `p` if it's within `r` of it.
    pub fn touching(&self, p: Vec3, r: f32) -> Option<Vec3> {
        let l = self.inv.transform_point3(p);
        let q = Vec3::new(
            l.x.clamp(-self.half.x, self.half.x),
            l.y.clamp(-self.half.y, self.half.y),
            0.0,
        );
        (l.distance(q) <= r).then(|| self.world.transform_point(q))
    }
}

/// A sash's panes in its own space (`openings.py`, `window_sash`): centre
/// and size, before the sash is stretched to its opening.
pub fn sash_panes() -> [(Vec2, Vec2); 4] {
    const SW: f32 = 1.0;
    const SH: f32 = 1.1;
    const F: f32 = 0.06;
    const S: f32 = 0.04;
    let ty = SH / 2.0 - 0.3;
    let (xin, yin) = (SW / 2.0 - F, SH / 2.0 - F);
    let pane = |x0: f32, x1: f32, y0: f32, y1: f32| {
        (
            Vec2::new((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            Vec2::new(x1 - x0 - 2.0 * S + 0.006, y1 - y0 - 2.0 * S + 0.006),
        )
    };
    [
        pane(-xin, -0.025, -yin, ty - 0.025),
        pane(0.025, xin, -yin, ty - 0.025),
        pane(-xin, 0.08, ty + 0.025, yin),
        pane(0.12, xin, ty + 0.025, yin),
    ]
}

/// Every pane of a building, in building space: (pose, half extents,
/// which sash it's in, counting the building's sashes in order). The
/// glass sits 1 cm behind the sash's centre plane, as modelled.
pub fn panes_of(geo: &Geo) -> Vec<(Transform, Vec2, usize)> {
    let mut out = Vec::new();
    let mut sash = 0;
    for d in &geo.props {
        if d.models.is_empty() || d.models.iter().any(|(n, _)| *n != SASH) {
            continue;
        }
        for (_, xf) in &d.models {
            sash += 1;
            for (c, size) in sash_panes() {
                let s = xf.scale;
                let local = Vec3::new(c.x * s.x, c.y * s.y, -0.01 * s.z);
                out.push((
                    Transform::from_translation(xf.translation + xf.rotation * local)
                        .with_rotation(xf.rotation),
                    Vec2::new(size.x * s.x, size.y * s.y) * 0.5,
                    sash - 1,
                ));
            }
        }
    }
    out
}

/// Which of a building's pieces are window glass that the panes draw in
/// their place (`buildings::dress_buildings` leaves them out).
pub fn window_glass(geo: &Geo) -> Vec<bool> {
    let mut out = vec![false; geo.pieces.len()];
    for d in &geo.props {
        if d.models.is_empty() || d.models.iter().any(|(n, _)| *n != SASH) {
            continue;
        }
        for i in d.pieces.clone() {
            if geo.pieces[i].mat == crate::render::materials::Mat::Glass {
                out[i] = true;
            }
        }
    }
    out
}

/// Whether a prop model's primitive of this material is a pane's glass
/// (drawn by the panes, not merged into the building).
pub fn drawn_by_panes(model: &str, material: &str) -> bool {
    model == SASH && material.eq_ignore_ascii_case("glass")
}

/// Panes by 4 m cell of the ground, for the sweeps.
#[derive(Resource, Default, Debug)]
pub struct PaneGrid(pub HashMap<(i32, i32), Vec<Entity>>);

const CELL: f32 = 4.0;

fn cell(p: Vec3) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32)
}

impl PaneGrid {
    /// Panes in the cells a segment's box touches.
    pub fn near(&self, a: Vec3, b: Vec3, pad: f32) -> impl Iterator<Item = Entity> + '_ {
        let (lo, hi) = (
            a.min(b) - Vec3::splat(pad + 1.5),
            a.max(b) + Vec3::splat(pad + 1.5),
        );
        let (c0, c1) = (cell(lo), cell(hi));
        // A rifle round crosses a few cells a tick; a long step is skipped
        // past the first 64 cells rather than walked.
        let n = ((c1.0 - c0.0 + 1) * (c1.1 - c0.1 + 1)).max(0);
        let (c0, c1) = if n > 64 {
            (cell(a.min(b)), cell(a.min(b)))
        } else {
            (c0, c1)
        };
        (c0.0..=c1.0)
            .flat_map(move |x| (c0.1..=c1.1).map(move |z| (x, z)))
            .filter_map(|c| self.0.get(&c))
            .flatten()
            .copied()
    }
}

pub struct GlassPlugin;

impl Plugin for GlassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaneGrid>()
            .add_systems(FixedUpdate, glaze.before(super::DestructionSystems))
            .add_systems(
                FixedUpdate,
                // After the shots are out, before they fly: each round's
                // step this tick is swept for panes.
                sweep_rounds
                    .in_set(crate::weapons::ballistics::BallisticsSystems)
                    .after(crate::weapons::ballistics::fire_shots)
                    .before(crate::weapons::ballistics::fly_projectiles),
            )
            .add_message::<Crunch>()
            .add_systems(
                FixedUpdate,
                (sweep_movers, blasts, crunch).in_set(super::DestructionSystems),
            );
    }
}

/// Glaze each building as it appears: a pane entity per pane, already
/// broken if the save says so.
fn glaze(
    mut commands: Commands,
    buildings: Query<(Entity, &Building, &Transform), Added<Building>>,
    broken: Res<Broken>,
    mut grid: ResMut<PaneGrid>,
) {
    for (b, building, bxf) in &buildings {
        let Some(geo) = crate::world::buildings::prefab(&building.prefab) else {
            continue;
        };
        let years = weathered_sashes(&building.prefab, bxf);
        for (local, half, sash) in panes_of(&geo) {
            let world = *bxf * local;
            let k = key(world.translation);
            let broken = if years.get(sash).copied().unwrap_or(false) {
                Some(Break {
                    seed: seed_of(k),
                    impact: Vec2::ZERO,
                    weathered: true,
                    fell: 1.0,
                })
            } else {
                broken.0.contains(&k).then(|| restored(k))
            };
            let e = commands
                .spawn((
                    Name::new("Pane"),
                    Pane {
                        half,
                        building: b,
                        world,
                        inv: world.compute_affine().inverse(),
                        broken,
                    },
                    local,
                    ChildOf(b),
                ))
                .id();
            grid.0.entry(cell(world.translation)).or_default().push(e);
        }
    }
}

/// A pane that was broken in an earlier session: the same teeth every
/// time (struck in the middle).
fn restored(k: [i32; 3]) -> Break {
    Break {
        seed: seed_of(k),
        impact: Vec2::ZERO,
        weathered: false,
        // Most windows are broken from outside.
        fell: 1.0,
    }
}

/// Which of a building's sashes (in [`panes_of`]'s order) the years have
/// already broken: World models' decay swaps those to `window_sash_broken`
/// (`buildings::dress_buildings`), and their panes start broken here so
/// they draw no glass over it and can't break again. Empty: none.
///
/// The decision is World models' (`world::decay::weathered_sashes`, the
/// same one `dress_buildings` swaps the models by), so the client's look
/// and the logic agree.
pub fn weathered_sashes(prefab: &str, at: &Transform) -> Vec<bool> {
    crate::world::decay::weathered_sashes(prefab, at)
}

pub fn seed_of(k: [i32; 3]) -> u64 {
    (k[0] as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (k[1] as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (k[2] as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
}

/// How many cells a pane of this area breaks into.
pub fn cells_for(area: f32) -> usize {
    (10.0 + area * 40.0).clamp(10.0, 28.0) as usize
}

/// Break a pane struck at `at` (world), its pieces pushed along `push`.
pub fn shatter(
    commands: &mut Commands,
    pool: &mut DebrisPool,
    broken: &mut Broken,
    out: &mut MessageWriter<Broke>,
    pane: &mut Pane,
    at: Vec3,
    push: Vec3,
) {
    if pane.broken.is_some() {
        return;
    }
    let k = key(pane.centre());
    let l = pane.inv.transform_point3(at);
    let impact = Vec2::new(l.x, l.y).clamp(-pane.half, pane.half);
    let seed = seed_of(k)
        ^ ((impact * 100.0).round().as_ivec2().x as u64) << 20
        ^ (impact * 100.0).round().as_ivec2().y as u64;
    let normal = pane.world.rotation * Vec3::Z;
    let b = Break {
        seed,
        impact,
        weathered: false,
        fell: if push.dot(normal) < 0.0 { -1.0 } else { 1.0 },
    };
    pane.broken = Some(b);
    broken.0.insert(k);
    out.write(Broke {
        at: pane.centre(),
        stuff: Stuff::Glass,
        size: pane.area(),
    });
    let s = fracture::shatter(
        pane.half.to_array(),
        impact.to_array(),
        seed,
        cells_for(pane.area()),
    );
    let mut rng = Rng::new(seed ^ 0xF1);
    // The biggest pieces get bodies; the crumbs are the impact's glitter.
    let mut falling = s.falling;
    falling.sort_by(|a, b| b.area().total_cmp(&a.area()));
    let pieces = falling
        .into_iter()
        .take(debris::PER_BREAK)
        .map(|sh| {
            let centre = pane
                .world
                .transform_point(Vec3::new(sh.at[0], sh.at[1], 0.0));
            let (velocity, spin) = debris::fling(&mut rng, centre, at, push);
            Piece {
                kind: Kind::Glass,
                poly: sh.poly,
                thick: THICK,
                at: Transform::from_translation(centre).with_rotation(pane.world.rotation),
                velocity,
                spin,
            }
        })
        .collect();
    debris::throw(commands, pool, pieces);
}

/// A round's push on the glass it breaks: a few metres a second its way.
fn round_push(v: Vec3) -> Vec3 {
    v.normalize_or_zero() * 3.0
}

/// Rounds and fragments: this tick's step, up to where it would stop.
#[allow(clippy::too_many_arguments)]
fn sweep_rounds(
    mut commands: Commands,
    rounds: Query<(&crate::weapons::ballistics::Projectile, &Transform)>,
    mut panes: Query<&mut Pane>,
    grid: Res<PaneGrid>,
    spatial: SpatialQuery,
    parents: Query<&ChildOf>,
    time: Res<Time>,
    mut pool: ResMut<DebrisPool>,
    mut broken: ResMut<Broken>,
    mut out: MessageWriter<Broke>,
) {
    if grid.0.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let filter =
        SpatialQueryFilter::from_mask([Layer::World, Layer::Item, Layer::Robot, Layer::Player]);
    for (p, t) in &rounds {
        let a = t.translation;
        let b = a + p.velocity * dt;
        let mut crossed: Vec<(f32, Entity, Vec3)> = grid
            .near(a, b, 0.0)
            .filter_map(|e| {
                let pane = panes.get(e).ok()?;
                if pane.broken.is_some() {
                    return None;
                }
                pane.crossing(a, b, 0.0).map(|x| (a.distance(x), e, x))
            })
            .collect();
        if crossed.is_empty() {
            continue;
        }
        // Where the round stops (or turns) this step, if before the glass.
        let own = |hit: Entity| {
            let mut cur = hit;
            for _ in 0..7 {
                if cur == p.gun || Some(cur) == p.shooter {
                    return true;
                }
                match parents.get(cur) {
                    Ok(c) => cur = c.parent(),
                    Err(_) => break,
                }
            }
            false
        };
        let stop = Dir3::new(b - a)
            .ok()
            .and_then(|dir| {
                spatial.cast_ray_predicate(a, dir, a.distance(b), true, &filter, &|h| !own(h))
            })
            .map_or(f32::INFINITY, |h| h.distance);
        crossed.sort_by(|x, y| x.0.total_cmp(&y.0));
        for (d, e, x) in crossed {
            if d > stop {
                break;
            }
            if let Ok(mut pane) = panes.get_mut(e) {
                shatter(
                    &mut commands,
                    &mut pool,
                    &mut broken,
                    &mut out,
                    &mut pane,
                    x,
                    round_push(p.velocity),
                );
            }
        }
    }
}

/// Things thrown, swung or punched through, and robots walking into it.
#[allow(clippy::too_many_arguments)]
fn sweep_movers(
    mut commands: Commands,
    items: Query<
        (Entity, &GlobalTransform, Option<&crate::interact::Held>),
        (
            With<crate::items::Item>,
            Without<crate::interact::body::InBackpack>,
        ),
    >,
    hands: Query<(Entity, &GlobalTransform, &crate::rig::RigHand), With<crate::interact::Hand>>,
    mut felt: MessageWriter<crate::rig::haptics::Haptic>,
    robots: Query<(&crate::robots::Robot, &GlobalTransform)>,
    mut panes: Query<&mut Pane>,
    grid: Res<PaneGrid>,
    time: Res<Time>,
    mut last: Local<HashMap<Entity, Vec3>>,
    mut pool: ResMut<DebrisPool>,
    mut broken: ResMut<Broken>,
    mut out: MessageWriter<Broke>,
) {
    if grid.0.is_empty() {
        return;
    }
    let dt = time.delta_secs().max(1e-4);
    let mut seen = HashMap::with_capacity(last.len());
    // (pane, where, push, the hand that did it)
    let mut hits: Vec<(Entity, Vec3, Vec3, Option<crate::rig::Side>)> = Vec::new();
    let movers = items
        .iter()
        .map(|(e, gt, held)| (e, gt, held.map(|h| h.side)))
        .chain(hands.iter().map(|(e, gt, h)| (e, gt, Some(h.side))));
    for (e, gt, side) in movers {
        let now = gt.translation();
        seen.insert(e, now);
        let Some(was) = last.get(&e).copied() else {
            continue;
        };
        let v = (now - was) / dt;
        // A teleport (stowed and drawn, respawned) isn't a swing.
        if v.length() < BREAK_SPEED || v.length() > 60.0 {
            continue;
        }
        for pe in grid.near(was, now, 0.05) {
            let Ok(pane) = panes.get(pe) else { continue };
            if pane.broken.is_some() {
                continue;
            }
            if let Some(x) = pane.crossing(was, now, 0.05) {
                hits.push((pe, x, v * 0.5, side));
            }
        }
    }
    *last = seen;
    // A robot's body: a few points up its middle (its root is the
    // body's centre, a walker's 1.15 m up), as wide as it is.
    for (r, gt) in &robots {
        if r.wrecked {
            continue;
        }
        let (scale, _, at) = gt.to_scale_rotation_translation();
        let s = scale.y.max(0.3);
        for h in [-0.5, 0.0, 0.4] {
            let p = at + Vec3::Y * h * s;
            for pe in grid.near(p, p, 0.5 * s) {
                let Ok(pane) = panes.get(pe) else { continue };
                if pane.broken.is_some() {
                    continue;
                }
                if let Some(x) = pane.touching(p, 0.4 * s) {
                    hits.push((pe, x, (x - p).normalize_or_zero() * 2.0, None));
                }
            }
        }
    }
    for (pe, x, push, side) in hits {
        if let Ok(mut pane) = panes.get_mut(pe) {
            // Glass giving way on your fist or what's in it: a hit's kick.
            if let (Some(side), None) = (side, pane.broken) {
                felt.write(crate::rig::haptics::Haptic {
                    side,
                    amplitude: 0.65,
                    seconds: 0.04,
                });
            }
            shatter(
                &mut commands,
                &mut pool,
                &mut broken,
                &mut out,
                &mut pane,
                x,
                push,
            );
        }
    }
}

/// A grenade's blast breaks the panes about it that it can see.
#[allow(clippy::too_many_arguments)]
fn blasts(
    mut commands: Commands,
    mut events: MessageReader<crate::weapons::grenades::GrenadeEvent>,
    mut panes: Query<&mut Pane>,
    grid: Res<PaneGrid>,
    spatial: SpatialQuery,
    mut pool: ResMut<DebrisPool>,
    mut broken: ResMut<Broken>,
    mut out: MessageWriter<Broke>,
) {
    for ev in events.read() {
        if ev.event != extraction_core::grenade::Event::Exploded {
            continue;
        }
        let at = ev.at;
        let near: Vec<Entity> = grid.near(at, at, BLAST_REACH).collect();
        for pe in near {
            let Ok(mut pane) = panes.get_mut(pe) else {
                continue;
            };
            let c = pane.centre();
            let v = c - at;
            if pane.broken.is_some() || v.length() > BLAST_REACH {
                continue;
            }
            let clear = Dir3::new(v).ok().is_none_or(|dir| {
                spatial
                    .cast_ray(
                        at,
                        dir,
                        v.length() - 0.05,
                        true,
                        &SpatialQueryFilter::from_mask(Layer::World),
                    )
                    .is_none()
            });
            if clear {
                let k = 1.0 - v.length() / BLAST_REACH;
                shatter(
                    &mut commands,
                    &mut pool,
                    &mut broken,
                    &mut out,
                    &mut pane,
                    c,
                    v.normalize_or_zero() * (2.0 + 8.0 * k),
                );
            }
        }
    }
}

/// A step on broken glass (for its sound).
#[derive(Message, Clone, Copy, Debug)]
pub struct Crunch {
    pub at: Vec3,
}

/// How far a robot hears a step on broken glass, metres: further than a
/// step on boards. A smashed window is an alarm you leave yourself.
pub const CRUNCH_HEARD: f32 = 22.0;

/// Whether `feet` stand in a broken pane's glass on the floor: below it,
/// within its width and up to 0.9 m out on the side it fell.
pub fn in_the_glass(pane: &Pane, feet: Vec3) -> bool {
    let Some(b) = pane.broken.filter(|b| !b.weathered) else {
        return false;
    };
    let l = pane.inv.transform_point3(feet);
    let out = l.z * b.fell;
    l.x.abs() < pane.half.x + 0.3
        && (-0.1..0.95).contains(&out)
        && l.y < -pane.half.y
        && l.y > -pane.half.y - 2.2
}

/// Steps in broken glass crunch, and the robots hear it.
fn crunch(
    mut steps: MessageReader<crate::player::Footstep>,
    panes: Query<&Pane>,
    grid: Res<PaneGrid>,
    mut out: MessageWriter<Crunch>,
    mut noise: MessageWriter<crate::robots::Noise>,
) {
    for s in steps.read() {
        let p = s.position;
        if grid
            .near(p, p, 1.0)
            .filter_map(|e| panes.get(e).ok())
            .any(|pane| in_the_glass(pane, p))
        {
            out.write(Crunch { at: p });
            noise.write(crate::robots::Noise {
                at: p,
                radius: CRUNCH_HEARD,
                source: Some(s.entity),
            });
        }
    }
}

/// The Surge: every pane whole again.
pub fn mend_all(world: &mut World) {
    let mut q = world.query::<&mut Pane>();
    for mut p in q.iter_mut(world) {
        if p.broken.is_some_and(|b| !b.weathered) {
            p.broken = None;
        }
    }
}
