//! The crossbow's bolts (`docs/ARMOURY.md`): a bolt flies (slower than
//! any bullet: it drops, and you lead a moving robot), strikes, and lies
//! where it came down to be picked up and shot again. One that hits stone
//! or steel hard can break.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::ammo::Load;

use super::Gun;
use super::ballistics::{Hit, Shot};
use super::flares::Launcher;
use crate::items::{Catalog, SpawnItem, spawn_item};
use crate::world::Layer;

/// A bolt's mass, grams.
pub const BOLT_G: f32 = 28.0;
/// Air drag on a bolt in flight (quadratic, per metre).
const DRAG: f32 = 0.0015;
/// Seconds a bolt flies before it's given up (it fell out of the world).
const FLIGHT: f32 = 6.0;
/// The chance a bolt breaks on stone, brick, concrete or a robot's plate.
pub const BREAK_HARD: f32 = 0.3;

/// A bolt in flight.
#[derive(Component, Debug)]
pub struct Quarrel {
    pub velocity: Vec3,
    pub damage: f32,
    pub shooter: Option<Entity>,
    pub age: f32,
    /// Its own dice (whether it breaks).
    pub seed: u64,
}

pub struct CrossbowPlugin;

impl Plugin for CrossbowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (loose, fly)
                .chain()
                .after(super::ballistics::BallisticsSystems),
        );
    }
}

/// A crossbow's shot becomes a bolt in flight.
fn loose(
    mut commands: Commands,
    mut shots: MessageReader<Shot>,
    guns: Query<&Gun, With<Launcher>>,
) {
    for shot in shots.read() {
        if guns
            .get(shot.gun)
            .is_ok_and(|g| g.firearm.def.cartridge == "bolt")
        {
            let dir = shot.direction.normalize_or(Vec3::NEG_Z);
            commands.spawn((
                Name::new("Crossbow bolt"),
                Quarrel {
                    velocity: dir * shot.speed,
                    damage: shot.damage,
                    shooter: shot.shooter,
                    age: 0.0,
                    seed: shot.seed,
                },
                Transform::from_translation(shot.origin).looking_to(dir, Vec3::Y),
                Visibility::default(),
            ));
        }
    }
}

/// Bolts fly, strike and come down as bolts you can pick up.
fn fly(
    mut commands: Commands,
    mut bolts: Query<(Entity, &mut Quarrel, &mut Transform)>,
    spatial: SpatialQuery,
    parts: Query<&crate::robots::RobotPart>,
    struck: super::ballistics::Struck,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    let filter = SpatialQueryFilter::from_mask([Layer::World, Layer::Robot]);
    for (e, mut q, mut t) in &mut bolts {
        q.age += dt;
        if q.age > FLIGHT {
            commands.entity(e).despawn();
            continue;
        }
        let v = q.velocity;
        let mut nv = v + Vec3::new(0.0, -9.81, 0.0) * dt;
        nv -= nv * (DRAG * nv.length() * dt).min(0.5);
        let step = (v + nv) * 0.5 * dt;
        let Ok(dir) = Dir3::new(step) else {
            continue;
        };
        if let Some(hit) = spatial.cast_ray(t.translation, dir, step.length(), true, &filter) {
            let at = t.translation + dir * hit.distance;
            let robot = parts.contains(hit.entity);
            if robot {
                hits.write(Hit {
                    entity: hit.entity,
                    point: at,
                    normal: hit.normal,
                    velocity: v,
                    damage: q.damage,
                    shooter: q.shooter,
                    mass_g: BOLT_G,
                    load: Load::Standard,
                    blow: None,
                });
            }
            commands.entity(e).despawn();
            // Stone and steel can snap it; anything else keeps it whole.
            let cos_in = (-dir.dot(hit.normal)).clamp(0.0, 1.0);
            let hard = robot || struck.stuff(hit.entity, at, *dir, cos_in).0.ricochets();
            let mut dice = extraction_core::noise::Rng::new(q.seed);
            if hard && dice.f32() < BREAK_HARD {
                continue;
            }
            let Some(kind) = catalog.kind("crossbow_bolt") else {
                continue;
            };
            // It drops back off what it hit, there to be picked up.
            spawn_item(
                &mut commands,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_translation(at + hit.normal * 0.05)
                        .looking_to(dir.as_vec3(), Vec3::Y),
                    condition: 1.0,
                },
            );
            continue;
        }
        t.translation += step;
        if let Ok(d) = Dir3::new(nv) {
            t.look_to(d, Vec3::Y);
        }
        q.velocity = nv;
    }
}

