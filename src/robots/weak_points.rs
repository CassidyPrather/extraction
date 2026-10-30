//! Weak points you can hit in VR, and see to hit: every robot's weak points
//! are big enough to hold a pistol on at fighting range, and wear the one
//! "shoot here" colour ([`SHOOT_HERE`]); nothing else on a robot does.
//!
//! - **What's weak** ([`is_weak`]): a part a pistol round tells on
//!   (`damage::strike`: half its health or more in one round), or a heavy's
//!   bolt (`weapons::melee::WeakPoint`). Plates never are; a walker's core
//!   isn't (four rounds), its eye, knees and turret are.
//! - **Big enough** ([`WEAK_ACROSS`]): a weak point's hit volume is at least
//!   this across however you face it, so it subtends at least what a VR
//!   pistol holds ([`VR_HOLD_DEG`]) at 25 m. Small ones are grown as they're
//!   made ([`grow`]): a leg gets a ball at its knee (the joint you'd aim at,
//!   not a fat shin); an eye, a rotor, a gun its box or ball made bigger.
//! - **Marked** ([`mark`]): a lit orange knee ball, or a ring round an eye,
//!   a rotor, a gun or a weak core, the size of what you can hit. It goes
//!   out when the part does: a dead part's no target.
//! - **Nothing else orange**: the robots' models' hazard paint ("Warn",
//!   "Yellow") is painted the robots' own teal glow instead ([`paint`]),
//!   and their lamps stay off orange (tested).

use avian3d::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::damage::{Strike, strike};
use super::{PartRole, RobotPart};

/// A pistol round's harm (the 9x18 PM's): what "weak" is measured by.
pub const PISTOL_ROUND: f32 = 34.0;
/// The cone (full angle, degrees) a VR pistol held out at arm's length
/// keeps most of its rounds in: hand tremor and tracking jitter, with the
/// sights on (about 20 cm at 20 m).
pub const VR_HOLD_DEG: f32 = 0.55;
/// The furthest a pistol fight is fought (m): at this a weak point still
/// subtends [`VR_HOLD_DEG`].
pub const FIGHT_RANGE: f32 = 25.0;
/// A weak point's hit volume is at least this across (m), however you
/// face it: [`VR_HOLD_DEG`] at [`FIGHT_RANGE`].
pub const WEAK_ACROSS: f32 = 0.24;
/// The one "shoot here" colour: a lit orange, to read through fog at 25 m.
pub const SHOOT_HERE: Color = Color::srgb(1.0, 0.4, 0.0);
/// How thick a mark's ring is (m, at a robot's own size).
const RING: f32 = 0.035;

/// Whether a part of `role` with `max_hp` is a weak point (`weak`: it
/// carries `weapons::melee::WeakPoint`, a heavy's bolt).
pub fn is_weak(role: PartRole, max_hp: f32, weak: bool) -> bool {
    if matches!(role, PartRole::Armour(_)) {
        return weak;
    }
    weak || strike(role, max_hp, max_hp, PISTOL_ROUND) == Strike::Telling
}

/// The angle (degrees) something `across` metres wide subtends at `range`.
pub fn subtends(across: f32, range: f32) -> f32 {
    (2.0 * (across * 0.5).atan2(range)).to_degrees()
}

/// What a hit volume offers to aim at: the narrowest it is across, of its
/// biggest piece (a compound's knee ball), in its own units.
pub fn across(c: &Collider) -> f32 {
    let s = c.shape();
    if let Some(b) = s.as_ball() {
        2.0 * b.radius
    } else if let Some(b) = s.as_cuboid() {
        2.0 * b.half_extents.x.min(b.half_extents.y).min(b.half_extents.z)
    } else if let Some(cy) = s.as_cylinder() {
        2.0 * cy.radius.min(cy.half_height)
    } else if let Some(cap) = s.as_capsule() {
        2.0 * cap.radius
    } else if let Some(comp) = s.as_compound() {
        comp.shapes()
            .iter()
            .map(|(_, sh)| across(&Collider::from(sh.clone())))
            .fold(0.0, f32::max)
    } else {
        0.0
    }
}

