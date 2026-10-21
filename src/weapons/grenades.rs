//! Hand grenades in the hand: the [`extraction_core::grenade`] rules given
//! a body.
//!
//! - The gripping hand holds the spoon down. The other hand takes the
//!   ring (the trigger pinches it, as it takes a slide) and pulls it away:
//!   the pin comes out and falls. Safe for as long as the grip holds.
//! - Let go (a throw is the hand's release velocity) or put it down, and
//!   the spoon flies: the fuse is lit. B on the gripping hand thumbs the
//!   spoon off while you hold on (cooking it).
//! - When it goes: fragments as real rounds (so walls stop them and
//!   concrete turns them), a blast that hurts and shoves what it can see
//!   and falls off with distance, and a bang every robot in the valley
//!   hears.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::grenade::{Event as FuseEvent, Grenade};
use extraction_core::noise::Rng;

use super::ballistics::{Hit, Projectile, ShotMods};
use crate::interact::{GrabPoint, Grabbable, GripRole, Hand, Held};
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::player::{BodyCollider, Player};
use crate::rig::RigInput;
use crate::world::Layer;

/// A grenade item's fuse, and who pulled its pin.
#[derive(Component, Debug)]
pub struct Live {
    pub grenade: Grenade,
    pub thrower: Option<Entity>,
}

/// Something a grenade did, for sound and the flash.
#[derive(Message, Clone, Copy, Debug)]
pub struct GrenadeEvent {
    pub grenade: Entity,
    pub event: FuseEvent,
    pub at: Vec3,
}

/// Where a hand takes the ring and pulls, item space (the ring is on the
/// +X side, below the fuse).
pub const PIN_AT: Vec3 = Vec3::new(0.034, 0.03, 0.0);
/// How far the ring must come away to pull the pin, metres.
pub const PIN_PULL: f32 = 0.05;
/// Fragments slow fast: their drag over a rifle round's.
pub const FRAGMENT_DRAG: f32 = 125.0;
/// How far a bang is heard, metres.
pub const BANG_HEARD: f32 = 160.0;

pub struct GrenadePlugin;

impl Plugin for GrenadePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<GrenadeEvent>()
            .add_observer(setup_grenades)
            .add_systems(
                FixedUpdate,
                (pull_pins, spoons, burn_fuses)
                    .chain()
                    .after(crate::interact::InteractSystems::Follow)
                    .before(super::ballistics::BallisticsSystems),
            );
    }
}

fn setup_grenades(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    let Some(def) = &catalog.get(item.kind()).grenade else {
        return;
    };
    commands.entity(add.entity).insert((
        Live {
            grenade: Grenade::new(def.clone()),
            thrower: None,
        },
        Grabbable {
            points: vec![
                GrabPoint::free(),
                GrabPoint::new(
                    Transform::from_translation(PIN_AT),
                    GripRole::Handle(0),
                    0.05,
                ),
            ],
        },
    ));
}

/// A hand on the ring pulling away from a grenade another hand holds.
#[allow(clippy::type_complexity)]
fn pull_pins(
    mut commands: Commands,
    mut hands: Query<(Entity, &mut Hand)>,
    mut grenades: Query<(Entity, &mut Live, &GlobalTransform, Option<&Held>)>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<GrenadeEvent>,
) {
    for (_, mut hand) in &mut hands {
        let Some((g, _)) = hand.handle else {
            continue;
        };
        let Ok((ge, mut live, gt, held)) = grenades.get_mut(g) else {
            continue;
        };
        let local = gt
            .affine()
            .inverse()
            .transform_point3(hand.world.translation);
        if local.distance(hand.handle_origin) < PIN_PULL {
            continue;
        }
        let events = live.grenade.pull_pin(held.is_some());
        if events.is_empty() {
            continue;
        }
        live.thrower = held.map(|h| h.player);
        // The ring comes away in the fingers and drops.
        hand.handle = None;
        if let Some(kind) = catalog.kind("grenade_pin") {
            let e = spawn_item(
                &mut commands,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_translation(hand.world.translation),
                    condition: 1.0,
                },
            );
            commands.entity(e).insert(LinearVelocity(hand.velocity));
        }
        for event in events {
            out.write(GrenadeEvent {
                grenade: ge,
                event,
                at: gt.translation(),
            });
        }
    }
}

