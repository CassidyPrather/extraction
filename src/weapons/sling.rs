//! Slings, as you see them (client side): a strap on every long gun in
//! someone's hands or on their back, and a slung gun swinging on it.
//!
//! - In the hands the strap hangs slack between its swivels; slung, it
//!   runs from the front swivel up over the shoulder and down to the rear.
//! - A slung gun is a pendulum hung from its rear swivel: the body's
//!   acceleration and turning set it swinging, and it settles. Only the
//!   drawn parts swing; the gun itself stays where its slot holds it.
//!
//! Swivels come from the gun's layout guess (under the handguard, under
//! the butt) until the models carry their own.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::Gun;
use crate::interact::{Held, InSocket, Socket, SocketKind};
use crate::items::{Catalog, Item};
use extraction_core::inventory::SlotKind;

/// Segments a strap is drawn in.
const SEGMENTS: usize = 8;
/// How far a slack strap sags, metres.
const SAG: f32 = 0.12;
/// The strap's width and thickness, metres.
const STRAP: Vec2 = Vec2::new(0.025, 0.004);
/// The shoulder in the torso's frame (as the arms have it).
const SHOULDER: Vec3 = Vec3::new(0.17, 0.12, 0.03);
/// The swing: natural frequency (rad/s), damping ratio, and how far it
/// may go (rad).
const SWING_W: f32 = 6.0;
const SWING_DAMP: f32 = 0.25;
const SWING_MAX: f32 = 0.45;

/// A gun's front and rear sling swivels, gun space: the models' own
/// (`pt_sling_front`, `pt_sling_rear`), else a guess under the handguard
/// and the butt.
pub fn swivels(id: &str, gun: &Gun) -> Option<(Vec3, Vec3)> {
    let known = known_swivels(id);
    if known.is_some() {
        return known;
    }
    let support = gun.layout.support?;
    let (lo, hi) = gun.layout.bounds();
    let front = Vec3::new(0.0, support.y - 0.03, support.z);
    let rear = Vec3::new(0.0, lo.y + 0.02, hi.z - 0.06);
    Some((front, rear))
}

/// A gun's swivels as its model has them, if they've been taken from it.
pub fn known_swivels(id: &str) -> Option<(Vec3, Vec3)> {
    let v = Vec3::new;
    match id {
        "ak74" => Some((v(-0.0145, 0.039, -0.461), v(-0.022, -0.06, 0.269))),
        "mosin" => Some((v(0.0, -0.009, -0.64), v(0.0, -0.045, 0.17))),
        "sks" => Some((v(-0.0195, 0.0, -0.482), v(-0.021, -0.075, 0.152))),
        "toz_double" => Some((v(0.0, 0.006, -0.62), v(0.0, -0.111, 0.25))),
        "ppsh" => Some((v(-0.021, 0.006, -0.185), v(0.0, -0.063, 0.17))),
        "mp133" => Some((v(0.0, -0.01, -0.636), v(0.0, -0.105, 0.25))),
        "toz8" => Some((v(0.0, -0.0212, -0.43), v(0.0, -0.093, 0.2))),
        "crossbow" => Some((v(0.0, -0.0125, -0.37), v(0.0, -0.1033, 0.25))),
        "svd" => Some((v(-0.0162, 0.043, -0.543), v(-0.0237, -0.074, 0.222))),
        "rpk74" => Some((v(-0.0172, 0.0435, -0.6), v(-0.0205, -0.048, 0.218))),
        _ => None,
    }
}

/// A long gun's strap: its drawn segments.
#[derive(Component)]
pub struct Strap {
    segments: Vec<Entity>,
}

/// A slung gun's swing: angles about its rear swivel (pitch, roll) and
/// their rates, and where its rear swivel was, to feel the body move.
#[derive(Component, Default)]
pub struct SlingSwing {
    pub angle: Vec2,
    rate: Vec2,
    last: Option<(Vec3, Vec3)>,
}

impl SlingSwing {
    /// The swing as a rotation about the rear swivel, in gun space.
    pub fn about(&self, pivot: Vec3) -> Transform {
        let r = Quat::from_rotation_x(self.angle.x) * Quat::from_rotation_z(self.angle.y);
        Transform::from_translation(pivot - r * pivot).with_rotation(r)
    }
}

#[derive(Resource)]
struct StrapLook {
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
}

pub struct SlingPlugin;

impl Plugin for SlingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, strap_look)
            .add_systems(Update, (give_straps, swing, lay_straps).chain());
    }
}

fn strap_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(StrapLook {
        mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.22, 0.15),
            perceptual_roughness: 0.9,
            ..default()
        }),
    });
}

