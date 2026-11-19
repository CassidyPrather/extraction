//! A dirigible's gas cells going slack (Robot bodies): a holed cell sags
//! in the air, its crown fallen in and its belly hanging; a ship down on
//! the ground lies with its cells draped over the gondola and the grass,
//! pressed low, spread wide, creased along their gores, the holed ones
//! flatter than the one still holding gas. Its eye is tucked under the
//! gondola's nose, clear of the ground.
//!
//! Only the drawing changes: the cells' meshes are reshaped in each cell's
//! own space (`slack_point`), from the model's (`cell_0`..`cell_2`) or the
//! procedural look's, whichever dressed it. The parts, their colliders and
//! the ship's flight (`dirigible`) stay as they are, so whatever is seated
//! on a part entity rides with it; [`slack_point`] says where a point of a
//! taut cell has gone, for anything that should sit on the sagged fabric.

use bevy::prelude::*;

use super::dirigible::{Dirigible, RADIUS};
use super::{PartRole, Robot, RobotPart};
use crate::world::Zone;

pub struct DirigibleLookPlugin;

impl Plugin for DirigibleLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (slacken_cells, tuck_eye));
    }
}

/// How a gas cell hangs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slack {
    /// Full of gas: as modelled.
    Taut,
    /// Holed in the air: its crown fallen in, its belly hanging.
    Holed,
    /// The ship down on the ground: draped over the gondola and the grass,
    /// flatter if it's `holed`.
    Down { holed: bool },
}

/// A slack cell's crown over the ground (m): one that's lost its gas, and
/// one that still has some.
pub const DOWN_TALL_HOLED: f32 = 0.9;
pub const DOWN_TALL: f32 = 1.2;

/// The ground (and what lies on it) under a cell, in the cell's space: its
/// height at x, z.
pub struct Floor {
    pub ground: f32,
    /// The gondola's top and its half extents (x, z) about its middle, in
    /// the cell's space: the fabric lies over it.
    pub gondola: Option<(f32, Vec2, Vec2)>,
}

impl Floor {
    pub fn at(&self, x: f32, z: f32) -> f32 {
        // Full over the gondola, rounding off to the grass round it: the
        // fabric drapes over a hump, not a block.
        match self.gondola {
            Some((top, mid, half)) if top > self.ground => {
                let d = ((x - mid.x).abs() - half.x).max(0.0);
                let e = ((z - mid.y).abs() - half.y).max(0.0);
                let k = (1.0 - (d * d + e * e).sqrt() / GONDOLA_FALL).clamp(0.0, 1.0);
                self.ground + (top - self.ground) * k * k * (3.0 - 2.0 * k)
            }
            _ => self.ground,
        }
    }
}

/// Where a point `p` of a taut cell (in the cell's space: its middle at the
/// origin, +Y up, its length along Z, `half` its half extents) lies once
/// it hangs as `slack`.
pub fn slack_point(p: Vec3, half: Vec3, slack: Slack, floor: &Floor) -> Vec3 {
    let up = (p.y / half.y).clamp(-1.0, 1.0);
    // Round the long axis: the gores run along it.
    let a = p.y.atan2(p.x);
    let side = (p.x.abs() / half.x).min(1.0);
    match slack {
        Slack::Taut => p,
        Slack::Holed => {
            // The crown falls in and the whole cell drops on its frame; the
            // belly bags a little wider; the ends pucker.
            let y = p.y * 0.6 - half.y * 0.3
                + 0.07 * (a * 9.0 + p.z * 1.9).sin() * (1.0 - 0.5 * up.abs());
            let x = p.x * (0.9 + 0.08 * (-up).max(0.0)) + 0.05 * (p.z * 2.3 + p.y * 3.0).sin();
            Vec3::new(x, y, p.z * 0.95)
        }
        Slack::Down { holed } => {
            let tall = if holed { DOWN_TALL_HOLED } else { DOWN_TALL };
            // 0 at its belly to 1 at its crown.
            let u = up * 0.5 + 0.5;
            // Wide at its foot, drawing in toward the crown: a mound, not
            // a box.
            let spread = (1.15 + 0.25 * (1.0 - u)) * (1.0 - 0.35 * u * u);
            let x = p.x * spread;
            let z = p.z * (1.0 + 0.05 * (1.0 - u));
            // (Its lower half pressed flat; only toward the crown does it
            // stand off the ground.)
            // A holed cell's lower half pressed flat; a full one a pillow,
            // its shoulders rounded off.
            let lift = if holed { u.powf(2.2) } else { u.powf(1.8) };
            let mut y = floor.ground + 0.02 + lift * tall;
            // Creases along the gores, and across them, deeper toward the
            // rim.
            let deep = if holed { 1.0 } else { 1.5 };
            y += (0.07 * (a * 10.0 + p.z * 1.6).sin() + 0.035 * (z * 3.3 + x * 2.1).sin())
                * side
                * deep;
            // The rim droops to the ground.
            let rim = ((x.abs() - half.x) / (0.5 * half.x)).clamp(0.0, 1.0);
            y -= 0.4 * rim * rim * (3.0 - 2.0 * rim);
            // One side fuller than the other.
            y += 0.04 * x;
            // Slumped toward its ends too: no lift holds it up now, so
            // there's no flat top and its end rims come down to the grass.
            let end = (p.z.abs() / half.z).clamp(0.0, 1.0);
            y = floor.ground + (y - floor.ground) * (1.0 - 0.6 * end * end);
            Vec3::new(x, y.max(floor.at(x, z) + 0.02), z)
        }
    }
}