/// A weak point's hit volume grown to at least `min` across (its own
/// units), and where its mark goes; None if it's big enough already. A
/// capsule (a leg) keeps its shin and gets a ball at its top end, the knee.
pub fn grow(c: &Collider, min: f32) -> Option<(Collider, Vec3)> {
    if across(c) >= min {
        return None;
    }
    let r = min * 0.5;
    let s = c.shape();
    if let Some(b) = s.as_ball() {
        let _ = b;
        Some((Collider::sphere(r), Vec3::ZERO))
    } else if let Some(b) = s.as_cuboid() {
        let h = b.half_extents;
        Some((
            Collider::cuboid(2.0 * h.x.max(r), 2.0 * h.y.max(r), 2.0 * h.z.max(r)),
            Vec3::ZERO,
        ))
    } else if let Some(cy) = s.as_cylinder() {
        Some((
            Collider::cylinder(cy.radius.max(r), 2.0 * cy.half_height.max(r)),
            Vec3::ZERO,
        ))
    } else if let Some(cap) = s.as_capsule() {
        let (a, b) = (cap.segment.a, cap.segment.b);
        let knee = if b.y >= a.y {
            Vec3::new(b.x, b.y, b.z)
        } else {
            Vec3::new(a.x, a.y, a.z)
        };
        Some((
            Collider::compound(vec![
                (Vec3::ZERO, Quat::IDENTITY, c.clone()),
                (knee, Quat::IDENTITY, Collider::sphere(r)),
            ]),
            knee,
        ))
    } else {
        None
    }
}

/// A part's weak-point mark (a child of the part): it goes out when the
/// part does.
#[derive(Component)]
pub struct WeakMark {
    pub part: Entity,
}

/// A part that's been looked at (grown and marked, or found not weak).
#[derive(Component)]
struct Checked;

/// The robots' hazard paint, painted off the "shoot here" colour: a model
/// material name as a robot wears it.
pub fn paint(name: &str) -> &str {
    match name {
        "Warn" | "Yellow" => "Glow",
        n => n,
    }
}

/// Whether a colour reads as "shoot here": orange to yellow, and strong.
pub fn reads_as_shoot_here(c: Color) -> bool {
    let h = Hsva::from(c);
    (15.0..=65.0).contains(&h.hue) && h.saturation > 0.45 && h.value > 0.35
}

pub struct WeakPointsPlugin;

impl Plugin for WeakPointsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (mark_weak_points, mark_out).chain());
    }
}

#[derive(Resource, Clone)]
struct MarkAssets {
    ball: Handle<Mesh>,
    ring: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
}

/// Every new part: if it's weak, grow it to [`WEAK_ACROSS`] (at its robot's
/// size) and mark it.
#[allow(clippy::type_complexity)]
fn mark_weak_points(
    mut commands: Commands,
    parts: Query<
        (
            Entity,
            &RobotPart,
            &Collider,
            Has<crate::weapons::melee::WeakPoint>,
        ),
        Without<Checked>,
    >,
    roots: Query<&Transform>,
    existing: Option<Res<MarkAssets>>,
    // (Headless there's nothing to draw with: grown and marked all the same.)
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let mut a = existing.map(|a| a.clone());
    for (e, part, collider, weak) in &parts {
        commands.entity(e).insert(Checked);
        if !is_weak(part.role, part.max_hp, weak) {
            continue;
        }
        // In the part's own units: a crawler's are 0.6 a walker's.
        let scale = roots
            .get(part.robot)
            .map_or(1.0, |t| t.scale.max_element())
            .max(0.05);
        let min = WEAK_ACROSS / scale;
        let (hit, at) = match grow(collider, min) {
            Some((c, at)) => {
                let a = c.clone();
                commands.entity(e).insert(c);
                (a, at)
            }
            None => (collider.clone(), Vec3::ZERO),
        };
        let size = across(&hit).max(min);
        let (shape, t) = mark(part.role, at, size);
        let mark = commands
            .spawn((WeakMark { part: e }, t, Visibility::default(), ChildOf(e)))
            .id();
        let (Some(meshes), Some(materials)) = (meshes.as_mut(), materials.as_mut()) else {
            continue;
        };
        let a = a.get_or_insert_with(|| {
            let m = MarkAssets {
                ball: meshes.add(Sphere::new(0.5).mesh().ico(3).unwrap()),
                ring: meshes.add(Torus::new(0.5 - RING, 0.5)),
                mat: materials.add(StandardMaterial {
                    base_color: SHOOT_HERE,
                    // (Lit enough to carry through fog, not so much it
                    // washes out to white: it must read orange.)
                    emissive: LinearRgba::rgb(1.6, 0.42, 0.0),
                    perceptual_roughness: 0.5,
                    ..default()
                }),
            };
            commands.insert_resource(m.clone());
            m
        });
        commands.entity(mark).insert((
            Mesh3d(if shape == Mark::Ball {
                a.ball.clone()
            } else {
                a.ring.clone()
            }),
            MeshMaterial3d(a.mat.clone()),
            NotShadowCaster,
        ));
    }
}

/// A mark's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// A lit ball: a knee.
    Ball,
    /// A ring round it.
    Ring,
}