/// The spoon goes when the grenade leaves the hand, or when B thumbs it
/// off while it's held (cooking).
#[allow(clippy::type_complexity)]
fn spoons(
    mut commands: Commands,
    mut grenades: Query<(Entity, &mut Live, &GlobalTransform, Option<&Held>)>,
    players: Query<&RigInput, With<Player>>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<GrenadeEvent>,
) {
    for (e, mut live, gt, held) in &mut grenades {
        let thumbed = held.is_some_and(|h| {
            players
                .get(h.player)
                .is_ok_and(|i| i.hand(h.side).secondary)
        });
        if held.is_some() && !thumbed {
            continue;
        }
        let events = live.grenade.release_spoon();
        if events.is_empty() {
            continue;
        }
        // The spoon flicks off on its spring.
        if let Some(kind) = catalog.kind("grenade_spoon") {
            let s = spawn_item(
                &mut commands,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_translation(gt.transform_point(Vec3::Z * 0.035)),
                    condition: 1.0,
                },
            );
            commands
                .entity(s)
                .insert(LinearVelocity(gt.rotation() * Vec3::new(0.0, 1.5, 2.0)));
        }
        for event in events {
            out.write(GrenadeEvent {
                grenade: e,
                event,
                at: gt.translation(),
            });
        }
    }
}

/// Fuses burn down; at zero, the grenade goes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn burn_fuses(
    mut commands: Commands,
    mut grenades: Query<(Entity, &mut Live, &GlobalTransform, Option<&Held>)>,
    mut hands: Query<&mut Hand>,
    parts: Query<(Entity, &GlobalTransform), With<crate::robots::RobotPart>>,
    bodies: Query<(Entity, &GlobalTransform), With<BodyCollider>>,
    mut pushed: Query<(&GlobalTransform, &RigidBody, Forces)>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    mut noise: MessageWriter<crate::robots::Noise>,
    mut out: MessageWriter<GrenadeEvent>,
) {
    let dt = time.delta_secs();
    for (e, mut live, gt, held) in &mut grenades {
        if !live.grenade.update(dt).contains(&FuseEvent::Exploded) {
            continue;
        }
        let at = gt.translation();
        let def = live.grenade.def.clone();
        let thrower = live.thrower;
        out.write(GrenadeEvent {
            grenade: e,
            event: FuseEvent::Exploded,
            at,
        });
        noise.write(crate::robots::Noise {
            at,
            radius: BANG_HEARD,
            source: thrower,
        });
        // Fragments: real rounds, all round. They can hurt the thrower too.
        // Seeded from where it burst, not its entity (which hangs on spawn
        // order).
        let mut rng = Rng::new(super::gun_seed("frag", at));
        let twist = rng.range(0.0, std::f32::consts::TAU);
        for d in extraction_core::grenade::spray(def.fragments, twist) {
            let dir = Vec3::from_array(d);
            commands.spawn((
                Name::new("Fragment"),
                Projectile {
                    velocity: dir * def.fragment_speed * rng.range(0.85, 1.1),
                    damage: def.fragment_damage,
                    shooter: None,
                    gun: e,
                    life: 0.6,
                    mods: ShotMods {
                        drag: FRAGMENT_DRAG,
                        ..default()
                    },
                    mass_g: def.fragment_g,
                    load: Default::default(),
                    seed: rng.next_u64(),
                    passes: 0,
                },
                Transform::from_translation(at + dir * 0.04),
            ));
        }
        // The blast: what it can see, by distance.
        let sees = |to: Vec3| {
            let v = to - at;
            let Ok(dir) = Dir3::new(v) else {
                return true;
            };
            spatial
                .cast_ray(
                    at,
                    dir,
                    v.length() - 0.05,
                    true,
                    &SpatialQueryFilter::from_mask(Layer::World),
                )
                .is_none()
        };
        for (target, tgt) in parts.iter().chain(bodies.iter()) {
            let p = tgt.translation();
            let harm = extraction_core::grenade::blast_at(&def, p.distance(at));
            if harm > 0.0 && sees(p) {
                hits.write(Hit {
                    entity: target,
                    point: p,
                    normal: (at - p).normalize_or(Vec3::Y),
                    velocity: (p - at).normalize_or(Vec3::Y) * 50.0,
                    damage: harm,
                    shooter: thrower,
                    mass_g: 0.0,
                    load: Default::default(),
                    blow: None,
                });
            }
        }
        for (pgt, rb, mut forces) in &mut pushed {
            if *rb != RigidBody::Dynamic {
                continue;
            }
            let p = pgt.translation();
            let k = 1.0 - p.distance(at) / def.blast_radius.max(0.1);
            if k > 0.0 && sees(p) {
                let dir = (p - at).normalize_or(Vec3::Y) + Vec3::Y * 0.3;
                forces.apply_linear_impulse(dir * 6.0 * k * k);
            }
        }
        // Gone, out of whatever hand had it.
        if let Some(h) = held {
            if let Ok(mut hand) = hands.get_mut(h.hand) {
                if hand.holding == Some(e) {
                    hand.holding = None;
                }
            }
        }
        commands.entity(e).despawn();
    }
}

