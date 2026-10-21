//! Zeroing a scope: the PSO-1's (and the PU's) elevation turret, turned by
//! hand, sets the range the sight is zeroed for, 100 m a click. The zero
//! is the angle the bore must hold above the line of sight for a round to
//! cross it at that range, from the game's own ballistics
//! ([`super::ballistics::zero_angle`]): the gun's muzzle velocity and the
//! scope's height over the bore. Put the reticle on a thing at the set
//! range and the round meets it; nearer it strikes high, farther low, so a
//! 400 m robot under a 300 m zero wants a little holdover.
//!
//! - The turret is worked like a gun's handle: the other hand's trigger on
//!   the drum takes it ([`TURRET_HANDLE`] on the gun the scope sits on),
//!   and turning the wrist about the drum's axis clicks it, clockwise seen
//!   from above to add range. Each click ticks in the hand.
//! - The scope's lens looks down by the zero angle ([`aim_lenses`], client
//!   side), so the reticle's centre is where the round goes at the set
//!   range.
//! - The PSO's reticle carries the rangefinder: a 1.7 m man fits between
//!   its baseline and the curve over the range he's at
//!   ([`pso_reticle`]).

use std::collections::HashMap;

use bevy::prelude::*;

use super::Gun;
use crate::gadgets::{MountedOn, Optic};
use crate::interact::{GrabPoint, Grabbable, GripRole, Hand};
use crate::items::{Catalog, Item};

/// The gun's handle id for a mounted scope's turret.
pub const TURRET_HANDLE: u8 = 7;
/// The wrist's turn per click.
pub const CLICK: f32 = 0.5;
/// Metres per click.
pub const CLICK_M: u32 = 100;
/// Where the elevation drums sit, in their scopes' space (the models'
/// `drum` nodes turn about their own +Y there).
pub const PSO_DRUM: Vec3 = Vec3::new(0.0, 0.0235, -0.01);
pub const PU_DRUM: Vec3 = Vec3::new(0.0, 0.01825, -0.012);
/// How close a hand must come to take the drum.
pub const DRUM_REACH: f32 = 0.04;
/// The height a man is taken to be, for the rangefinder.
pub const STADIA_M: f32 = 1.7;

/// A scope's elevation turret: its setting in clicks, and its stops.
#[derive(Component, Clone, Copy, Debug)]
pub struct Turret {
    pub setting: u32,
    pub min: u32,
    pub max: u32,
    /// Where its drum is, in the scope's space.
    pub drum: Vec3,
}

impl Turret {
    pub fn range_m(&self) -> f32 {
        (self.setting * CLICK_M) as f32
    }
}

/// What a scope is zeroed for: the range, the angle its line of sight
/// looks down from the bore, and so where its lens looks in the scope's
/// own frame (it may sit a hair off true on its mount; the zero takes that
/// out, as zeroing a real one does), for the gun it's on.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Zero {
    pub range_m: f32,
    pub angle: f32,
    pub look: Quat,
    pub gun: Option<Entity>,
}

impl Default for Zero {
    fn default() -> Self {
        Self {
            range_m: 0.0,
            angle: 0.0,
            look: Quat::IDENTITY,
            gun: None,
        }
    }
}

/// A reticle with the PSO's rangefinder.
#[derive(Component)]
pub struct Rangefinder;

/// A turret clicked (for its sound).
#[derive(Message, Clone, Copy, Debug)]
pub struct TurretClicked {
    pub scope: Entity,
    pub setting: u32,
    pub at: Vec3,
}

pub struct ZeroPlugin;

impl Plugin for ZeroPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TurretClicked>()
            .add_observer(fit_turrets)
            .add_systems(
                Update,
                (
                    turret_points,
                    turn_turrets,
                    flat_drum.run_if(resource_exists::<ButtonInput<KeyCode>>),
                    zero_scopes,
                )
                    .chain(),
            );
    }
}

/// Scopes with elevation drums: the PSO-1 (1 to 10, and the rangefinder)
/// and the PU (1 to 13).
fn fit_turrets(
    add: On<Add, Optic>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    let mut e = commands.entity(add.entity);
    match catalog.get(item.kind()).id.as_str() {
        "pso_scope" => {
            e.insert((
                Turret {
                    setting: 1,
                    min: 1,
                    max: 10,
                    drum: PSO_DRUM,
                },
                Zero::default(),
                Rangefinder,
            ));
        }
        "pu_scope" => {
            e.insert((
                Turret {
                    setting: 1,
                    min: 1,
                    max: 13,
                    drum: PU_DRUM,
                },
                Zero::default(),
            ));
        }
        _ => {}
    }
}