/// Long guns get a strap (drawn only when someone has the gun).
fn give_straps(
    mut commands: Commands,
    guns: Query<(Entity, &Gun, &Item), Without<Strap>>,
    look: Option<Res<StrapLook>>,
    catalog: Res<Catalog>,
) {
    let Some(look) = look else {
        return;
    };
    for (e, gun, item) in &guns {
        if swivels(&catalog.get(item.kind()).id, gun).is_none() {
            continue;
        }
        let segments = (0..SEGMENTS)
            .map(|_| {
                commands
                    .spawn((
                        Name::new("Strap"),
                        Mesh3d(look.mesh.clone()),
                        MeshMaterial3d(look.mat.clone()),
                        Transform::default(),
                        Visibility::Hidden,
                        NotShadowCaster,
                    ))
                    .id()
            })
            .collect();
        commands
            .entity(e)
            .insert((Strap { segments }, SlingSwing::default()));
    }
}

/// Where a slung gun hangs from: the shoulder its slot is on, world.
fn slung_shoulder(
    in_socket: Option<&InSocket>,
    sockets: &Query<(&Socket, &Transform, &ChildOf)>,
    torsos: &Query<&GlobalTransform>,
) -> Option<Vec3> {
    let (socket, local, parent) = sockets.get(in_socket?.socket).ok()?;
    if socket.kind != SocketKind::Body(SlotKind::Sling) {
        return None;
    }
    let side = if local.translation.x < 0.0 { -1.0 } else { 1.0 };
    let torso = torsos.get(parent.parent()).ok()?;
    Some(torso.transform_point(SHOULDER * Vec3::new(side, 1.0, 1.0)))
}

/// Slung guns swing on their straps: the body's acceleration and turn push
/// them, a damped spring brings them back.
fn swing(
    mut guns: Query<(
        &Gun,
        &Item,
        &GlobalTransform,
        &mut SlingSwing,
        Option<&InSocket>,
    )>,
    sockets: Query<(&Socket, &Transform, &ChildOf)>,
    torsos: Query<&GlobalTransform>,
    catalog: Res<Catalog>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().clamp(1e-4, 0.05);
    for (gun, item, gt, mut s, in_socket) in &mut guns {
        if slung_shoulder(in_socket, &sockets, &torsos).is_none() {
            *s = SlingSwing::default();
            continue;
        }
        let Some((_, rear)) = swivels(&catalog.get(item.kind()).id, gun) else {
            continue;
        };
        let at = gt.transform_point(rear);
        // Push from the pivot's acceleration, in gun space.
        let mut push = Vec2::ZERO;
        if let Some((p0, v0)) = s.last {
            let v = (at - p0) / dt;
            let a = (v - v0) / dt;
            let local = gt.rotation().inverse() * a;
            push = Vec2::new(local.z, -local.x) * 0.05;
            s.last = Some((at, v));
        } else {
            s.last = Some((at, Vec3::ZERO));
        }
        let w = SWING_W;
        let c = 2.0 * SWING_DAMP * w;
        let accel = -w * w * s.angle - c * s.rate + push * w * w;
        s.rate += accel * dt;
        let next = s.angle + s.rate * dt;
        s.angle = next.clamp(Vec2::splat(-SWING_MAX), Vec2::splat(SWING_MAX));
    }
}

/// Draws each strap: slack between the swivels in the hands, over the
/// shoulder when slung, hidden otherwise.
#[allow(clippy::type_complexity)]
fn lay_straps(
    guns: Query<(
        &Gun,
        &Item,
        &GlobalTransform,
        &Strap,
        &SlingSwing,
        Option<&Held>,
        Option<&InSocket>,
    )>,
    sockets: Query<(&Socket, &Transform, &ChildOf)>,
    torsos: Query<&GlobalTransform>,
    mut segs: Query<(&mut Transform, &mut Visibility), Without<Socket>>,
    catalog: Res<Catalog>,
) {
    for (gun, item, gt, strap, s, held, in_socket) in &guns {
        let shoulder = slung_shoulder(in_socket, &sockets, &torsos);
        let shown = held.is_some() || shoulder.is_some();
        let Some((front, rear)) = swivels(&catalog.get(item.kind()).id, gun) else {
            continue;
        };
        // Swing moves the swivels with the drawn gun.
        let sw = gt.compute_transform() * s.about(rear);
        let (f, r) = (sw.transform_point(front), sw.transform_point(rear));
        let curve = |t: f32| -> Vec3 {
            match shoulder {
                // Front up to the shoulder, down to the rear.
                Some(sh) => {
                    let lift = sh + Vec3::Y * 0.02;
                    if t < 0.5 {
                        f.lerp(lift, t * 2.0)
                    } else {
                        lift.lerp(r, (t - 0.5) * 2.0)
                    }
                }
                // Slack: a sag in the middle.
                None => f.lerp(r, t) - Vec3::Y * SAG * 4.0 * t * (1.0 - t),
            }
        };
        for (i, seg) in strap.segments.iter().enumerate() {
            let Ok((mut t, mut vis)) = segs.get_mut(*seg) else {
                continue;
            };
            let want = if shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *vis != want {
                *vis = want;
            }
            if !shown {
                continue;
            }
            let a = curve(i as f32 / SEGMENTS as f32);
            let b = curve((i + 1) as f32 / SEGMENTS as f32);
            let d = b - a;
            let len = d.length().max(1e-3);
            *t = Transform::from_translation((a + b) * 0.5)
                .looking_to(d / len, Vec3::Y)
                .with_scale(Vec3::new(STRAP.x, STRAP.y, len + 0.004));
        }
    }
}

