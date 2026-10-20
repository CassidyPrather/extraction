//! How a gun handles in the hands: recoil, weight, sway.
//!
//! Everything here moves the gun, never the head: the result is a pose
//! offset and a follow lag ([`HoldFeel`]) that the hand-following applies
//! to the held gun's target, so the camera stays where the headset puts it.
//!
//! - **Recoil** is an impulse. Each shot gives the gun a kick of momentum
//!   (`FirearmDef::recoil`, per gun and cartridge) shared with whatever is
//!   holding it ([`Hold::mass`]): muzzle rise about the grip, a little
//!   random yaw, and a push back along the bore. A damped spring (the
//!   hands) pulls it back; two hands are a stiffer spring than one, and a
//!   stock against the shoulder or cheek stiffer and heavier still. Fast
//!   fire from one hand stacks up; from the shoulder it barely shows.
//! - **Weight**: heavier guns follow the controller softly ([`lag`]),
//!   less so with two hands on them.
//! - **Strain**: a long gun held out in one hand droops and sways, by its
//!   moment about the wrist; sprint fatigue adds a little sway to any gun.
//!
//! The tunables are the constants below; `docs/DESIGN.md` ("Weapon feel")
//! lists what each one does to the four guns.

use bevy::prelude::*;
use extraction_core::firearm::{ActionKind, Event as FireEvent, Malfunction};

use super::{Gun, GunEvent};
use crate::interact::{Held, HoldFeel, Supported};
use crate::items::{Catalog, Item};
use crate::player::Body;
use crate::rig::RigHead;

/// Muzzle rise per unit of recoil momentum over the moving mass, rad/s per
/// (N s / kg).
pub const KICK_RISE: f32 = 12.5;
/// Push back along the bore, m/s per (N s / kg).
pub const KICK_BACK: f32 = 1.0;
/// Random yaw as a share of the rise (either way).
pub const KICK_YAW: f32 = 0.3;
/// The hands' spring, natural frequency in rad/s: one hand, two hands,
/// shouldered. Higher recovers faster and rises less.
pub const RECOVER_ONE: f32 = 15.0;
pub const RECOVER_TWO: f32 = 28.0;
pub const RECOVER_SHOULDER: f32 = 35.0;
/// Damping ratio of that spring: a touch under critical, so the muzzle
/// settles without a wobble you'd notice.
pub const RECOVER_DAMPING: f32 = 0.8;
/// Mass the holder adds to the gun's, kg: a wrist, two arms, the body
/// behind a shouldered stock.
pub const BRACE_ONE: f32 = 0.5;
pub const BRACE_TWO: f32 = 1.5;
pub const BRACE_SHOULDER: f32 = 6.0;
/// Follow lag, seconds per kg of gun: one hand, two hands, shouldered.
pub const LAG_ONE: f32 = 0.012;
pub const LAG_TWO: f32 = 0.005;
pub const LAG_SHOULDER: f32 = 0.004;
/// One-handed droop, radians per kg m of moment about the wrist.
pub const DROOP: f32 = 0.1;
/// One-handed sway, radians: a floor, plus this much per kg m.
pub const SWAY_BASE: f32 = 0.0015;
pub const SWAY_MOMENT: f32 = 0.012;
/// Extra sway when out of breath (stamina empty), radians, any hold;
/// shared by a second hand and halved again by a shoulder.
pub const SWAY_TIRED: f32 = 0.006;
/// How close the stock's butt must be to the head to count as shouldered
/// (a cheek weld or the shoulder pocket), metres.
pub const SHOULDER_REACH: f32 = 0.3;

/// How a gun is being held this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hold {
    pub hands: u8,
    pub shouldered: bool,
}

impl Hold {
    /// The spring's natural frequency.
    pub fn recover(self) -> f32 {
        if self.shouldered {
            RECOVER_SHOULDER
        } else if self.hands >= 2 {
            RECOVER_TWO
        } else {
            RECOVER_ONE
        }
    }

    /// Mass moving with the gun under recoil.
    pub fn mass(self, gun: f32) -> f32 {
        gun + if self.shouldered {
            BRACE_SHOULDER
        } else if self.hands >= 2 {
            BRACE_TWO
        } else {
            BRACE_ONE
        }
    }

    /// Follow lag for a gun of `mass` kg.
    pub fn lag(self, mass: f32) -> f32 {
        mass * if self.shouldered {
            LAG_SHOULDER
        } else if self.hands >= 2 {
            LAG_TWO
        } else {
            LAG_ONE
        }
    }
}

/// A gun's handling state: the recoil spring and the strain on the hold.
#[derive(Component, Debug, Default)]
pub struct Handling {
    pub hold: Hold,
    /// Muzzle rise (x, up) and yaw (y, left), radians, and their rates.
    pub rise: Vec2,
    rise_rate: Vec2,
    /// Push back along the bore, metres, and its rate.
    pub push: f32,
    push_rate: f32,
    /// Droop, radians down, eased toward its target.
    pub droop: f32,
    clock: f32,
    /// A visual-only stroke of the bolt or slide after a self-loader
    /// fires, 1 at the shot down to 0.
    pub cycle: f32,
}

impl Handling {
    /// A shot's momentum, with a yaw share in -1..1.
    pub fn kick(&mut self, impulse: f32, gun_mass: f32, yaw: f32) {
        let v = impulse / self.hold.mass(gun_mass).max(0.1);
        self.rise_rate += Vec2::new(KICK_RISE * v, KICK_RISE * v * KICK_YAW * yaw);
        self.push_rate += KICK_BACK * v;
    }