/// A gun with a zeroable scope on it has the drum among its grab points
/// (where the scope's drum is now); one without, none.
fn turret_points(
    mut guns: Query<(Entity, &GlobalTransform, &mut Grabbable), With<Gun>>,
    scopes: Query<(&MountedOn, &GlobalTransform, &Turret)>,
) {
    for (g, gt, mut grab) in &mut guns {
        let want = scopes.iter().find(|(m, ..)| m.0 == g).map(|(_, sgt, t)| {
            gt.affine()
                .inverse()
                .transform_point3(sgt.transform_point(t.drum))
        });
        // Read first: only a change marks the gun's grab points changed.
        let have = grab
            .bypass_change_detection()
            .points
            .iter()
            .position(|p| p.role == GripRole::Handle(TURRET_HANDLE));
        match (want, have) {
            (Some(at), Some(i)) => {
                if grab.points[i].local.translation.distance(at) > 1e-3 {
                    grab.points[i].local.translation = at;
                }
            }
            (Some(at), None) => grab.points.push(GrabPoint::new(
                Transform::from_translation(at),
                GripRole::Handle(TURRET_HANDLE),
                DRUM_REACH,
            )),
            (None, Some(i)) => {
                grab.points.remove(i);
            }
            (None, None) => {}
        }
    }
}

/// The hand's turn about `axis`: the angle of its right-hand direction,
/// flattened onto the drum's face.
fn wrist(rot: Quat, axis: Vec3) -> Vec3 {
    let r = rot * Vec3::X;
    (r - axis * r.dot(axis)).normalize_or_zero()
}

/// A hand on the drum turns it a click per [`CLICK`] of the wrist.
fn turn_turrets(
    hands: Query<(Entity, &Hand)>,
    mut scopes: Query<(Entity, &MountedOn, &GlobalTransform, &mut Turret)>,
    mut taken: Local<HashMap<Entity, (Entity, Vec3, u32)>>,
    mut clicked: MessageWriter<TurretClicked>,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
    rigs: Query<&crate::rig::RigHand>,
) {
    taken.retain(|h, _| {
        hands
            .get(*h)
            .is_ok_and(|(_, hand)| matches!(hand.handle, Some((_, TURRET_HANDLE))))
    });
    for (hand_e, hand) in &hands {
        let Some((gun, TURRET_HANDLE)) = hand.handle else {
            continue;
        };
        let Some((scope, _, sgt, mut turret)) = scopes.iter_mut().find(|(_, m, ..)| m.0 == gun)
        else {
            continue;
        };
        let axis = sgt.rotation() * Vec3::Y;
        let now = wrist(hand.world.rotation, axis);
        let (_, from, start) = *taken.entry(hand_e).or_insert((scope, now, turret.setting));
        // Clockwise from above (a turn about -axis) adds range.
        let turned = from.cross(now).dot(-axis).atan2(from.dot(now));
        let clicks = (turned / CLICK).round() as i64;
        let setting = (start as i64 + clicks).clamp(turret.min as i64, turret.max as i64) as u32;
        if setting != turret.setting {
            turret.setting = setting;
            clicked.write(TurretClicked {
                scope,
                setting,
                at: sgt.transform_point(turret.drum),
            });
            if let Ok(rig) = rigs.get(hand_e) {
                haptics.write(crate::rig::haptics::Haptic {
                    side: rig.side,
                    amplitude: 0.2,
                    seconds: 0.012,
                });
            }
        }
    }
}

/// Flat mode: `]` and `[` click the drum of the scope on the gun in your
/// right hand up and down.
fn flat_drum(
    keys: Res<ButtonInput<KeyCode>>,
    players: Query<&Children, (With<crate::rig::LocalPlayer>, Without<crate::bot::Bot>)>,
    hands: Query<(&crate::rig::RigHand, &Hand)>,
    mut scopes: Query<(Entity, &MountedOn, &GlobalTransform, &mut Turret)>,
    mut clicked: MessageWriter<TurretClicked>,
) {
    let step: i64 = if keys.just_pressed(KeyCode::BracketRight) {
        1
    } else if keys.just_pressed(KeyCode::BracketLeft) {
        -1
    } else {
        return;
    };
    let Some(gun) = players.iter().next().and_then(|c| {
        c.iter()
            .filter_map(|h| hands.get(h).ok())
            .find(|(rh, _)| rh.side == crate::rig::Side::Right)
            .and_then(|(_, h)| h.holding)
    }) else {
        return;
    };
    for (scope, m, sgt, mut turret) in &mut scopes {
        if m.0 != gun {
            continue;
        }
        let setting =
            (turret.setting as i64 + step).clamp(turret.min as i64, turret.max as i64) as u32;
        if setting != turret.setting {
            turret.setting = setting;
            clicked.write(TurretClicked {
                scope,
                setting,
                at: sgt.transform_point(turret.drum),
            });
        }
    }
}