/// Where a point of a taut cell (`local`, in its part's space, as seated
/// things are placed) lies once the ship is down with that cell `holed` or
/// not: on the draped fabric, for anything seated on a cell (its gas
/// valve). `cell_at` is the cell part's place in the ship (`dirigible`
/// spawns them at z = -4.8, 0, 4.8). The ship down, its root stands
/// RADIUS + 1 over the ground.
pub fn down_point(local: Vec3, cell_at: Vec3, holed: bool) -> Vec3 {
    let floor = Floor {
        ground: -RADIUS - 1.0 - cell_at.y,
        gondola: Some((
            GONDOLA_TOP - cell_at.y,
            Vec2::new(-cell_at.x, -cell_at.z),
            GONDOLA_HALF,
        )),
    };
    slack_point(
        local,
        Vec3::new(RADIUS, RADIUS, RADIUS),
        Slack::Down { holed },
        &floor,
    )
}

/// What the fabric drapes over once the ship is down (in the ship's
/// space): the gondola with its engine pods and their mounts, a little
/// clear so none of it shows through the fabric's coarse mesh.
const GONDOLA_TOP: f32 = -RADIUS - 0.5 + 0.55 + 0.3;
const GONDOLA_HALF: Vec2 = Vec2::new(0.8, 2.1);
/// How far round it the hump falls away to the grass (m).
const GONDOLA_FALL: f32 = 1.2;

/// A cell's look, settling from one slack to the next.
#[derive(Component, Clone, Copy, Debug)]
struct CellLook {
    from: Slack,
    to: Slack,
    /// 0..1 from `from` to `to`.
    k: f32,
    /// The `k` it was last drawn at.
    drawn: f32,
}

/// A mesh of a cell's, reshaped from its own copy of the original.
#[derive(Component)]
struct Slackened {
    original: Handle<Mesh>,
}

/// How long a cell takes to settle (s), and how many times on the way it's
/// redrawn.
const SETTLE: f32 = 1.5;
const STEPS: f32 = 4.0;