/// A bolt in flight is drawn as the bolt item's model would be: a thin
/// steel rod (client side).
pub fn quarrel_visuals(
    mut commands: Commands,
    added: Query<Entity, Added<Quarrel>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut kit: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    for e in &added {
        let (mesh, mat) = kit
            .get_or_insert_with(|| {
                (
                    meshes.add(
                        Cylinder::new(0.004, 0.38)
                            .mesh()
                            .build()
                            .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.35, 0.35, 0.36),
                        metallic: 1.0,
                        perceptual_roughness: 0.5,
                        ..default()
                    }),
                )
            })
            .clone();
        commands.entity(e).with_children(|c| {
            c.spawn((Mesh3d(mesh), MeshMaterial3d(mat)));
        });
    }
}

#[cfg(test)]
mod vr_tests {
    use super::*;
    use crate::interact::GripRole;
    use crate::rig::Side;
    use crate::weapons::feel::vr_tests::*;
    use extraction_core::firearm::FireMode;

    /// The crossbow by hand: the left hand takes the string with its
    /// trigger and draws it back past the latch, where it catches; a bolt
    /// laid in the groove; loosed, it flies (no bullet, no case), strikes a
    /// wall and lies there as a bolt you can pick up. Let go short, the
    /// string slaps home and nothing's caught.
    #[test]
    fn the_crossbow_draws_looses_and_its_bolt_lies_to_be_found() {
        let mut r = hold("crossbow", false, false);
        let string = r.vr.point(r.gun, |p| *p == GripRole::Handle(0));
        let back =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .back()
                .as_vec3();
        let travel = gun_mut(&mut r).layout.handle.as_ref().unwrap().travel;
        let draw = |r: &mut Range, k: f32| {
            r.vr.hand(Side::Left, string, Quat::IDENTITY, 0.0, 0.0);
            r.vr.step(10);
            r.vr.trigger(Side::Left, 1.0);
            r.vr.step(2);
            assert!(
                r.vr.hand_state(Side::Left).handle.is_some(),
                "on the string"
            );
            glide_side(r, Side::Left, string, string + back * travel * k, 20);
            r.vr.trigger(Side::Left, 0.0);
            r.vr.move_hand(Side::Left, r.feet + Vec3::new(-0.4, 1.0, 0.0));
            r.vr.step(4);
        };
        draw(&mut r, 0.6);
        assert!(!gun_mut(&mut r).firearm.locked_back, "short: not caught");
        draw(&mut r, 1.1);
        assert!(gun_mut(&mut r).firearm.locked_back, "on the latch");
        take(&mut r, "crossbow_bolt", 1);
        let (port, _) = load_port(&mut r);
        let away = r.vr.hand_state(Side::Left).world.translation;
        glide(&mut r, away, port, 12);
        r.vr.step(2);
        r.vr.grip(Side::Left, 0.0);
        r.vr.step(2);
        assert_eq!(gun_mut(&mut r).firearm.live_rounds(), 1, "a bolt on");
        while gun_mut(&mut r).firearm.fire_mode() != FireMode::Semi {
            press(&mut r, "stick");
        }
        // A wall 8 m down the line.
        let (muzzle, fwd) = {
            let g = r.vr.app.world().get::<Gun>(r.gun).unwrap();
            let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            (gt.transform_point(g.layout.muzzle), gt.forward().as_vec3())
        };
        r.vr.app.world_mut().spawn((
            Transform::from_translation(muzzle + fwd * 8.0).looking_to(fwd, Vec3::Y),
            RigidBody::Static,
            Collider::cuboid(3.0, 3.0, 0.2),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
        r.vr.step(3);
        let count = |r: &mut Range| {
            let cat = r.vr.app.world().resource::<Catalog>().clone();
            let mut q = r.vr.app.world_mut().query::<&crate::items::Item>();
            q.iter(r.vr.app.world())
                .filter(|i| cat.get(i.kind()).id == "crossbow_bolt")
                .count()
        };
        let before = count(&mut r);
        r.vr.trigger(Side::Right, 1.0);
        r.vr.step(3);
        r.vr.trigger(Side::Right, 0.0);
        let mut q = r.vr.app.world_mut().query::<&Quarrel>();
        assert_eq!(q.iter(r.vr.app.world()).count(), 1, "a bolt in the air");
        r.vr.step(60);
        let mut q = r.vr.app.world_mut().query::<&Quarrel>();
        assert_eq!(q.iter(r.vr.app.world()).count(), 0, "it struck");
        // (Plywood-and-earth here: it doesn't break.)
        assert_eq!(count(&mut r), before + 1, "a bolt to pick up");
        assert!(!gun_mut(&mut r).firearm.locked_back);
    }
}