/// The height of a scope's line of sight over its gun's bore.
pub fn sight_height(gun: &Gun, gun_gt: &GlobalTransform, scope_gt: &GlobalTransform) -> f32 {
    gun_gt
        .affine()
        .inverse()
        .transform_point3(scope_gt.translation())
        .y
        - gun.layout.muzzle.y
}

/// Each scope's zero for its turret's range and the gun it's on.
fn zero_scopes(
    mut scopes: Query<(&Turret, &mut Zero, Option<&MountedOn>, &GlobalTransform)>,
    guns: Query<(&Gun, &GlobalTransform)>,
) {
    for (turret, mut zero, mounted, sgt) in &mut scopes {
        let gun = mounted.and_then(|m| guns.get(m.0).ok().map(|g| (m.0, g)));
        let Some((g_e, (g, ggt))) = gun else {
            if zero.gun.is_some() {
                *zero = Zero::default();
            }
            continue;
        };
        let same = zero.range_m == turret.range_m() && zero.gun == Some(g_e);
        let angle = if same {
            zero.angle
        } else {
            super::ballistics::zero_angle(
                g.firearm.def.muzzle_velocity,
                sight_height(g, ggt, sgt),
                turret.range_m(),
            )
        };
        // The line of sight is the bore turned down by the angle: in the
        // scope's frame, however the scope sits.
        let look = sgt.rotation().inverse() * ggt.rotation() * Quat::from_rotation_x(-angle);
        // (Not `angle_between`: its acos can't see under about 3e-4 rad in
        // f32, and a look that stale is a 10 cm miss at 300 m.)
        let moved = (zero.look.inverse() * look).xyz().length() * 2.0;
        if !same || moved > 1e-6 {
            *zero = Zero {
                range_m: turret.range_m(),
                angle,
                look,
                gun: Some(g_e),
            };
        }
    }
}

/// Where a zeroed scope looks: from the scope, its line of sight.
pub fn sight_line(scope_gt: &GlobalTransform, zero: &Zero) -> (Vec3, Vec3) {
    (
        scope_gt.translation(),
        scope_gt.rotation() * zero.look * Vec3::NEG_Z,
    )
}

/// Client side: a zeroed scope's lens looks down along its line of sight.
pub fn aim_lenses(
    mut cams: Query<(&ChildOf, &mut Transform), With<crate::render::OffscreenCamera>>,
    optics: Query<(Option<&Zero>, Option<&super::optics::EyeBox>)>,
) {
    for (parent, mut t) in &mut cams {
        let Ok((zero, eye)) = optics.get(parent.parent()) else {
            continue;
        };
        // The zero, and off-axis the picture sliding against the reticle
        // (the reticle sits on the lens; the view behind it turns).
        let shift = eye.map_or(Vec2::ZERO, |e| e.shift);
        let want = zero.map_or(Quat::IDENTITY, |z| z.look)
            * Quat::from_euler(EulerRot::YXZ, -shift.x, shift.y, 0.0);
        if t.rotation.angle_between(want) > 1e-5 {
            t.rotation = want;
        }
    }
}

/// A drum's rest pose, before it was turned.
#[derive(Component)]
pub struct DrumRest(Quat);

/// Client side: each scope's `drum` node turns with its setting, a full
/// turn over its positions (clockwise from above as the range goes up).
pub fn turn_drums(
    mut commands: Commands,
    scopes: Query<(Entity, &Turret)>,
    children: Query<&Children>,
    mut nodes: Query<(&Name, &mut Transform, Option<&DrumRest>)>,
) {
    for (e, turret) in &scopes {
        let positions = (turret.max - turret.min + 1) as f32;
        let turn = -std::f32::consts::TAU * (turret.setting - turret.min) as f32 / positions;
        for d in children.iter_descendants(e) {
            let Ok((name, mut t, rest)) = nodes.get_mut(d) else {
                continue;
            };
            if name.as_str() != "drum" {
                continue;
            }
            let rest = match rest {
                Some(r) => r.0,
                None => {
                    commands.entity(d).insert(DrumRest(t.rotation));
                    t.rotation
                }
            };
            let want = rest * Quat::from_rotation_y(turn);
            if t.rotation.angle_between(want) > 1e-4 {
                t.rotation = want;
            }
        }
    }
}