/// Whether the ship's down on the ground.
fn down(d: &Dirigible, robot: &Robot, root: Vec3, zone: &Zone) -> bool {
    d.sinking
        && robot.wrecked
        && d.velocity == Vec3::ZERO
        && root.y - zone.ground(root.x, root.z) <= RADIUS + 1.05
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn slacken_cells(
    mut commands: Commands,
    ships: Query<(&Dirigible, &Robot, &GlobalTransform, &Children)>,
    mut cells: Query<(
        Entity,
        &RobotPart,
        &Transform,
        &GlobalTransform,
        Option<&mut CellLook>,
    )>,
    kids: Query<&Children>,
    looks: Query<(&Mesh3d, &GlobalTransform, Option<&Slackened>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    zone: Res<Zone>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (d, robot, gt, children) in &ships {
        let root = gt.translation();
        let is_down = down(d, robot, root, &zone);
        for c in children.iter() {
            let Ok((cell, part, local, cell_gt, look)) = cells.get_mut(c) else {
                continue;
            };
            let PartRole::GasCell(i) = part.role else {
                continue;
            };
            let holed = !d.cells.get(i as usize).copied().unwrap_or(true);
            let want = if is_down {
                Slack::Down { holed }
            } else if holed {
                Slack::Holed
            } else {
                Slack::Taut
            };
            let mut l = match look {
                Some(l) => *l,
                None if want == Slack::Taut => continue,
                None => CellLook {
                    from: Slack::Taut,
                    to: want,
                    k: 0.0,
                    drawn: -1.0,
                },
            };
            if l.to != want {
                // From where it's got to (near enough: the nearer end).
                l.from = if l.k < 0.5 { l.from } else { l.to };
                l.to = want;
                l.k = 0.0;
                l.drawn = -1.0;
            }
            l.k = (l.k + dt / SETTLE).min(1.0);
            let step = (l.k * STEPS).floor() / STEPS;
            if step != l.drawn {
                l.drawn = step;
                let k = step * step * (3.0 - 2.0 * step);
                draw_cell(
                    &mut commands,
                    cell,
                    local,
                    cell_gt,
                    l,
                    k,
                    &kids,
                    &looks,
                    &mut meshes,
                    &zone,
                );
            }
            commands.entity(cell).insert(l);
        }
    }
}

/// Reshape every mesh of `cell` (itself and its descendants) to `k` of
/// the way from `l.from` to `l.to`.
#[allow(clippy::too_many_arguments)]
fn draw_cell(
    commands: &mut Commands,
    cell: Entity,
    local: &Transform,
    cell_gt: &GlobalTransform,
    l: CellLook,
    k: f32,
    kids: &Query<&Children>,
    looks: &Query<(&Mesh3d, &GlobalTransform, Option<&Slackened>)>,
    meshes: &mut Assets<Mesh>,
    zone: &Zone,
) {
    // The cell's own frame, without any scale its look carries.
    let (_, rot, at) = cell_gt.to_scale_rotation_translation();
    let frame = bevy::math::Affine3A::from_rotation_translation(rot, at);
    let to_cell = frame.inverse();
    let mut found = vec![cell];
    found.extend(kids.iter_descendants(cell));
    let mut drawn = Vec::new();
    for e in found {
        let Ok((mesh, gt, slackened)) = looks.get(e) else {
            continue;
        };
        let original = slackened.map_or(mesh.0.clone(), |s| s.original.clone());
        let Some(src) = meshes.get(&original) else {
            continue;
        };
        let m = to_cell * gt.affine();
        drawn.push((e, original.clone(), src.clone(), m));
    }
    if drawn.is_empty() {
        return;
    }
    // The cell's extents, over all its meshes.
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for (_, _, src, m) in &drawn {
        if let Some(bevy::mesh::VertexAttributeValues::Float32x3(ps)) =
            src.attribute(Mesh::ATTRIBUTE_POSITION)
        {
            for p in ps {
                let q = m.transform_point3(Vec3::from(*p));
                lo = lo.min(q);
                hi = hi.max(q);
            }
        }
    }
    let mid = (lo + hi) * 0.5;
    let half = ((hi - lo) * 0.5).max(Vec3::splat(0.1));
    // The ground and the gondola, in the cell's space (the gondola's top
    // and footprint as `dirigible` spawns it, in the ship's space).
    let ground = zone.ground(at.x, at.z) - at.y;
    let gondola = (
        GONDOLA_TOP - local.translation.y,
        Vec2::new(-local.translation.x, -local.translation.z),
        GONDOLA_HALF,
    );
    let floor = Floor {
        ground,
        gondola: Some(gondola),
    };
    for (e, original, src, m) in drawn {
        let back = m.inverse();
        let mut out = src;
        if let Some(bevy::mesh::VertexAttributeValues::Float32x3(ps)) =
            out.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            for p in ps.iter_mut() {
                let q = m.transform_point3(Vec3::from(*p)) - mid;
                let a = slack_point(q, half, l.from, &floor);
                let b = slack_point(q, half, l.to, &floor);
                *p = back.transform_point3(a.lerp(b, k) + mid).to_array();
            }
        }
        out.remove_attribute(Mesh::ATTRIBUTE_TANGENT);
        if out.indices().is_some() {
            out.compute_smooth_normals();
        } else {
            out.compute_flat_normals();
        }
        let _ = out.generate_tangents();
        let h = meshes.add(out);
        commands
            .entity(e)
            .insert((Mesh3d(h), Slackened { original }));
    }
}

/// The eye's place once the ship's down: under the gondola's nose, its
/// underside a hand clear of the ground (in the ship's space, over the
/// ground under it).
pub const EYE_DOWN: Vec3 = Vec3::new(0.0, 0.35, -2.15);

/// Down on the ground, the eye (which hangs under the gondola in flight)
/// is tucked under the gondola's nose, clear of the ground.
fn tuck_eye(
    ships: Query<(&Dirigible, &Robot, &GlobalTransform, &Children)>,
    mut parts: Query<(&RobotPart, &mut Transform)>,
    zone: Res<Zone>,
) {
    for (d, robot, gt, children) in &ships {
        let root = gt.translation();
        if !down(d, robot, root, &zone) {
            continue;
        }
        let ground = zone.ground(root.x, root.z) - root.y;
        let want = Vec3::new(EYE_DOWN.x, ground + EYE_DOWN.y, EYE_DOWN.z);
        for c in children.iter() {
            let Ok((p, mut t)) = parts.get_mut(c) else {
                continue;
            };
            if p.role == PartRole::Sensor && t.translation.distance(want) > 0.01 {
                t.translation = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor(ground: f32) -> Floor {
        Floor {
            ground,
            gondola: Some((ground + 1.1, Vec2::ZERO, GONDOLA_HALF)),
        }
    }

    /// Points round a taut cell (RADIUS across, `len` half its length).
    fn cell(len: f32) -> impl Iterator<Item = Vec3> {
        (0..24).flat_map(move |i| {
            (0..=12).map(move |j| {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                let v = j as f32 / 12.0 * std::f32::consts::PI;
                Vec3::new(
                    RADIUS * v.sin() * a.cos(),
                    RADIUS * v.cos(),
                    len * v.sin() * a.sin(),
                )
            })
        })
    }

    /// Down, its cells lie on the ground and over the gondola, not as a
    /// rigid ship parked on the grass: every point lands between the
    /// ground and the crown's height (a holed cell lower), its rim on the
    /// ground, and it spreads wider than it flew.
    #[test]
    fn a_downed_dirigibles_cells_lie_slack_on_the_ground() {
        let ground = -RADIUS - 1.0;
        let half = Vec3::new(RADIUS, RADIUS, 3.0);
        let f = floor(ground);
        for holed in [true, false] {
            let tall = if holed { DOWN_TALL_HOLED } else { DOWN_TALL };
            let mut lowest_rim = f32::MAX;
            let mut widest = 0.0f32;
            let mut highest = f32::MIN;
            for p in cell(3.0) {
                let d = slack_point(p, half, Slack::Down { holed }, &f);
                assert!(
                    d.y >= ground && d.y <= ground + tall.max(1.1) + 0.3,
                    "{p} lies at {} over the ground (holed {holed})",
                    d.y - ground
                );
                if p.y.abs() < 0.05 {
                    lowest_rim = lowest_rim.min(d.y - ground);
                }
                widest = widest.max(d.x.abs());
                highest = highest.max(d.y - ground);
            }
            assert!(lowest_rim < 0.15, "its rim on the ground: {lowest_rim}");
            assert!(widest > RADIUS * 1.15, "spread: {widest}");
            assert!(highest < 2.0 * RADIUS * 0.5, "flat, not round: {highest}");
            // Even the cell still holding gas is a low pillow: under half a
            // taut cell's height (2 RADIUS), and its ends lower than its
            // middle.
            if !holed {
                assert!(highest <= RADIUS, "a low pillow: {highest}");
                let end = slack_point(
                    Vec3::new(0.0, RADIUS * 0.5, 2.9),
                    half,
                    Slack::Down { holed },
                    &f,
                );
                let mid = slack_point(
                    Vec3::new(0.0, RADIUS * 0.5, 0.0),
                    half,
                    Slack::Down { holed },
                    &f,
                );
                assert!(
                    end.y < mid.y - 0.2,
                    "its ends slumped: {} vs {}",
                    end.y,
                    mid.y
                );
            }
        }
    }

    /// Something seated on a cell's lower flank (Heavy's gas valve) lies
    /// on the draped fabric once the ship is down: low, over the ground.
    #[test]
    fn a_valve_on_a_downed_cell_lies_on_the_fabric() {
        for (z, holed) in [(-4.8, true), (0.0, false), (4.8, true)] {
            let p = down_point(Vec3::new(1.85, -1.85, 0.0), Vec3::new(0.0, 0.0, z), holed);
            let over = p.y - (-RADIUS - 1.0);
            assert!((0.0..0.6).contains(&over), "{z}: {over} over the ground");
        }
    }

    /// Holed in the air, a cell sags: its crown falls well below a taut
    /// one's, and it's no taller than it was.
    #[test]
    fn a_holed_cell_sags_in_the_air() {
        let half = Vec3::new(RADIUS, RADIUS, 3.0);
        let f = floor(-100.0);
        let crown = slack_point(Vec3::Y * RADIUS, half, Slack::Holed, &f);
        assert!(crown.y < RADIUS * 0.5, "crown fallen in: {}", crown.y);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for p in cell(3.0) {
            let y = slack_point(p, half, Slack::Holed, &f).y;
            lo = lo.min(y);
            hi = hi.max(y);
        }
        assert!(hi - lo < 2.0 * RADIUS * 0.75, "shrunk: {}", hi - lo);
    }

    /// A dirigible holed and come down: its eye ends clear of the ground
    /// (it used to land 0.1 m under it), and its cells are drawn lying low
    /// over the gondola, not round and full size.
    #[test]
    fn a_downed_dirigible_lies_slack_with_its_eye_clear() {
        let opts = crate::LaunchOptions {
            map: "training_yard".into(),
            map_explicit: true,
            new_game: true,
            robots: false,
            ..Default::default()
        };
        let mut app = crate::soak::drive::client_app(opts);
        for _ in 0..30 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let at = zone.on_ground(0.0, 0.0) + Vec3::Y * (RADIUS + 1.6);
        let ship = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let job = super::super::Job {
                kind: super::super::JobKind::Survey,
                site: at,
                duration: 1.0e6,
                exit: at,
            };
            let e = super::super::dirigible::spawn_dirigible(&mut c, at, job);
            q.apply(app.world_mut());
            e
        };
        // Let it be dressed, then hole two cells: it sinks.
        for _ in 0..120 {
            app.update();
        }
        {
            let mut d = app.world_mut().get_mut::<Dirigible>(ship).unwrap();
            d.cells = [false, true, false];
            d.sinking = true;
        }
        for _ in 0..400 {
            app.update();
        }
        let w = app.world_mut();
        let root = w.get::<GlobalTransform>(ship).unwrap().translation();
        let ground = zone.ground(root.x, root.z);
        assert!(
            w.get::<Robot>(ship).unwrap().wrecked,
            "it came down: {} over the ground",
            root.y - ground
        );
        let parts: Vec<(Entity, PartRole)> = w
            .query::<(Entity, &RobotPart)>()
            .iter(w)
            .filter(|(_, p)| p.robot == ship)
            .map(|(e, p)| (e, p.role))
            .collect();
        // Its eye: the part, or the salvage seated in its place once it's
        // down (Heavy's walker_eye on the gondola's nose, which takes the
        // part's place).
        let catalog = w.resource::<crate::items::Catalog>().clone();
        let seated_eye = w
            .query::<(Entity, &crate::items::Item, &GlobalTransform)>()
            .iter(w)
            .find(|(_, it, g)| {
                catalog.get(it.0.kind).id == "walker_eye" && g.translation().distance(root) < 8.0
            })
            .map(|(e, ..)| e);
        let eye = parts
            .iter()
            .find(|(_, r)| *r == PartRole::Sensor)
            .map(|(e, _)| *e)
            .or(seated_eye)
            .expect("its eye, as a part or seated");
        let eye_y = w.get::<GlobalTransform>(eye).unwrap().translation().y;
        // (The part's a 0.3 m sphere; the seated eye sits on the nose.)
        let under = if seated_eye == Some(eye) { 0.0 } else { 0.3 };
        assert!(
            eye_y - under > ground,
            "the eye clear: {}",
            eye_y - under - ground
        );
        // Every drawn point of its cells lies low.
        let mut highest = f32::MIN;
        let mut slack = 0;
        for (cell, role) in &parts {
            if !matches!(role, PartRole::GasCell(_)) {
                continue;
            }
            let mut all = vec![*cell];
            let mut k = 0;
            while k < all.len() {
                if let Some(ch) = w.get::<Children>(all[k]) {
                    all.extend(ch.iter());
                }
                k += 1;
            }
            for e in all {
                let (Some(m), Some(gt)) = (w.get::<Mesh3d>(e), w.get::<GlobalTransform>(e)) else {
                    continue;
                };
                if w.get::<Slackened>(e).is_some() {
                    slack += 1;
                }
                let meshes = w.resource::<Assets<Mesh>>();
                let Some(bevy::mesh::VertexAttributeValues::Float32x3(ps)) = meshes
                    .get(&m.0)
                    .and_then(|m| m.attribute(Mesh::ATTRIBUTE_POSITION))
                else {
                    continue;
                };
                for p in ps {
                    highest = highest.max(gt.transform_point(Vec3::from(*p)).y - ground);
                }
            }
        }
        assert!(slack >= 3, "its cells reshaped: {slack}");
        assert!(
            highest < DOWN_TALL + 0.6,
            "its cells lie low: {highest} over the ground"
        );
    }
}