#[cfg(test)]
mod vr_tests {
    use crate::interact::{GripRole, InSocket, Socket, SocketKind};
    use crate::rig::Side;
    use crate::vr_harness::Vr;
    use avian3d::prelude::RigidBody;
    use bevy::prelude::*;
    use extraction_core::inventory::SlotKind;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    /// Let go of a rifle at your shoulder and it's slung there; reach over
    /// that shoulder and grip, and it's back in your hand.
    #[test]
    fn a_rifle_let_go_at_the_shoulder_is_slung_and_comes_back() {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        let at = feet + Vec3::new(0.2, 1.2, -0.4);
        let gun = vr.spawn("ak74", Transform::from_translation(at));
        vr.app.world_mut().entity_mut(gun).insert(RigidBody::Static);
        let grip = vr.point(gun, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip, Quat::IDENTITY, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(gun)
            .insert(RigidBody::Dynamic);
        vr.step(10);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(gun));
        // The right shoulder, from the torso.
        let shoulder = {
            let mut q = vr
                .app
                .world_mut()
                .query::<(&GlobalTransform, &crate::interact::body::Torso)>();
            let (t, _) = q.iter(vr.app.world()).next().expect("a torso");
            t.transform_point(crate::interact::body::SHOULDER)
        };
        // Up to the front of the shoulder, and open the hand.
        let from = vr.hand_state(Side::Right).world.translation;
        let to = shoulder + Vec3::new(0.0, 0.05, -0.12);
        for i in 1..=20 {
            vr.move_hand(Side::Right, from.lerp(to, i as f32 / 20.0));
            vr.step(1);
        }
        vr.grip(Side::Right, 0.0);
        vr.step(5);
        let slot = vr
            .app
            .world()
            .get::<InSocket>(gun)
            .and_then(|s| vr.app.world().get::<Socket>(s.socket))
            .map(|s| s.kind.clone());
        assert_eq!(slot, Some(SocketKind::Body(SlotKind::Sling)), "slung");
        // Away, then back over the shoulder for it.
        vr.move_hand(Side::Right, feet + Vec3::new(0.3, 1.0, -0.3));
        vr.step(10);
        assert!(
            vr.app
                .world()
                .get::<crate::interact::ReachZone>(gun)
                .is_some(),
            "a slung gun can be reached for"
        );
        let over = shoulder + Vec3::new(0.0, 0.06, 0.05);
        vr.move_hand(Side::Right, over);
        vr.step(4);
        vr.grip(Side::Right, 1.0);
        vr.step(5);
        assert_eq!(
            vr.hand_state(Side::Right).holding,
            Some(gun),
            "back in hand"
        );
        assert!(vr.app.world().get::<InSocket>(gun).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shove sets a slung gun swinging, and it settles within a few
    /// seconds; it never swings past its limit.
    #[test]
    fn a_swing_settles() {
        let mut s = SlingSwing::default();
        s.rate = Vec2::new(3.0, -2.0);
        let mut most = 0.0f32;
        for _ in 0..300 {
            let w = SWING_W;
            let c = 2.0 * SWING_DAMP * w;
            let accel = -w * w * s.angle - c * s.rate;
            s.rate += accel / 90.0;
            s.angle =
                (s.angle + s.rate / 90.0).clamp(Vec2::splat(-SWING_MAX), Vec2::splat(SWING_MAX));
            most = most.max(s.angle.length());
        }
        assert!(most > 0.1 && most <= SWING_MAX * 1.5);
        assert!(s.angle.length() < 0.01, "settled: {:?}", s.angle);
    }
}