/// The PSO-1's reticle, `fov_deg` across: the aiming chevron at the centre
/// with three below for holdover a thousandth apart, the windage scale
/// either side, and the rangefinder low on the left: a baseline and a
/// curve over it, a 1.7 m man fitting between them at the range marked
/// above (2 to 10, hundreds of metres).
pub fn pso_reticle(images: &mut Assets<Image>, fov_deg: f32) -> Handle<Image> {
    let n = 512u32;
    let px_per_rad = n as f32 / fov_deg.to_radians();
    // The Russian "thousandth": a circle's 6000th.
    let mil = std::f32::consts::TAU / 6000.0 * px_per_rad;
    let c = n as f32 / 2.0;
    let mut data = vec![0u8; (n * n * 4) as usize];
    let mut dot = |x: f32, y: f32| {
        let (x, y) = (x.round() as i32, y.round() as i32);
        if x >= 0 && y >= 0 && x < n as i32 && y < n as i32 {
            let i = ((y as u32 * n + x as u32) * 4) as usize;
            data[i + 3] = 255;
        }
    };
    let mut line = |a: Vec2, b: Vec2, dot: &mut dyn FnMut(f32, f32)| {
        let steps = (a.distance(b) * 2.0).ceil().max(1.0) as usize;
        for i in 0..=steps {
            let p = a.lerp(b, i as f32 / steps as f32);
            dot(p.x, p.y);
            dot(p.x + 0.5, p.y);
            dot(p.x, p.y + 0.5);
        }
    };
    // Chevrons: the aiming one at the centre (its tip the aim point), and
    // three smaller under it.
    let chevron = |tip: Vec2,
                   size: f32,
                   dot: &mut dyn FnMut(f32, f32),
                   line: &mut dyn FnMut(Vec2, Vec2, &mut dyn FnMut(f32, f32))| {
        line(tip, tip + Vec2::new(-size, size * 1.6), dot);
        line(tip, tip + Vec2::new(size, size * 1.6), dot);
    };
    chevron(Vec2::new(c, c), mil * 1.0, &mut dot, &mut line);
    for k in 1..=3 {
        chevron(
            Vec2::new(c, c + mil * k as f32),
            mil * 0.5,
            &mut dot,
            &mut line,
        );
    }
    // The windage scale: a tick each thousandth, long every fifth.
    for k in -10i32..=10 {
        if k == 0 {
            continue;
        }
        let x = c + k as f32 * mil;
        let h = if k % 5 == 0 { mil * 0.8 } else { mil * 0.4 };
        line(Vec2::new(x, c), Vec2::new(x, c - h), &mut dot);
    }
    line(
        Vec2::new(c - 10.0 * mil, c),
        Vec2::new(c - 1.5 * mil, c),
        &mut dot,
    );
    line(
        Vec2::new(c + 1.5 * mil, c),
        Vec2::new(c + 10.0 * mil, c),
        &mut dot,
    );
    // The rangefinder: the baseline, and over it the curve 1.7 m high at
    // each range, 1000 m at the left to 200 m at the right.
    // (Inside the lens's circle.)
    let base = c + 0.5 * c;
    let (x0, x1) = (c - 0.7 * c, c - 0.1 * c);
    let x_at = |m: f32| x0 + (x1 - x0) * (1000.0 - m) / 800.0;
    line(Vec2::new(x0, base), Vec2::new(x1, base), &mut dot);
    let mut last = None;
    for i in 0..=80 {
        let m = 1000.0 - 10.0 * i as f32;
        let p = Vec2::new(x_at(m), base - STADIA_M / m * px_per_rad);
        if let Some(q) = last {
            line(q, p, &mut dot);
        }
        last = Some(p);
    }
    // Its numbers, hundreds of metres, over the curve.
    for m in [200.0f32, 400.0, 600.0, 800.0, 1000.0] {
        let x = x_at(m);
        let top = base - STADIA_M / m * px_per_rad - 4.0;
        line(Vec2::new(x, top), Vec2::new(x, top + 3.0), &mut dot);
        digits((m / 100.0) as u32, Vec2::new(x - 4.0, top - 14.0), &mut dot);
    }
    images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    ))
}