/// The grenade model's `pin` goes with the pin, its `spoon` with the
/// spoon (client side).
pub fn show_grenade_parts(
    grenades: Query<(&Live, &Children)>,
    roots: Query<&Children, With<crate::items::ItemModelRoot>>,
    mut nodes: Query<(&Name, &mut Visibility)>,
) {
    use extraction_core::grenade::Fuse;
    for (live, children) in &grenades {
        let pinned = live.grenade.fuse == Fuse::Pinned;
        let spooned = matches!(live.grenade.fuse, Fuse::Pinned | Fuse::Held);
        for root in children.iter().filter_map(|c| roots.get(c).ok()) {
            for node in root.iter() {
                let Ok((name, mut vis)) = nodes.get_mut(node) else {
                    continue;
                };
                let show = match name.as_str() {
                    "pin" => pinned,
                    "spoon" => spooned,
                    _ => continue,
                };
                let want = if show {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *vis != want {
                    *vis = want;
                }
            }
        }
    }
}

/// The bang (client side): a flash, sparks, thrown earth and a cloud of
/// smoke that drifts off on the wind.
pub(crate) fn explosion_fx(
    mut commands: Commands,
    mut events: MessageReader<GrenadeEvent>,
    assets: Option<Res<super::impacts::FxAssets>>,
    pool: Option<ResMut<super::impacts::FxPool>>,
) {
    use super::impacts::{SPRAY, Spray, spray_bits};
    let (Some(a), Some(mut pool)) = (assets, pool) else {
        events.clear();
        return;
    };
    let pool = &mut *pool;
    for ev in events.read() {
        if ev.event != FuseEvent::Exploded {
            continue;
        }
        commands.spawn((
            Name::new("Grenade flash"),
            super::ballistics::Flash { life: 0.12 },
            PointLight {
                color: Color::srgb(1.0, 0.8, 0.55),
                intensity: 2_000_000.0,
                range: 30.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(ev.at + Vec3::Y * 0.3),
        ));
        let sprays = [
            Spray {
                n: 24,
                mat: |a| a.spark.clone(),
                size: Vec3::new(0.008, 0.008, 0.2),
                out: 4.0,
                scatter: 14.0,
                life: 0.25,
                align: true,
                ..SPRAY
            },
            Spray {
                n: 14,
                mat: |a| a.clod.clone(),
                size: Vec3::splat(0.04),
                out: 6.0,
                scatter: 4.0,
                life: 1.4,
                spin: 12.0,
                ..SPRAY
            },
            Spray {
                n: 12,
                mesh: |a| a.ball.clone(),
                mat: |a| a.soil_dust.clone(),
                size: Vec3::splat(0.5),
                out: 2.5,
                scatter: 2.0,
                life: 4.0,
                grow: 3.0,
                gravity: -0.35,
                drag: 1.6,
                wind: 1.0,
                ..SPRAY
            },
        ];
        for s in &sprays {
            spray_bits(
                &mut commands,
                pool,
                &a,
                s,
                ev.at + Vec3::Y * 0.1,
                Vec3::Y,
                Vec3::ZERO,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Side;
    use crate::vr_harness::Vr;
    use extraction_core::grenade::Fuse;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    /// Something to count what hits it: a block standing where a robot or
    /// a person would.
    #[derive(Component)]
    struct Dummy;

    #[derive(Resource, Default)]
    struct Struck(Vec<(Entity, f32)>);

    fn range() -> (Vr, Vec3) {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.app.init_resource::<Struck>().add_systems(
            FixedUpdate,
            (|mut hits: MessageReader<Hit>,
              dummies: Query<(), With<Dummy>>,
              mut s: ResMut<Struck>| {
                for h in hits.read() {
                    if dummies.contains(h.entity) {
                        s.0.push((h.entity, h.damage));
                    }
                }
            })
            .after(super::super::ballistics::BallisticsSystems),
        );
        (vr, feet)
    }

    fn take(vr: &mut Vr, id: &str, at: Vec3) -> Entity {
        let g = vr.spawn(id, Transform::from_translation(at));
        vr.app.world_mut().entity_mut(g).insert(RigidBody::Static);
        vr.hand(Side::Right, at, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        vr.app.world_mut().entity_mut(g).insert(RigidBody::Dynamic);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(g));
        g
    }

    /// The left trigger pinches the ring and pulls it away.
    fn pull_pin(vr: &mut Vr, g: Entity) {
        let ring = vr.point(g, |r| matches!(r, GripRole::Handle(_)));
        vr.hand(Side::Left, ring, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.trigger(Side::Left, 1.0);
        vr.step(2);
        assert!(vr.hand_state(Side::Left).handle.is_some(), "took the ring");
        for i in 1..=8 {
            vr.move_hand(Side::Left, ring + Vec3::X * 0.012 * i as f32);
            vr.step(1);
        }
        vr.trigger(Side::Left, 0.0);
        vr.step(2);
    }

    fn fuse(vr: &Vr, g: Entity) -> Option<Fuse> {
        vr.app.world().get::<Live>(g).map(|l| l.grenade.fuse)
    }

    /// Thrown with the pin in, dropped, left a minute: it never goes off.
    #[test]
    fn a_pinned_grenade_never_goes_off() {
        let (mut vr, feet) = range();
        let g = take(&mut vr, "rgd5", feet + Vec3::new(0.2, 1.2, -0.3));
        // A good overarm throw.
        for i in 0..8 {
            vr.move_hand(
                Side::Right,
                feet + Vec3::new(0.2, 1.2 + 0.05 * i as f32, -0.3 - 0.12 * i as f32),
            );
            vr.step(1);
        }
        vr.grip(Side::Right, 0.0);
        vr.step(90 * 20);
        assert_eq!(
            fuse(&vr, g),
            Some(Fuse::Pinned),
            "still there, still pinned"
        );
    }

    /// Pin out, the grip holds the spoon: safe for as long as you like.
    /// Let go and it goes off after its fuse, not before.
    #[test]
    fn the_pin_with_one_hand_the_spoon_with_the_other() {
        let (mut vr, feet) = range();
        let g = take(&mut vr, "rgd5", feet + Vec3::new(0.2, 1.2, -0.3));
        pull_pin(&mut vr, g);
        assert_eq!(fuse(&vr, g), Some(Fuse::Held), "pin out, spoon held");
        vr.step(90 * 8);
        assert_eq!(fuse(&vr, g), Some(Fuse::Held), "still safe in the hand");
        vr.grip(Side::Right, 0.0);
        let mut ticks = 0;
        while vr.app.world().get_entity(g).is_ok() {
            vr.step(1);
            ticks += 1;
            assert!(ticks < 90 * 10, "never went off");
        }
        let secs = ticks as f32 / 90.0;
        assert!((3.4..4.0).contains(&secs), "went off after {secs:.2} s");
    }

    /// The same grenade, a dummy either side: one in the open, one behind
    /// a concrete wall. The open one is torn up; the wall takes everything
    /// meant for the other.
    #[test]
    fn fragments_and_blast_respect_cover() {
        let (mut vr, feet) = range();
        let at = feet + Vec3::new(0.0, 0.5, -6.0);
        let open = vr
            .app
            .world_mut()
            .spawn((
                Dummy,
                Transform::from_translation(at + Vec3::new(3.0, 0.4, 0.0)),
                RigidBody::Static,
                Collider::cuboid(1.0, 1.6, 0.5),
                CollisionLayers::new(Layer::Robot, LayerMask::ALL),
            ))
            .id();
        let hidden = vr
            .app
            .world_mut()
            .spawn((
                Dummy,
                Transform::from_translation(at + Vec3::new(-3.0, 0.4, 0.0)),
                RigidBody::Static,
                Collider::cuboid(1.0, 1.6, 0.5),
                CollisionLayers::new(Layer::Robot, LayerMask::ALL),
            ))
            .id();
        vr.app.world_mut().spawn((
            crate::world::surfaces::Surfaces(vec![(
                Vec3::new(0.1, 1.5, 1.5),
                Transform::IDENTITY,
                crate::render::materials::Mat::Concrete,
            )]),
            Transform::from_translation(at + Vec3::new(-1.5, 0.5, 0.0)),
            RigidBody::Static,
            Collider::cuboid(0.2, 3.0, 3.0),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
        let g = vr.spawn("rgd5", Transform::from_translation(at));
        vr.app.world_mut().entity_mut(g).insert(RigidBody::Static);
        {
            let mut live = vr.app.world_mut().get_mut::<Live>(g).unwrap();
            live.grenade.pull_pin(true);
            live.grenade.release_spoon();
        }
        vr.step(90 * 5);
        assert!(vr.app.world().get_entity(g).is_err(), "it went off");
        let struck = &vr.app.world().resource::<Struck>().0;
        let on = |e: Entity| {
            struck
                .iter()
                .filter(|(x, _)| *x == e)
                .map(|(_, d)| d)
                .sum::<f32>()
        };
        assert!(on(open) > 30.0, "the open dummy took {}", on(open));
        assert_eq!(on(hidden), 0.0, "the wall took it all");
    }
}