    /// Integrates the spring for `dt` seconds.
    pub fn settle(&mut self, dt: f32) {
        let w = self.hold.recover();
        let c = 2.0 * RECOVER_DAMPING * w;
        // Semi-implicit Euler in small steps: stable for these stiffnesses.
        let n = (dt / 0.004).ceil().max(1.0) as usize;
        let h = dt / n as f32;
        for _ in 0..n {
            self.rise_rate += (-w * w * self.rise - c * self.rise_rate) * h;
            self.rise += self.rise_rate * h;
            self.push_rate += (-w * w * self.push - c * self.push_rate) * h;
            self.push += self.push_rate * h;
        }
    }
}

pub struct FeelPlugin;

impl Plugin for FeelPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(add_handling).add_systems(
            FixedUpdate,
            handle_guns
                .after(crate::interact::InteractSystems::Grab)
                .before(crate::interact::InteractSystems::Follow),
        );
    }
}

fn add_handling(add: On<Add, Gun>, mut commands: Commands) {
    commands
        .entity(add.entity)
        .insert((Handling::default(), HoldFeel::default()));
}

/// The rear of the stock (the middle of the butt plate), gun space, if the
/// gun has a stock: the back face of its parts' bounds.
pub fn butt(gun: &Gun) -> Option<Vec3> {
    gun.layout.support?;
    let (lo, hi) = gun.layout.bounds();
    Some(Vec3::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, hi.z))
}

/// Mass times the distance from the grip to the gun's middle: what a
/// wrist holds up.
fn moment(gun: &Gun, mass: f32) -> f32 {
    let (lo, hi) = gun.layout.bounds();
    let centre = (lo + hi) * 0.5;
    mass * (centre - gun.layout.grip.translation).length()
}

/// Works out each held gun's hold and writes its feel for this tick.
#[allow(clippy::type_complexity)]
fn handle_guns(
    mut guns: Query<(
        &Gun,
        &Item,
        &GlobalTransform,
        &mut Handling,
        &mut HoldFeel,
        Option<&Held>,
        Option<&Supported>,
    )>,
    heads: Query<(&ChildOf, &GlobalTransform), With<RigHead>>,
    bodies: Query<&Body>,
    catalog: Res<Catalog>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (gun, item, gt, mut handling, mut feel, held, support) in &mut guns {
        let Some(held) = held else {
            if handling.hold != Hold::default() || feel.smoothed.is_some() {
                *handling = Handling::default();
                *feel = HoldFeel::default();
            }
            continue;
        };
        let mass = catalog.get(item.kind()).mass;
        let head = heads
            .iter()
            .find(|(c, _)| c.parent() == held.player)
            .map(|(_, h)| h.translation());
        let shouldered = match (butt(gun), head) {
            (Some(b), Some(head)) => gt.transform_point(b).distance(head) < SHOULDER_REACH,
            _ => false,
        };
        handling.hold = Hold {
            hands: 1 + u8::from(support.is_some()),
            shouldered,
        };
        handling.settle(dt);
        handling.cycle = (handling.cycle - dt / 0.07).max(0.0);
        handling.clock += dt;
        let hold = handling.hold;
        // Strain: droop and sway, eased so changing grip doesn't snap.
        let one_handed = hold.hands < 2 && !hold.shouldered;
        let m = moment(gun, mass);
        let droop_to = if one_handed { DROOP * m } else { 0.0 };
        let ease = 1.0 - (-dt / 0.3).exp();
        handling.droop += (droop_to - handling.droop) * ease;
        let tired = bodies.get(held.player).map_or(0.0, |b| 1.0 - b.stamina);
        let share = if hold.shouldered {
            0.25
        } else if hold.hands >= 2 {
            0.5
        } else {
            1.0
        };
        let amp = if one_handed {
            SWAY_BASE + SWAY_MOMENT * m
        } else {
            0.0
        } + SWAY_TIRED * tired.clamp(0.0, 1.0) * share;
        let t = handling.clock;
        let sway = Vec2::new(
            (t * 1.9).sin() * 0.7 + (t * 0.83 + 1.3).sin() * 0.3,
            (t * 1.3 + 0.4).sin() * 0.6 + (t * 0.57).sin() * 0.4,
        ) * amp;
        // Rotate about the grip (the hand), then push back along the bore.
        let pitch = handling.rise.x + sway.x - handling.droop;
        let yaw = handling.rise.y + sway.y;
        let pivot = gun.layout.grip.translation;
        let r = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
        feel.offset = Transform::from_translation(pivot - r * pivot + Vec3::Z * handling.push)
            .with_rotation(r);
        feel.lag = hold.lag(mass);
    }
}

/// Shots kick the gun that fired them, and start the bolt's stroke.
pub fn kick_guns(
    mut events: MessageReader<GunEvent>,
    mut guns: Query<(&Gun, &Item, &mut Handling)>,
    catalog: Res<Catalog>,
    mut rng: ResMut<super::GunRng>,
) {
    for ev in events.read() {
        if !matches!(ev.event, FireEvent::Fired { .. }) {
            continue;
        }
        let Ok((gun, item, mut handling)) = guns.get_mut(ev.gun) else {
            continue;
        };
        let mass = catalog.get(item.kind()).mass;
        let yaw = rng.0.f32() * 2.0 - 1.0;
        handling.kick(gun.firearm.def.recoil, mass, yaw);
        if gun.firearm.def.action == ActionKind::SemiAuto {
            handling.cycle = 1.0;
        }
    }
}