/// How a weak part of `role` is marked, at `at` (local), `size` across: a
/// knee is a ball a little smaller than what you can hit (it's inside the
/// leg's hit ball); an eye or a gun a ring facing you round it; a rotor or
/// a core a ring round it flat.
pub fn mark(role: PartRole, at: Vec3, size: f32) -> (Mark, Transform) {
    match role {
        PartRole::Leg(_) => (
            Mark::Ball,
            Transform::from_translation(at).with_scale(Vec3::splat(size * 0.8)),
        ),
        PartRole::Sensor | PartRole::Weapon => (
            Mark::Ring,
            Transform::from_translation(at)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(size * 1.1)),
        ),
        _ => (
            Mark::Ring,
            Transform::from_translation(at).with_scale(Vec3::splat(size * 1.1)),
        ),
    }
}

/// A dead part's mark goes out: it's no target now.
fn mark_out(mut marks: Query<(&WeakMark, &mut Visibility)>, parts: Query<&RobotPart>) {
    for (m, mut v) in &mut marks {
        let alive = parts.get(m.part).is_ok_and(|p| p.alive());
        let want = if alive {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What's weak is what a pistol round tells on: a walker's eye, knees
    /// and turret, not its core or its plates; a drone's rotors and eye,
    /// not its body; a heavy's bolt, plate or not.
    #[test]
    fn weak_is_what_a_round_tells_on() {
        assert!(is_weak(PartRole::Sensor, 25.0, false));
        assert!(is_weak(PartRole::Leg(0), 35.0, false));
        assert!(is_weak(PartRole::Weapon, 45.0, false));
        assert!(!is_weak(PartRole::Core, 130.0, false));
        assert!(!is_weak(PartRole::Armour(0), 40.0, false));
        assert!(is_weak(PartRole::Rotor(1), 14.0, false));
        assert!(!is_weak(PartRole::Core, 70.0, false));
        assert!(is_weak(PartRole::Armour(100), 200.0, true));
    }

    /// The size: [`WEAK_ACROSS`] holds a VR pistol's cone at 25 m, and
    /// twice it near enough at 15 m.
    #[test]
    fn a_weak_point_holds_a_pistol_at_fighting_range() {
        assert!(subtends(WEAK_ACROSS, FIGHT_RANGE) >= VR_HOLD_DEG);
        assert!(subtends(WEAK_ACROSS, 15.0) >= 1.6 * VR_HOLD_DEG);
        // The walker's knee before this: a 12 cm shin, half the cone at 25 m.
        assert!(subtends(0.12, FIGHT_RANGE) < VR_HOLD_DEG);
    }

    /// Growing: a leg gets a knee ball at its top, an eye a bigger box, a
    /// rotor a deeper puck; what's big enough is left alone.
    #[test]
    fn small_weak_points_grow() {
        let leg = Collider::capsule(0.06, 0.5);
        let (c, knee) = grow(&leg, WEAK_ACROSS).unwrap();
        assert!((knee.y - 0.25).abs() < 1e-4, "the knee is the top end");
        assert!(across(&c) >= WEAK_ACROSS - 1e-4);
        let eye = Collider::cuboid(0.3, 0.08, 0.06);
        assert!(across(&grow(&eye, WEAK_ACROSS).unwrap().0) >= WEAK_ACROSS - 1e-4);
        let rotor = Collider::cylinder(0.15, 0.04);
        assert!(across(&grow(&rotor, WEAK_ACROSS).unwrap().0) >= WEAK_ACROSS - 1e-4);
        assert!(grow(&Collider::sphere(0.4), WEAK_ACROSS).is_none());
    }

    /// The shoot-here colour reads as it, and the robots' own colours
    /// don't: their shell, dark metal, teal glow, the rotor blur, and the
    /// hazard paint once painted (`paint`).
    #[test]
    fn only_weak_points_wear_the_colour() {
        assert!(reads_as_shoot_here(SHOOT_HERE));
        for c in [
            Color::srgb(0.86, 0.87, 0.88),
            Color::srgb(0.08, 0.085, 0.09),
            Color::srgb(0.1, 0.4, 0.5),
            Color::srgb(0.58, 0.56, 0.53),
        ] {
            assert!(!reads_as_shoot_here(c), "{c:?}");
        }
        // The models' hazard paint (models.rs) reads as it, so robots
        // don't wear it.
        assert!(reads_as_shoot_here(Color::srgb(0.9, 0.6, 0.1)));
        assert!(reads_as_shoot_here(Color::srgb(0.9, 0.74, 0.16)));
        assert_eq!(paint("Warn"), "Glow");
        assert_eq!(paint("Yellow"), "Glow");
        assert_eq!(paint("Shell"), "Shell");
    }

    /// Every robot, as each is made: every weak point it has is at least
    /// [`WEAK_ACROSS`] across at its size (it holds a VR pistol's cone at
    /// 25 m) and wears a mark; its plates and strong parts don't. Prints
    /// what each subtends at 15 and 25 m.
    #[test]
    fn every_robots_weak_points_are_big_and_marked() {
        use super::super::{Job, JobKind, Robot, RobotKind};
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let job = |at: Vec3| Job {
            kind: JobKind::Survey,
            site: at,
            duration: 999.0,
            exit: at,
        };
        let spot = |k: f32| zone.on_ground(40.0 + 14.0 * k, 230.0);
        {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            use super::super::*;
            walker::spawn_walker(&mut c, &zone, spot(0.0), job(spot(0.0)));
            let at = spot(1.0);
            let crawler = walker::spawn_walker_sized(
                &mut c,
                &zone,
                at,
                job(at),
                false,
                indoor::CRAWLER_HP,
                indoor::CRAWLER_SIZE,
                None,
            );
            c.entity(crawler)
                .insert(Breed("Crawler"))
                .with_children(|k| {
                    k.spawn((
                        indoor::CrawlerGun,
                        part_bundle(
                            crawler,
                            PartRole::Weapon,
                            30.0,
                            Collider::sphere(0.08),
                            Transform::from_xyz(0.0, -0.1, -0.44),
                        ),
                    ));
                });
            hound::spawn_hound(&mut c, &zone, spot(2.0), job(spot(2.0)));
            grappler::spawn_grappler(&mut c, &zone, spot(3.0), job(spot(3.0)));
            heavy::spawn_heavy(&mut c, &zone, &catalog, spot(4.0), job(spot(4.0)));
            let up = Vec3::Y * 3.0;
            drone::spawn_drone(&mut c, spot(5.0) + up, job(spot(5.0)));
            mender::spawn_mender(&mut c, spot(6.0) + up, spot(6.0));
            let s = spot(7.0);
            sentry::spawn_sentry(
                &mut c,
                &zone,
                Vec2::new(s.x, s.z),
                0.0,
                1.0,
                Vec2::new(s.x, s.z + 5.0),
            );
            let s = spot(8.0);
            spotter::spawn_spotter(&mut c, &zone, Vec2::new(s.x, s.z));
            let (a, b) = (spot(9.0), spot(10.5));
            plough::spawn_plough(&mut c, &zone, vec![[a.x, a.z], [b.x, b.z]]);
            sorter::spawn_sorter(
                &mut c,
                Transform::from_translation(spot(12.0)),
                Vec2::new(12.0, 8.0),
                6.0,
                sorter::depot_spots(),
                2,
            );
            dirigible::spawn_dirigible(&mut c, spot(14.0) + Vec3::Y * 40.0, job(spot(14.0)));
            pylon::spawn_pylon(&mut c, spot(16.0), 300.0, spot(16.0));
            q.apply(app.world_mut());
        }
        for _ in 0..12 {
            app.update();
        }
        let marked: std::collections::HashSet<Entity> = app
            .world_mut()
            .query::<&WeakMark>()
            .iter(app.world())
            .map(|m| m.part)
            .collect();
        let mut kinds = std::collections::BTreeSet::new();
        let mut table = Vec::new();
        let mut q = app.world_mut().query::<(
            Entity,
            &RobotPart,
            &Collider,
            Has<crate::weapons::melee::WeakPoint>,
        )>();
        let parts: Vec<(Entity, PartRole, f32, Collider, bool, Entity)> = q
            .iter(app.world())
            .map(|(e, p, c, w)| (e, p.role, p.max_hp, c.clone(), w, p.robot))
            .collect();
        for (e, role, hp, c, w, robot) in parts {
            let world = app.world();
            let Some(r) = world.get::<Robot>(robot) else {
                continue;
            };
            let name = super::super::robot_name(r.kind, world.get::<super::super::Breed>(robot));
            let scale = world
                .get::<Transform>(robot)
                .map_or(1.0, |t| t.scale.max_element());
            let size = across(&c) * scale;
            if !is_weak(role, hp, w) {
                assert!(
                    !marked.contains(&e),
                    "{name} {role:?} isn't weak but is marked"
                );
                continue;
            }
            kinds.insert(name.clone());
            table.push(format!(
                "{name:>10} {role:<12?} {size:.2} m  {:.2} deg at 15 m, {:.2} at 25 m",
                subtends(size, 15.0),
                subtends(size, 25.0)
            ));
            assert!(
                size >= WEAK_ACROSS - 1e-3,
                "{name} {role:?} is {size:.3} m across: under {WEAK_ACROSS}"
            );
            assert!(marked.contains(&e), "{name} {role:?} is weak and unmarked");
        }
        table.sort();
        eprintln!("{}", table.join("\n"));
        // Every kind made has a weak point, or says why not.
        for k in [RobotKind::Walker, RobotKind::Drone] {
            assert!(kinds.contains(&format!("{k:?}")), "{k:?} has no weak point");
        }
    }
}