/// A number in a 3x5 pixel face, doubled, its top left at `at`.
fn digits(v: u32, at: Vec2, dot: &mut dyn FnMut(f32, f32)) {
    const FACE: [[u8; 5]; 10] = [
        [7, 5, 5, 5, 7],
        [2, 6, 2, 2, 7],
        [7, 1, 7, 4, 7],
        [7, 1, 3, 1, 7],
        [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7],
        [7, 4, 7, 5, 7],
        [7, 1, 1, 2, 2],
        [7, 5, 7, 5, 7],
        [7, 5, 7, 1, 7],
    ];
    let s = v.to_string();
    for (i, ch) in s.chars().enumerate() {
        let d = ch.to_digit(10).unwrap_or(0) as usize;
        for (row, bits) in FACE[d].iter().enumerate() {
            for col in 0..3 {
                if bits & (4 >> col) != 0 {
                    for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                        dot(
                            at.x + i as f32 * 8.0 + col as f32 * 2.0 + dx,
                            at.y + row as f32 * 2.0 + dy,
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::{InSocket, Socket, SocketKind, SocketOwner};
    use crate::rig::Side;
    use crate::vr_harness::Vr;
    use crate::weapons::ballistics::{Projectile, Shot};
    use avian3d::prelude::RigidBody;
    use extraction_core::items::Mount;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    /// The pure part: a 300 m zero crosses the line at 300; nearer it's
    /// high, farther low.
    #[test]
    fn a_zero_crosses_the_line_at_its_range() {
        use crate::weapons::ballistics::{height_at, zero_angle};
        let (v, h) = (900.0, 0.06);
        let a = zero_angle(v, h, 300.0);
        let at = |m: f32| height_at(v, a, m).unwrap().0 - h;
        assert!(at(300.0).abs() < 0.005, "{}", at(300.0));
        assert!(at(150.0) > 0.03, "high at 150: {}", at(150.0));
        assert!(at(400.0) < -0.15, "low at 400: {}", at(400.0));
    }

    /// Put a PSO on an AK by hand, take its drum with the other hand's
    /// trigger and turn it two clicks (to 300 m); aim the reticle at a
    /// point 300 m off and the round crosses there. At 400 m it's low (hold
    /// over), and at the old 100 m zero, 300 m is low too.
    #[test]
    fn zeroed_at_300_hits_at_300() {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        // The rifle in the right hand.
        let ak = vr.spawn(
            "ak74",
            Transform::from_translation(feet + Vec3::new(0.15, 1.2, -0.45)),
        );
        vr.app.world_mut().entity_mut(ak).insert(RigidBody::Static);
        let grip = vr.point(ak, |r| *r == GripRole::Primary);
        let rake = vr.app.world().get::<Gun>(ak).unwrap().layout.grip.rotation;
        vr.hand(Side::Right, grip, rake, 1.0, 0.0);
        vr.step(3);
        vr.app.world_mut().entity_mut(ak).insert(RigidBody::Dynamic);
        vr.step(20);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(ak));
        // The scope in the left, let go on the dovetail.
        let at = feet + Vec3::new(-0.3, 1.0, -0.3);
        let scope = vr.spawn("pso_scope", Transform::from_translation(at));
        vr.app
            .world_mut()
            .entity_mut(scope)
            .insert(RigidBody::Static);
        vr.hand(Side::Left, at, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Left, 1.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(scope)
            .insert(RigidBody::Dynamic);
        assert_eq!(vr.hand_state(Side::Left).holding, Some(scope));
        let dovetail = {
            let mut q = vr
                .app
                .world_mut()
                .query::<(&Socket, &SocketOwner, &GlobalTransform)>();
            q.iter(vr.app.world())
                .find(|(s, o, _)| o.0 == ak && s.kind == SocketKind::Mount(Mount::Dovetail))
                .map(|(.., g)| g.translation())
                .expect("a dovetail")
        };
        let from = vr.hand_state(Side::Left).world.translation;
        let off = vr
            .app
            .world()
            .get::<GlobalTransform>(scope)
            .unwrap()
            .translation()
            - from;
        for i in 1..=20 {
            vr.move_hand(Side::Left, from.lerp(dovetail - off, i as f32 / 20.0));
            vr.step(1);
        }
        vr.grip(Side::Left, 0.0);
        vr.step(5);
        assert!(vr.app.world().get::<InSocket>(scope).is_some(), "mounted");
        assert_eq!(vr.app.world().get::<MountedOn>(scope), Some(&MountedOn(ak)));
        // The drum: the left hand's trigger on it, and a turn of the wrist
        // clockwise seen from above.
        vr.move_hand(Side::Left, feet + Vec3::new(-0.3, 1.0, -0.2));
        vr.step(3);
        let drum = vr.point(ak, |r| *r == GripRole::Handle(TURRET_HANDLE));
        vr.hand(Side::Left, drum, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.trigger(Side::Left, 1.0);
        vr.step(2);
        assert_eq!(
            vr.hand_state(Side::Left).handle,
            Some((ak, TURRET_HANDLE)),
            "took the drum"
        );
        for i in 1..=20 {
            let turn = Quat::from_rotation_y(-2.0 * CLICK * i as f32 / 20.0);
            vr.hand(Side::Left, drum, turn, 0.0, 1.0);
            vr.step(1);
        }
        vr.trigger(Side::Left, 0.0);
        vr.step(2);
        assert_eq!(vr.app.world().get::<Turret>(scope).unwrap().setting, 3);
        vr.step(2);
        let zero = *vr.app.world().get::<Zero>(scope).unwrap();
        assert_eq!(zero.range_m, 300.0);
        assert!(zero.angle > 0.0);

        // Set the rifle down up in the air, level sight line, and shoot
        // along its bore.
        vr.grip(Side::Right, 0.0);
        vr.step(2);
        let miss = |vr: &mut Vr, zero: Zero, at_m: f32, seed: u64| -> f32 {
            vr.app.world_mut().entity_mut(ak).insert(RigidBody::Static);
            let pose = Transform::from_translation(feet + Vec3::new(0.0, 400.0, 0.0))
                .with_rotation(Quat::from_rotation_x(zero.angle));
            *vr.app.world_mut().get_mut::<Transform>(ak).unwrap() = pose;
            vr.step(2);
            let sgt = *vr.app.world().get::<GlobalTransform>(scope).unwrap();
            let zero = *vr.app.world().get::<Zero>(scope).unwrap();
            let (eye, look) = sight_line(&sgt, &zero);
            let target = eye + look * at_m;
            let (muzzle, bore, speed) = {
                let g = vr.app.world().get::<Gun>(ak).unwrap();
                let gt = vr.app.world().get::<GlobalTransform>(ak).unwrap();
                (
                    gt.transform_point(g.layout.muzzle),
                    gt.rotation() * Vec3::NEG_Z,
                    g.firearm.def.muzzle_velocity,
                )
            };
            vr.app.world_mut().write_message(Shot {
                gun: ak,
                shooter: None,
                origin: muzzle,
                direction: bore,
                speed,
                damage: 10.0,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed,
            });
            let mut last: Option<Vec3> = None;
            for i in 0..200 {
                vr.step(1);
                let mut q = vr.app.world_mut().query::<(&Projectile, &Transform)>();
                let Some((_, t)) = q.iter(vr.app.world()).next() else {
                    if i < 5 {
                        continue;
                    }
                    break;
                };
                let p = t.translation;
                let along = |x: Vec3| (x - eye).dot(look);
                if along(p) >= at_m {
                    let a = last.unwrap_or(p);
                    let f = (at_m - along(a)) / (along(p) - along(a)).max(1e-6);
                    let cross = a.lerp(p, f);
                    return cross.y - target.y;
                }
                last = Some(p);
            }
            panic!("the round never got {at_m} m");
        };
        let at300 = miss(&mut vr, zero, 300.0, 1);
        assert!(at300.abs() < 0.05, "zeroed at 300, at 300: {at300}");
        let at400 = miss(&mut vr, zero, 400.0, 2);
        assert!(at400 < -0.15, "at 400 it wants holdover: {at400}");
        // Back to 100 m: 300 m is low.
        vr.app.world_mut().get_mut::<Turret>(scope).unwrap().setting = 1;
        vr.step(2);
        let zero100 = *vr.app.world().get::<Zero>(scope).unwrap();
        assert_eq!(zero100.range_m, 100.0);
        let low = miss(&mut vr, zero100, 300.0, 3);
        assert!(low < -0.15, "a 100 m zero at 300: {low}");
    }
}