/// What's drawn stuck in a jammed gun's port, shown while it has that
/// malfunction.
#[derive(Component)]
pub struct JamProp(pub Malfunction);

/// Case radius and length for a cartridge, metres.
pub(crate) fn case_size(cartridge: &str) -> (f32, f32) {
    match cartridge {
        "9x18" => (0.0049, 0.018),
        "5.45x39" => (0.005, 0.039),
        "7.62x39" => (0.0056, 0.039),
        "12ga" => (0.0105, 0.07),
        "7.62x38R" => (0.0049, 0.0385),
        "7.62x25" => (0.0049, 0.025),
        "7.62x54R" => (0.0062, 0.054),
        _ => (0.005, 0.03),
    }
}

/// Gives each self-loader its stuck case (a stovepipe: standing up out of
/// the port, mouth out) and its double feed (a case in the chamber with a
/// round nosed into it), and shows the one that's happening (client side).
pub fn jam_visuals(
    mut commands: Commands,
    added: Query<(Entity, &Gun), Added<Gun>>,
    guns: Query<&Gun>,
    mut props: Query<(&JamProp, &ChildOf, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mats: Local<Option<(Handle<StandardMaterial>, Handle<StandardMaterial>)>>,
) {
    let (brass, copper) = mats
        .get_or_insert_with(|| {
            let m = |r, g, b| StandardMaterial {
                base_color: Color::srgb(r, g, b),
                metallic: 1.0,
                perceptual_roughness: 0.35,
                ..default()
            };
            (
                materials.add(m(0.78, 0.6, 0.3)),
                materials.add(m(0.72, 0.4, 0.25)),
            )
        })
        .clone();
    for (e, gun) in &added {
        if gun.firearm.def.action != ActionKind::SemiAuto {
            continue;
        }
        let (r, len) = case_size(&gun.firearm.def.cartridge);
        let case = meshes.add(Cylinder::new(r, len));
        let bullet = meshes.add(Cylinder::new(r * 0.75, len * 0.45));
        let port = gun.layout.port;
        let out = gun.layout.eject.normalize_or(Vec3::X);
        // Stovepipe: caught by the slide, standing up and out of the port.
        let axis = (out + Vec3::Y * 1.2).normalize();
        commands.spawn((
            Name::new("Stovepipe case"),
            JamProp(Malfunction::FailureToEject),
            Mesh3d(case.clone()),
            MeshMaterial3d(brass.clone()),
            Transform::from_translation(port + axis * len * 0.3)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, axis)),
            Visibility::Hidden,
            ChildOf(e),
        ));
        // Double feed: the case still in the chamber's mouth, the next
        // round tipped up into it from below.
        // Set into the port's opening, so it shows past the cover or slide.
        let bore = Vec3::new(port.x * 0.6, port.y + r, port.z);
        let pair = commands
            .spawn((
                Name::new("Double feed"),
                JamProp(Malfunction::DoubleFeed),
                Transform::from_translation(bore),
                Visibility::Hidden,
                ChildOf(e),
            ))
            .id();
        commands.spawn((
            Mesh3d(case.clone()),
            MeshMaterial3d(brass.clone()),
            Transform::from_xyz(0.0, 0.0, -len * 0.3)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, Vec3::NEG_Z)),
            ChildOf(pair),
        ));
        let tip = Quat::from_rotation_x(0.6) * Vec3::NEG_Z;
        let round = commands
            .spawn((
                Mesh3d(case),
                MeshMaterial3d(brass.clone()),
                Transform::from_xyz(0.0, -r * 0.5, len * 0.5)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, tip)),
                ChildOf(pair),
            ))
            .id();
        commands.spawn((
            Mesh3d(bullet),
            MeshMaterial3d(copper.clone()),
            Transform::from_xyz(0.0, len * 0.65, 0.0),
            ChildOf(round),
        ));
    }
    for (prop, parent, mut vis) in &mut props {
        let on = guns
            .get(parent.parent())
            .is_ok_and(|g| g.firearm.malfunction == Some(prop.0));
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Peak muzzle rise of one shot, radians.
    fn peak(hold: Hold, impulse: f32, mass: f32) -> f32 {
        let mut h = Handling { hold, ..default() };
        h.kick(impulse, mass, 0.0);
        let mut best = 0.0f32;
        for _ in 0..90 {
            h.settle(1.0 / 90.0);
            best = best.max(h.rise.x);
        }
        assert!(h.rise.x.abs() < 0.01, "settles within a second");
        best
    }

    #[test]
    fn the_hold_tames_the_kick() {
        let one = Hold {
            hands: 1,
            shouldered: false,
        };
        let two = Hold {
            hands: 2,
            shouldered: false,
        };
        let shoulder = Hold {
            hands: 2,
            shouldered: true,
        };
        // The AK: 2.2 N s, 3.3 kg.
        let (a, b, c) = (
            peak(one, 2.2, 3.3),
            peak(two, 2.2, 3.3),
            peak(shoulder, 2.2, 3.3),
        );
        assert!(a > b * 2.0 && b > c * 2.0, "{a} {b} {c}");
        // Roughly: 10 degrees, 4, under 2.
        assert!((0.12..0.25).contains(&a), "{a}");
        assert!(c < 0.035, "{c}");
        // A heavier gun of the same kick rises less.
        assert!(peak(one, 2.2, 6.0) < a);
    }
}

/// Through the headset path: poses, grip and trigger into `RigInput`.
#[cfg(test)]
mod vr_tests {
    use super::*;
    use crate::interact::GripRole;
    use crate::rig::Side;
    use crate::vr_harness::Vr;
    use crate::weapons::ballistics::Projectile;
    use extraction_core::firearm::{FireMode, Magazine};
    use std::collections::HashSet;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);
    const EYE: Vec3 = Vec3::new(0.0, 1.65, 0.0);

    struct Range {
        vr: Vr,
        gun: Entity,
        feet: Vec3,
    }

    /// Stand in the open with `id` in the right hand, held out in front
    /// at chest height (or with the stock to the cheek), the left hand on
    /// the handguard if `two`.
    fn hold(id: &str, two: bool, shoulder: bool) -> Range {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.head(EYE);
        let gun = vr.spawn(
            id,
            Transform::from_translation(feet + Vec3::new(0.15, 1.2, -0.45)),
        );
        vr.app
            .world_mut()
            .entity_mut(gun)
            .insert(avian3d::prelude::RigidBody::Static);
        let grip = vr.point(gun, |r| *r == GripRole::Primary);
        // Turn the hand by the grip's rake, as a wrist does, so the bore
        // starts level.
        let rake = vr.app.world().get::<Gun>(gun).unwrap().layout.grip.rotation;
        vr.hand(Side::Right, grip, rake, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(gun)
            .insert(avian3d::prelude::RigidBody::Dynamic);
        vr.step(20);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(gun));
        if shoulder {
            // Bring the whole gun back until the butt sits under the eye.
            let b = butt(vr.app.world().get::<Gun>(gun).unwrap()).unwrap();
            let gt = *vr.app.world().get::<GlobalTransform>(gun).unwrap();
            let want = feet + EYE + Vec3::new(0.0, -0.12, 0.08);
            let shift = want - gt.transform_point(b);
            vr.move_hand(Side::Right, grip + shift);
            vr.step(30);
        }
        if two {
            let support = vr.point(gun, |r| *r == GripRole::Support);
            vr.hand(Side::Left, support, Quat::IDENTITY, 1.0, 0.0);
            vr.step(20);
            assert!(vr.hand_state(Side::Left).supporting == Some(gun));
        }
        Range { vr, gun, feet }
    }

    fn gun_mut(r: &mut Range) -> Mut<'_, Gun> {
        r.vr.app.world_mut().get_mut::<Gun>(r.gun).unwrap()
    }

    /// A full magazine in the mechanism, a round chambered, on `mode`.
    fn load(r: &mut Range, family: &str, rounds: u32, mode: FireMode) {
        let mut g = gun_mut(r);
        let f = &mut g.firearm;
        f.insert_magazine(Magazine {
            family: family.into(),
            capacity: rounds,
            rounds,
        })
        .unwrap();
        f.set_bolt_travel(1.0);
        f.release_bolt();
        while f.fire_mode() != mode {
            f.cycle_mode();
        }
        f.condition = 1.0;
        f.fouling = 0.0;
    }

    /// Holds the trigger until `n` shots are out (or a few seconds pass),
    /// returning each projectile's direction as it left.
    fn fire(r: &mut Range, n: usize, pulse: bool) -> Vec<Vec3> {
        // Rounds already in the air aren't this burst's.
        let mut seen: HashSet<Entity> = {
            let mut q =
                r.vr.app
                    .world_mut()
                    .query_filtered::<Entity, With<Projectile>>();
            q.iter(r.vr.app.world()).collect()
        };
        let mut dirs = Vec::new();
        for i in 0..400 {
            let down = !pulse || (i / 6) % 2 == 0;
            r.vr.trigger(Side::Right, if down { 1.0 } else { 0.0 });
            r.vr.step(1);
            let mut q = r.vr.app.world_mut().query::<(Entity, &Projectile)>();
            for (e, p) in q.iter(r.vr.app.world()) {
                if seen.insert(e) {
                    dirs.push(p.velocity.normalize());
                }
            }
            if dirs.len() >= n {
                break;
            }
        }
        r.vr.trigger(Side::Right, 0.0);
        r.vr.step(2);
        dirs
    }

    /// RMS angle of the shots from their mean: the group's size.
    fn group(dirs: &[Vec3]) -> f32 {
        let mean = dirs.iter().copied().sum::<Vec3>().normalize();
        (dirs
            .iter()
            .map(|d| d.angle_between(mean).powi(2))
            .sum::<f32>()
            / dirs.len() as f32)
            .sqrt()
    }

    /// Ten rounds of automatic from an AK at the same still hands: held
    /// out in one hand they climb and scatter; with the second hand on
    /// the handguard the group is far tighter; to the shoulder, tighter
    /// still.
    #[test]
    fn two_hands_group_tighter_than_one_and_the_shoulder_tighter_still() {
        let mut sizes = Vec::new();
        for (two, shoulder) in [(false, false), (true, false), (true, true)] {
            let mut r = hold("ak74", two, shoulder);
            load(&mut r, "ak545", 30, FireMode::Auto);
            let dirs = fire(&mut r, 10, false);
            assert_eq!(dirs.len(), 10);
            let hold = r.vr.app.world().get::<Handling>(r.gun).unwrap().hold;
            assert_eq!(hold.shouldered, shoulder, "{hold:?}");
            sizes.push(group(&dirs).to_degrees());
        }
        let [one, two, shoulder] = sizes[..] else {
            unreachable!()
        };
        assert!(two < one * 0.6, "one hand {one:.2} deg, two {two:.2}");
        assert!(
            shoulder < two * 0.8,
            "two hands {two:.2} deg, shoulder {shoulder:.2}"
        );
    }

    /// Kick after kick from a shouldered Mosin and a double: the gun
    /// moves, the head (the camera) and the player never do.
    #[test]
    fn recoil_moves_the_gun_never_the_head() {
        for (id, mode) in [("ak74", FireMode::Semi), ("pm_pistol", FireMode::Semi)] {
            let mut r = hold(id, id == "ak74", id == "ak74");
            let family = if id == "ak74" { "ak545" } else { "pm" };
            load(&mut r, family, 8, mode);
            let head = |r: &mut Range| {
                let mut q =
                    r.vr.app
                        .world_mut()
                        .query_filtered::<&GlobalTransform, With<RigHead>>();
                q.iter(r.vr.app.world()).next().unwrap().compute_transform()
            };
            let root = |r: &Range| *r.vr.app.world().get::<Transform>(r.vr.player).unwrap();
            let (h0, p0) = (head(&mut r), root(&r));
            let mut rose = 0.0f32;
            let mut q0 = None;
            for _ in 0..6 {
                r.vr.trigger(Side::Right, 1.0);
                r.vr.step(1);
                for _ in 0..12 {
                    r.vr.step(1);
                    let g = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
                    let q = g.rotation();
                    let q0 = *q0.get_or_insert(q);
                    rose = rose.max(q.angle_between(q0));
                }
                r.vr.trigger(Side::Right, 0.0);
                r.vr.step(12);
            }
            let (h1, p1) = (head(&mut r), root(&r));
            assert!(rose > 0.005, "{id}: the gun kicked ({rose})");
            assert!(
                h0.translation.distance(h1.translation) < 1e-4
                    && h0.rotation.angle_between(h1.rotation) < 1e-4,
                "{id}: the head moved"
            );
            assert!(
                p0.translation.distance(p1.translation) < 1e-3,
                "{id}: the player moved"
            );
            let _ = r.feet;
        }
    }

    /// Grabs `gun`'s handle with the left trigger and pulls it through its
    /// travel and lets it go.
    fn rack(r: &mut Range) {
        let slide = r.vr.point(r.gun, |p| matches!(p, GripRole::Handle(_)));
        r.vr.hand(Side::Left, slide, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(2);
        r.vr.trigger(Side::Left, 1.0);
        r.vr.step(2);
        assert!(
            r.vr.hand_state(Side::Left).handle.is_some(),
            "took the handle"
        );
        let travel = gun_mut(r).layout.handle.as_ref().unwrap().travel;
        let back =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .back();
        for i in 1..=12 {
            r.vr.move_hand(Side::Left, slide + back * (travel * 1.15 * i as f32 / 12.0));
            r.vr.step(1);
        }
        r.vr.trigger(Side::Left, 0.0);
        r.vr.hand(
            Side::Left,
            slide + Vec3::new(-0.3, -0.2, 0.0),
            Quat::IDENTITY,
            0.0,
            0.0,
        );
        r.vr.step(4);
    }

    /// A stovepipe on the pistol: the trigger does nothing until the slide
    /// is racked, then it fires.
    #[test]
    fn a_stovepipe_stops_the_gun_until_racked() {
        let mut r = hold("pm_pistol", false, false);
        load(&mut r, "pm", 8, FireMode::Semi);
        {
            let mut g = gun_mut(&mut r);
            g.firearm.malfunction = Some(Malfunction::FailureToEject);
            g.firearm.bolt_travel = 0.2;
        }
        assert!(fire(&mut r, 1, true).is_empty(), "jammed: no shot");
        rack(&mut r);
        assert!(gun_mut(&mut r).firearm.malfunction.is_none());
        assert_eq!(fire(&mut r, 1, true).len(), 1, "cleared: it fires");
    }

    /// A real magazine of `id` with `rounds`, seated in the gun's well.
    fn seat_mag(r: &mut Range, id: &str, rounds: u32) -> Entity {
        let mag = r.vr.spawn(
            id,
            Transform::from_translation(r.feet + Vec3::new(-0.3, 1.0, -0.3)),
        );
        let well = gun_mut(r).magwell.unwrap();
        r.vr.app.world_mut().get_mut::<Item>(mag).unwrap().rounds = rounds;
        r.vr.app.world_mut().commands().queue(move |w: &mut World| {
            let mut socket = w.get_mut::<crate::interact::Socket>(well).unwrap().clone();
            {
                let mut commands = w.commands();
                crate::interact::seat_in_socket(&mut commands, mag, well, &mut socket);
            }
            *w.get_mut::<crate::interact::Socket>(well).unwrap() = socket;
            w.write_message(crate::interact::InteractEvent::Socketed {
                item: mag,
                socket: well,
            });
        });
        r.vr.step(4);
        assert!(gun_mut(r).firearm.magazine.is_some(), "seated");
        mag
    }

    /// Takes `id` from the air into the left hand.
    fn take(r: &mut Range, id: &str, rounds: u32) -> Entity {
        let at = r.feet + Vec3::new(-0.35, 1.0, -0.3);
        let e = r.vr.spawn(id, Transform::from_translation(at));
        r.vr.app.world_mut().get_mut::<Item>(e).unwrap().rounds = rounds;
        r.vr.app
            .world_mut()
            .entity_mut(e)
            .insert(avian3d::prelude::RigidBody::Static);
        r.vr.hand(Side::Left, at, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(2);
        r.vr.grip(Side::Left, 1.0);
        r.vr.step(3);
        r.vr.app
            .world_mut()
            .entity_mut(e)
            .insert(avian3d::prelude::RigidBody::Dynamic);
        assert_eq!(r.vr.hand_state(Side::Left).holding, Some(e), "took {id}");
        e
    }

    /// The gun's load port and its up, in the world.
    fn load_port(r: &mut Range) -> (Vec3, Vec3) {
        let port = gun_mut(r).layout.load_port;
        let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
        (gt.transform_point(port), gt.up().as_vec3())
    }

    /// Glides the left hand from `a` to `b`.
    fn glide(r: &mut Range, a: Vec3, b: Vec3, steps: usize) {
        for i in 0..=steps {
            r.vr.move_hand(Side::Left, a.lerp(b, i as f32 / steps as f32));
            r.vr.step(1);
        }
    }

    /// A Mosin with its bolt open: a clip set in the guide and pushed down
    /// strips five rounds into the magazine.
    #[test]
    fn a_clip_pushed_down_the_guide_loads_the_mosin() {
        let mut r = hold("mosin", false, false);
        {
            let mut g = gun_mut(&mut r);
            g.firearm.set_handle_lift(1.0);
            g.firearm.set_bolt_travel(1.0);
        }
        let clip = take(&mut r, "mosin_clip", 5);
        let (port, up) = load_port(&mut r);
        let away = r.vr.hand_state(Side::Left).world.translation;
        glide(&mut r, away, port + up * 0.05, 15);
        assert_eq!(gun_mut(&mut r).firearm.internal_rounds, 0, "not yet pushed");
        glide(&mut r, port + up * 0.05, port + up * 0.015, 8);
        r.vr.step(2);
        assert_eq!(gun_mut(&mut r).firearm.internal_rounds, 5, "stripped");
        assert_eq!(r.vr.app.world().get::<Item>(clip).unwrap().rounds, 0);
        // Close the bolt: a round goes up the spout.
        {
            let mut g = gun_mut(&mut r);
            g.firearm.set_bolt_travel(0.0);
            g.firearm.set_handle_lift(0.0);
        }
        assert_eq!(gun_mut(&mut r).firearm.live_rounds(), 5);
    }

    /// The double: thumb the lever to break it, bring a shell to each
    /// barrel, thumb it shut: two in.
    #[test]
    fn shells_go_into_the_open_double_by_hand() {
        let mut r = hold("toz_double", false, false);
        r.vr.primary(Side::Right, true);
        r.vr.step(2);
        r.vr.primary(Side::Right, false);
        r.vr.step(2);
        assert!(gun_mut(&mut r).firearm.bolt_travel > 0.5, "broken open");
        for n in 1..=2 {
            let shell = take(&mut r, "shell_12ga", 1);
            let (port, _) = load_port(&mut r);
            let away = r.vr.hand_state(Side::Left).world.translation;
            glide(&mut r, away, port, 15);
            r.vr.step(2);
            let loaded = gun_mut(&mut r)
                .firearm
                .chambers
                .iter()
                .filter(|c| c.is_some())
                .count();
            assert_eq!(loaded, n, "shell {n} in");
            assert!(
                r.vr.app.world().get_entity(shell).is_err(),
                "out of the hand"
            );
            assert_eq!(r.vr.hand_state(Side::Left).holding, None);
            r.vr.grip(Side::Left, 0.0);
            r.vr.step(2);
        }
        r.vr.primary(Side::Right, true);
        r.vr.step(2);
        r.vr.primary(Side::Right, false);
        r.vr.step(2);
        assert!(gun_mut(&mut r).firearm.is_closed());
        assert_eq!(gun_mut(&mut r).firearm.live_rounds(), 2);
    }

    /// The SKS by hand: rack it empty and it holds open; a clip pushed down
    /// the guide fills it; a tug on the handle runs it home on a round; ten
    /// shots and it holds open again; a loose round thumbed in goes in.
    #[test]
    fn the_sks_by_hand_clip_shots_hold_open_and_a_loose_round() {
        let mut r = hold("sks", false, false);
        {
            let mut g = gun_mut(&mut r);
            let f = &mut g.firearm;
            while f.fire_mode() != FireMode::Semi {
                f.cycle_mode();
            }
            f.condition = 1.0;
            f.fouling = 0.0;
        }
        rack(&mut r);
        assert!(gun_mut(&mut r).firearm.locked_back, "held open, empty");
        take(&mut r, "sks_clip", 10);
        let (port, up) = load_port(&mut r);
        let away = r.vr.hand_state(Side::Left).world.translation;
        glide(&mut r, away, port + up * 0.05, 15);
        glide(&mut r, port + up * 0.05, port + up * 0.015, 8);
        r.vr.step(2);
        assert_eq!(gun_mut(&mut r).firearm.internal_rounds, 10, "stripped");
        r.vr.grip(Side::Left, 0.0);
        r.vr.step(2);
        rack(&mut r);
        assert_eq!(
            gun_mut(&mut r).firearm.chambers[0],
            Some(extraction_core::firearm::Round::Live),
            "a tug off the stop runs it home on a round"
        );
        let n = fire(&mut r, 10, true).len();
        assert_eq!(n, 10);
        assert!(
            gun_mut(&mut r).firearm.locked_back,
            "holds open on the last"
        );
        take(&mut r, "round_762x39", 1);
        let away = r.vr.hand_state(Side::Left).world.translation;
        let (port, _) = load_port(&mut r);
        glide(&mut r, away, port, 15);
        r.vr.step(2);
        assert_eq!(gun_mut(&mut r).firearm.internal_rounds, 1, "thumbed in");
    }

    /// The AK by hand: a magazine brought up into the well seats; rack;
    /// it fires.
    #[test]
    fn an_ak_magazine_goes_in_by_hand() {
        let mut r = hold("ak74", false, false);
        let mag = take(&mut r, "ak_mag", 30);
        let well = {
            let g = gun_mut(&mut r);
            let w = g.magwell.unwrap();
            r.vr.app
                .world()
                .get::<GlobalTransform>(w)
                .unwrap()
                .translation()
        };
        let away = r.vr.hand_state(Side::Left).world.translation;
        glide(&mut r, away, well - Vec3::Y * 0.25, 15);
        glide(&mut r, well - Vec3::Y * 0.25, well, 30);
        r.vr.step(5);
        r.vr.grip(Side::Left, 0.0);
        r.vr.step(5);
        assert!(
            r.vr.app
                .world()
                .get::<crate::interact::InSocket>(mag)
                .is_some(),
            "seated"
        );
        {
            let mut g = gun_mut(&mut r);
            let f = &mut g.firearm;
            while f.fire_mode() != FireMode::Semi {
                f.cycle_mode();
            }
        }
        rack(&mut r);
        assert_eq!(fire(&mut r, 1, true).len(), 1);
    }

    /// A gun-hand button for one press (the headset's B is `secondary`).
    fn press(r: &mut Range, which: &str) {
        for down in [true, false] {
            {
                let mut input =
                    r.vr.app
                        .world_mut()
                        .get_mut::<crate::rig::RigInput>(r.vr.player)
                        .unwrap();
                let h = input.hand_mut(Side::Right);
                match which {
                    "a" => h.primary = down,
                    "b" => h.secondary = down,
                    _ => h.stick_click = down,
                }
            }
            r.vr.step(2);
        }
    }

    /// The Nagant by hand: A swings the gate open; a round to the gate
    /// goes in and B turns the cylinder, seven times; A shuts it; seven
    /// double-action shots; open again, and the rod (stick click) and B
    /// clear the seven cases.
    #[test]
    fn the_nagant_loads_at_the_gate_fires_and_clears_by_hand() {
        let mut r = hold("nagant", false, false);
        press(&mut r, "a");
        assert!(gun_mut(&mut r).firearm.gate_open, "A opens the gate");
        for n in 1..=7 {
            take(&mut r, "round_762x38r", 1);
            let (port, _) = load_port(&mut r);
            let away = r.vr.hand_state(Side::Left).world.translation;
            glide(&mut r, away, port, 12);
            r.vr.step(2);
            r.vr.grip(Side::Left, 0.0);
            r.vr.step(2);
            assert_eq!(gun_mut(&mut r).firearm.live_rounds(), n, "round {n} in");
            press(&mut r, "b");
        }
        press(&mut r, "a");
        assert!(!gun_mut(&mut r).firearm.gate_open);
        assert_eq!(fire(&mut r, 7, true).len(), 7, "seven double-action shots");
        assert!(fire(&mut r, 1, true).is_empty(), "then clicks");
        press(&mut r, "a");
        let cases = |r: &mut Range| {
            let mut q = r.vr.app.world_mut().query::<&crate::weapons::Casing>();
            q.iter(r.vr.app.world()).count()
        };
        let before = cases(&mut r);
        for _ in 0..7 {
            press(&mut r, "stick");
            press(&mut r, "b");
        }
        r.vr.step(3);
        assert_eq!(cases(&mut r) - before, 7, "the rod pushed out seven cases");
        assert!(gun_mut(&mut r).firearm.chambers.iter().all(|c| c.is_none()));
    }

    /// The PPSh by hand: a drum in, the bolt racked back onto the sear; a
    /// second of trigger on auto is about fifteen rounds, and it ends back
    /// on the sear.
    #[test]
    fn the_ppsh_racks_onto_its_sear_and_runs_on_auto() {
        let mut r = hold("ppsh", true, true);
        seat_mag(&mut r, "ppsh_drum", 71);
        {
            let mut g = gun_mut(&mut r);
            g.firearm.condition = 1.0;
            g.firearm.fouling = 0.0;
        }
        // Off the support to rack, then back on it.
        r.vr.grip(Side::Left, 0.0);
        r.vr.step(3);
        rack(&mut r);
        assert!(gun_mut(&mut r).firearm.locked_back, "on the sear");
        assert_eq!(
            gun_mut(&mut r).firearm.live_rounds(),
            71,
            "nothing chambered"
        );
        while gun_mut(&mut r).firearm.fire_mode() != FireMode::Auto {
            press(&mut r, "stick");
        }
        // Hold the trigger a second (90 ticks).
        let shots = {
            let mut seen: HashSet<Entity> = HashSet::new();
            r.vr.trigger(Side::Right, 1.0);
            for _ in 0..90 {
                r.vr.step(1);
                let mut q =
                    r.vr.app
                        .world_mut()
                        .query_filtered::<Entity, With<Projectile>>();
                seen.extend(q.iter(r.vr.app.world()));
            }
            r.vr.trigger(Side::Right, 0.0);
            r.vr.step(3);
            seen.len()
        };
        assert!((13..=16).contains(&shots), "{shots} rounds in a second");
        assert!(gun_mut(&mut r).firearm.locked_back, "back on the sear");
        assert_eq!(
            gun_mut(&mut r).firearm.live_rounds() as usize,
            71 - shots,
            "every round fired, none lost"
        );
    }

    /// A double feed on the pistol: racking does nothing with the magazine
    /// in; thumb the mag release, rack, and the pair falls out; a fresh
    /// magazine and a rack, and it fires.
    #[test]
    fn a_double_feed_clears_with_the_magazine_out() {
        let mut r = hold("pm_pistol", false, false);
        seat_mag(&mut r, "pm_mag", 8);
        {
            let mut g = gun_mut(&mut r);
            let f = &mut g.firearm;
            while f.fire_mode() != FireMode::Semi {
                f.cycle_mode();
            }
        }
        rack(&mut r);
        // Wear it out and let the next shot jam as a double feed.
        {
            let mut g = gun_mut(&mut r);
            g.firearm.condition = 0.0;
            g.firearm.fouling = 1.0;
        }
        let mut tries = 0;
        while gun_mut(&mut r).firearm.malfunction != Some(Malfunction::DoubleFeed) {
            tries += 1;
            assert!(tries < 400, "never double-fed");
            let jammed = gun_mut(&mut r).firearm.malfunction.is_some();
            let empty = gun_mut(&mut r).firearm.rounds_in_feed() < 2;
            if jammed || empty {
                // Not the one we want: start over with a full magazine.
                let mut g = gun_mut(&mut r);
                let f = &mut g.firearm;
                f.malfunction = None;
                f.mag_unseated = false;
                f.locked_back = false;
                f.bolt_travel = 0.0;
                f.chambers[0] = Some(extraction_core::firearm::Round::Live);
                f.hammers_cocked[0] = true;
                if let Some(m) = &mut f.magazine {
                    m.rounds = 8;
                }
            }
            fire(&mut r, 1, true);
        }
        let ejected = |r: &mut Range| {
            let mut q = r.vr.app.world_mut().query::<&crate::weapons::Casing>();
            q.iter(r.vr.app.world()).count()
        };
        let cases = ejected(&mut r);
        rack(&mut r);
        assert_eq!(
            gun_mut(&mut r).firearm.malfunction,
            Some(Malfunction::DoubleFeed),
            "the magazine holds them"
        );
        assert_eq!(ejected(&mut r), cases, "nothing came out");
        // Mag release: the thumb button on the gun hand.
        r.vr.primary(Side::Right, true);
        r.vr.step(3);
        r.vr.primary(Side::Right, false);
        r.vr.step(3);
        assert!(gun_mut(&mut r).firearm.magazine.is_none(), "dropped");
        rack(&mut r);
        assert!(gun_mut(&mut r).firearm.malfunction.is_none(), "cleared");
        assert!(ejected(&mut r) > cases, "the case flew");
        seat_mag(&mut r, "pm_mag", 8);
        gun_mut(&mut r).firearm.condition = 1.0;
        gun_mut(&mut r).firearm.fouling = 0.0;
        rack(&mut r);
        assert_eq!(fire(&mut r, 1, true).len(), 1, "back in the fight");
    }

    /// A failure to feed on the AK: it clicks; racking alone doesn't help;
    /// a palm slapped up on the magazine, then a rack, does.
    #[test]
    fn a_failure_to_feed_wants_tap_rack() {
        let mut r = hold("ak74", false, false);
        let mag = seat_mag(&mut r, "ak_mag", 30);
        {
            let mut g = gun_mut(&mut r);
            let f = &mut g.firearm;
            while f.fire_mode() != FireMode::Semi {
                f.cycle_mode();
            }
            // As the jam leaves it: empty chamber, the magazine slipped.
            f.malfunction = Some(Malfunction::FailureToFeed);
            f.mag_unseated = true;
            f.hammers_cocked[0] = true;
        }
        assert!(fire(&mut r, 1, true).is_empty(), "click");
        rack(&mut r);
        assert!(
            fire(&mut r, 1, true).is_empty(),
            "racking alone feeds nothing"
        );
        // The tap: the left palm comes up hard under the floorplate.
        let floor = {
            let w = r.vr.app.world();
            let gt = w.get::<GlobalTransform>(mag).unwrap();
            gt.translation()
                - gt.rotation() * Vec3::Y * w.get::<crate::weapons::Mag>(mag).unwrap().height * 0.5
        };
        for i in 0..=8 {
            let at = floor - Vec3::Y * (0.12 - 0.015 * i as f32);
            r.vr.hand(Side::Left, at, Quat::IDENTITY, 0.0, 0.0);
            r.vr.step(1);
        }
        r.vr.step(2);
        assert!(!gun_mut(&mut r).firearm.mag_unseated, "the tap seated it");
        rack(&mut r);
        assert_eq!(fire(&mut r, 1, true).len(), 1, "tap, rack, bang");
    }
}

#[cfg(test)]
mod bot_pose {
    use super::*;

    /// Where the bot puts each long gun when it aims (its sight 32 cm in
    /// front of the eye, `bot::drive`), the stock is at the head: bots
    /// fight shouldered, as a competent player would, so recoil and strain
    /// don't skew the sims.
    #[test]
    fn bots_aim_from_the_shoulder() {
        for id in ["ak74", "mosin", "toz_double", "sks"] {
            let layout = crate::weapons::layout::layout(id).unwrap();
            let sight = layout.grip.compute_affine().inverse() * layout.sight.compute_affine();
            let hand = Transform::from_xyz(0.0, 0.0, -0.32).compute_affine() * sight.inverse();
            let gun = hand * layout.grip.compute_affine().inverse();
            let (lo, hi) = layout.bounds();
            let butt = Vec3::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, hi.z);
            let d = gun.transform_point3(butt).length();
            assert!(d < SHOULDER_REACH * 0.6, "{id}: butt {d:.3} m from the eye");
        }
    }
}
