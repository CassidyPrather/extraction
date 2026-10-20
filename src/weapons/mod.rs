//! Guns: the [`Firearm`] state machines from the core, given bodies.
//!
//! - Spawning a firearm item gives it a [`Gun`], grab points (grip,
//!   handguard, the slide or bolt), a magazine-well socket and a compound
//!   collider from its layout.
//! - Hands drive it: trigger, face buttons (mag release, slide stop,
//!   selector), and grabbing the slide/handle/bolt, whose motion along the
//!   gun becomes bolt travel.
//! - The mechanism's events become shots ([`ballistics`]), cases and live
//!   rounds thrown from the port, recoil ([`feel`]) and [`GunEvent`]s
//!   for sound.

pub mod ballistics;
pub mod feel;
pub mod flat;
pub mod impacts;
pub mod layout;
pub mod muzzle;
pub mod visuals;

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::firearm::{
    ActionKind, Event as FireEvent, FireMode, Firearm, Magazine, Round,
};
use extraction_core::items::Category;
use extraction_core::noise::Rng;

use crate::interact::{
    GrabPoint, Grabbable, GripRole, Hand, Held, InteractEvent, InteractSystems, Socket, SocketKind,
    SocketOwner,
};
use crate::items::{Catalog, CustomVisual, Item, SpawnItem, spawn_item};
use crate::player::Player;
use crate::rig::{RigHand, RigInput};
use crate::world::Layer;
use layout::{GunLayout, HandleKind};

/// A gun's live mechanism and its layout.
#[derive(Component)]
pub struct Gun {
    pub firearm: Firearm,
    pub layout: GunLayout,
    /// The magazine-well socket entity, if any.
    pub magwell: Option<Entity>,
    /// Last trigger value seen, for the trigger-part animation.
    pub trigger: f32,
}

/// A magazine item: what it fits and holds. Rounds live in `Item.rounds`.
#[derive(Component, Clone, Debug)]
pub struct Mag {
    pub family: String,
    pub capacity: u32,
    pub height: f32,
}

/// A spent case on the ground (for sounds), despawned after a while.
#[derive(Component)]
pub struct Casing {
    pub life: f32,
}

/// Something a gun did, with where it did it: for audio and effects.
#[derive(Message, Clone, Debug)]
pub struct GunEvent {
    pub gun: Entity,
    pub item: String,
    pub event: FireEvent,
    pub at: Vec3,
}

/// Seeded randomness for jams, duds and spread, so sims replay.
#[derive(Resource)]
pub struct GunRng(pub Rng);

/// Where a player's right-hand gun puts its sight line and its handguard,
/// in hand space: flat mode and bots use it to aim and to place the
/// supporting hand.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct AimHint {
    pub sight: Transform,
    pub support: Option<Vec3>,
}

fn update_aim_hints(
    mut players: Query<(&mut AimHint, &Children)>,
    hands: Query<(&crate::rig::RigHand, &Hand)>,
    guns: Query<(Entity, &Gun, &GlobalTransform)>,
    optics: Query<(
        &crate::gadgets::MountedOn,
        &crate::gadgets::Optic,
        &GlobalTransform,
    )>,
) {
    for (mut hint, children) in &mut players {
        let gun = children.iter().find_map(|c| {
            hands
                .get(c)
                .ok()
                .filter(|(rh, _)| rh.side == crate::rig::Side::Right)
                .and_then(|(_, h)| h.holding)
                .and_then(|e| guns.get(e).ok())
        });
        *hint = match gun {
            Some((ge, g, ggt)) => {
                let grip_inv = g.layout.grip.compute_affine().inverse();
                // A mounted scope moves the sight line up to its eyepiece.
                let sight = optics
                    .iter()
                    .find(|(m, _, _)| m.0 == ge)
                    .map(|(_, o, ogt)| {
                        let eye = ogt.compute_transform() * Transform::from_translation(o.eye);
                        Transform::from_matrix(
                            (ggt.affine().inverse() * eye.compute_affine()).into(),
                        )
                    })
                    .unwrap_or(g.layout.sight);
                AimHint {
                    sight: Transform::from_matrix((grip_inv * sight.compute_affine()).into()),
                    support: g.layout.support.map(|s| grip_inv.transform_point3(s)),
                }
            }
            None => AimHint {
                sight: Transform::from_xyz(0.0, 0.07, 0.0),
                support: None,
            },
        };
    }
}

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<GunEvent>()
            .insert_resource(GunRng(Rng::new(0x6A11)))
            .add_observer(setup_gun_items)
            .add_observer(|add: On<Add, Player>, mut commands: Commands| {
                commands.entity(add.entity).insert(AimHint::default());
            })
            .add_systems(
                PreUpdate,
                update_aim_hints.before(crate::rig::RigDriverSystems),
            )
            .add_plugins((
                ballistics::BallisticsPlugin,
                feel::FeelPlugin,
                impacts::ImpactPlugin,
            ))
            .add_systems(
                FixedUpdate,
                (
                    magwell_events,
                    work_handles,
                    tap_magazines,
                    feed_by_hand,
                    buttons,
                    triggers,
                    sync_mag_rounds,
                    age_casings,
                )
                    .chain()
                    .after(InteractSystems::Follow),
            );
    }
}

/// Gives firearm and magazine items their mechanism, grips and sockets.
fn setup_gun_items(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    let def = catalog.get(item.kind());
    if let Some(mag) = &def.magazine {
        commands.entity(add.entity).insert(Mag {
            family: mag.fits.clone(),
            capacity: mag.capacity,
            height: def.size_m()[1],
        });
        return;
    }
    let (Some(fdef), Some(layout)) = (&def.firearm, layout::layout(&def.id)) else {
        return;
    };
    let firearm = Firearm::new(fdef.clone(), item.condition);
    let mut points = vec![GrabPoint::new(layout.grip, GripRole::Primary, 0.12)];
    if let Some(s) = layout.support {
        points.push(GrabPoint::new(
            Transform::from_translation(s),
            GripRole::Support,
            0.14,
        ));
    }
    if let Some(h) = &layout.handle {
        points.push(GrabPoint::new(
            Transform::from_translation(h.grab),
            GripRole::Handle(0),
            0.07,
        ));
    }
    // Collider: the bounds of the blockout, as one box.
    let (lo, hi) = layout.bounds();
    let centre = (lo + hi) * 0.5;
    let size = (hi - lo).max(Vec3::splat(0.01));
    let mut e = commands.entity(add.entity);
    e.insert((
        CustomVisual,
        Grabbable { points },
        Collider::compound(vec![(
            centre,
            Quat::IDENTITY,
            Collider::cuboid(size.x, size.y, size.z),
        )]),
    ));
    let gun = add.entity;
    let magwell = layout.magwell.map(|seat| {
        let family = fdef.magazine_family.clone().unwrap_or_default();
        commands
            .spawn((
                Name::new("Magazine well"),
                Socket {
                    kind: SocketKind::MagWell(family),
                    occupant: None,
                    radius: 0.09,
                    seat: Transform::IDENTITY,
                },
                SocketOwner(gun),
                seat,
                Visibility::default(),
                ChildOf(gun),
            ))
            .id()
    });
    commands.entity(gun).insert(Gun {
        firearm,
        layout,
        magwell,
        trigger: 0.0,
    });
}

/// Magazines seated in or pulled from a well update the mechanism.
fn magwell_events(
    mut events: MessageReader<InteractEvent>,
    mut guns: Query<&mut Gun>,
    sockets: Query<(&Socket, &SocketOwner)>,
    mut mags: Query<(&mut Item, &Mag, &mut Transform)>,
    mut out: MessageWriter<GunEvent>,
    gts: Query<&GlobalTransform>,
) {
    for ev in events.read() {
        match *ev {
            InteractEvent::Socketed { item, socket } => {
                let Ok((s, owner)) = sockets.get(socket) else {
                    continue;
                };
                if !matches!(s.kind, SocketKind::MagWell(_)) {
                    continue;
                }
                let (Ok(mut gun), Ok((it, mag, mut t))) =
                    (guns.get_mut(owner.0), mags.get_mut(item))
                else {
                    continue;
                };
                // Hang the magazine from the top of the well.
                *t = Transform::from_xyz(0.0, -mag.height * 0.5, 0.0);
                let m = Magazine {
                    family: mag.family.clone(),
                    capacity: mag.capacity,
                    rounds: it.rounds,
                };
                if gun.firearm.insert_magazine(m).is_ok() {
                    out.write(GunEvent {
                        gun: owner.0,
                        item: String::new(),
                        event: FireEvent::MagazineInserted,
                        at: gts
                            .get(owner.0)
                            .map(|g| g.translation())
                            .unwrap_or_default(),
                    });
                }
            }
            InteractEvent::Unsocketed { item, socket } => {
                let Ok((s, owner)) = sockets.get(socket) else {
                    continue;
                };
                if !matches!(s.kind, SocketKind::MagWell(_)) {
                    continue;
                }
                let Ok(mut gun) = guns.get_mut(owner.0) else {
                    continue;
                };
                if let Some(m) = gun.firearm.remove_magazine() {
                    if let Ok((mut it, _, _)) = mags.get_mut(item) {
                        it.rounds = m.rounds;
                    }
                    out.write(GunEvent {
                        gun: owner.0,
                        item: String::new(),
                        event: FireEvent::MagazineRemoved,
                        at: gts
                            .get(owner.0)
                            .map(|g| g.translation())
                            .unwrap_or_default(),
                    });
                }
            }
            _ => {}
        }
    }
}

/// Keeps a seated magazine's own count in step with the gun's.
fn sync_mag_rounds(
    guns: Query<&Gun>,
    sockets: Query<&Socket>,
    mut items: Query<&mut Item, With<Mag>>,
) {
    for gun in &guns {
        let (Some(well), Some(m)) = (gun.magwell, gun.firearm.magazine.as_ref()) else {
            continue;
        };
        if let Some(Ok(mut it)) = sockets
            .get(well)
            .ok()
            .and_then(|s| s.occupant)
            .map(|e| items.get_mut(e))
        {
            if it.rounds != m.rounds {
                it.rounds = m.rounds;
            }
        }
    }
}

/// A hand on a slide, charging handle, bolt or barrels works the action.
fn work_handles(
    hands: Query<(Entity, &Hand)>,
    mut guns: Query<(Entity, &mut Gun, &GlobalTransform, &Item)>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
    // Per hand: the gun, and its bolt travel and handle lift when the hand
    // took hold. The hand moves the part from there, so taking a slide that
    // is locked back, a bolt left open or a handle left up doesn't snap it
    // to where the part rests.
    mut taken: Local<std::collections::HashMap<Entity, (Entity, f32, f32)>>,
) {
    taken.retain(|h, (g, ..)| {
        hands
            .get(*h)
            .is_ok_and(|(_, hand)| hand.handle.map(|x| x.0) == Some(*g))
    });
    for (hand_e, hand) in &hands {
        let Some((gun_e, _)) = hand.handle else {
            continue;
        };
        let Ok((e, mut gun, gt, item)) = guns.get_mut(gun_e) else {
            continue;
        };
        let (_, start, start_lift) = *taken.entry(hand_e).or_insert((
            gun_e,
            gun.firearm.bolt_travel,
            gun.firearm.handle_lift,
        ));
        let Some(h) = gun.layout.handle.clone() else {
            continue;
        };
        let local = gt
            .affine()
            .inverse()
            .transform_point3(hand.world.translation);
        let d = local - hand.handle_origin;
        let events = match h.kind {
            HandleKind::Slide | HandleKind::ChargingHandle => {
                if d.z > 0.003 {
                    gun.firearm.take_off_stop();
                }
                let travel = (start + d.z / h.travel.max(1e-3)).clamp(0.0, 1.0);
                gun.firearm.set_bolt_travel(travel)
            }
            HandleKind::TurnBolt => {
                let mut ev = Vec::new();
                if gun.firearm.bolt_travel < 0.05 {
                    let lift = (start_lift + d.y / 0.035).clamp(0.0, 1.0);
                    ev.extend(gun.firearm.set_handle_lift(lift));
                }
                if gun.firearm.handle_lift >= 0.95 {
                    let travel = (start + d.z / h.travel.max(1e-3)).clamp(0.0, 1.0);
                    ev.extend(gun.firearm.set_bolt_travel(travel));
                }
                ev
            }
            HandleKind::BreakBarrels => {
                // Push the forend down to open; lift it to close.
                if d.y < -0.06 {
                    gun.firearm.set_break_open(true)
                } else if d.y > 0.02 {
                    gun.firearm.set_break_open(false)
                } else {
                    Vec::new()
                }
            }
        };
        let name = catalog.get(item.kind()).id.clone();
        emit(e, &name, &gun, gt, events, &mut out, &mut ejected);
    }
}

/// How fast a palm must come up into a seated magazine's floorplate to
/// seat it (the tap of tap-rack), m/s relative to the gun.
pub const TAP_SPEED: f32 = 0.7;
/// How near the floorplate the palm must be, metres.
pub const TAP_REACH: f32 = 0.1;

/// A hand slapped up on a seated magazine: a slipped one goes home.
fn tap_magazines(
    hands: Query<(Entity, &Hand)>,
    mut guns: Query<(
        Entity,
        &mut Gun,
        &GlobalTransform,
        &Item,
        &LinearVelocity,
        Option<&Held>,
    )>,
    sockets: Query<&Socket>,
    mags: Query<(&GlobalTransform, &Mag)>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
) {
    for (e, mut gun, gt, item, vel, held) in &mut guns {
        if !gun.firearm.mag_unseated {
            continue;
        }
        let Some((mgt, mag)) = gun
            .magwell
            .and_then(|w| sockets.get(w).ok())
            .and_then(|s| s.occupant)
            .and_then(|m| mags.get(m).ok())
        else {
            continue;
        };
        let up = gt.rotation() * Vec3::Y;
        let floor = mgt.translation() - mgt.rotation() * Vec3::Y * (mag.height * 0.5);
        let tapped = hands.iter().any(|(he, h)| {
            held.is_none_or(|held| held.hand != he)
                && h.holding.is_none()
                && h.world.translation.distance(floor) < TAP_REACH
                && (h.velocity - vel.0).dot(up) > TAP_SPEED
        });
        if tapped {
            let events = gun.firearm.tap_magazine();
            let name = catalog.get(item.kind()).id.clone();
            emit(e, &name, &gun, gt, events, &mut out, &mut ejected);
        }
    }
}

/// How near a gun's load port (its open breech, its clip guide) a held
/// round, box or clip must come, metres from the item's middle.
pub const LOAD_REACH: f32 = 0.07;
/// How far a clip in the guide must be pushed down to strip, metres.
pub const STRIP_PUSH: f32 = 0.02;
/// Seconds between rounds fed from a box held at the port.
pub const BOX_FEED: f32 = 0.3;

/// Rounds by hand: with the action open, a loose round or shell brought to
/// the load port goes in (and leaves the hand), a box held there feeds a
/// round at a time, and a stripper clip set in the guide and pushed down
/// strips its rounds into the magazine.
#[allow(clippy::too_many_arguments)]
fn feed_by_hand(
    mut commands: Commands,
    mut hands: Query<(Entity, &mut Hand)>,
    mut guns: Query<(Entity, &mut Gun, &GlobalTransform, &Item), Without<Mag>>,
    mut ammo: Query<(&mut Item, &GlobalTransform), Without<Gun>>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    // Per hand: (gun, the clip's height along the gun's up when it reached
    // the guide), and when a box last fed.
    mut clips: Local<std::collections::HashMap<Entity, (Entity, f32)>>,
    mut boxes: Local<std::collections::HashMap<Entity, f32>>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
) {
    let now = time.elapsed_secs();
    for (hand_e, mut hand) in &mut hands {
        let Some(held) = hand.holding else {
            clips.remove(&hand_e);
            continue;
        };
        let Ok((it, at)) = ammo.get(held) else {
            clips.remove(&hand_e);
            continue;
        };
        let def = catalog.get(it.kind());
        let (cartridge, clip) = match (&def.magazine, &def.cartridge) {
            (Some(m), _) if m.stripper => (m.cartridge.clone(), true),
            (None, Some(c)) if def.category == Category::Ammo => (c.clone(), false),
            _ => {
                clips.remove(&hand_e);
                continue;
            }
        };
        if it.rounds == 0 {
            continue;
        }
        let at = at.translation();
        let near = guns.iter_mut().find(|(_, g, gt, _)| {
            g.firearm.def.cartridge == cartridge
                && gt.transform_point(g.layout.load_port).distance(at) < LOAD_REACH
        });
        let Some((gun_e, mut gun, gt, gun_item)) = near else {
            clips.remove(&hand_e);
            continue;
        };
        let name = catalog.get(gun_item.kind()).id.clone();
        let mut events = Vec::new();
        let Ok((mut it, _)) = ammo.get_mut(held) else {
            continue;
        };
        if clip {
            // Down the guide: the clip's height in the gun's frame.
            let h = gt.affine().inverse().transform_point3(at).y;
            let entry = clips.entry(hand_e).or_insert((gun_e, h));
            if entry.0 != gun_e {
                *entry = (gun_e, h);
            }
            entry.1 = entry.1.max(h);
            if entry.1 - h >= STRIP_PUSH {
                let moved = gun.firearm.strip_clip(it.rounds);
                it.rounds -= moved;
                events.extend((0..moved).map(|_| FireEvent::RoundLoaded));
                clips.remove(&hand_e);
            }
        } else if def.rounds <= 1 {
            // A loose round: thumbed in, out of the hand.
            if gun.firearm.load_round() {
                events.push(FireEvent::RoundLoaded);
                hand.holding = None;
                commands.entity(held).despawn();
            }
        } else if boxes.get(&hand_e).is_none_or(|t| now - t >= BOX_FEED) {
            // A box at the port: a round at a time, as fast as fingers.
            if gun.firearm.load_round() {
                it.rounds -= 1;
                events.push(FireEvent::RoundLoaded);
                boxes.insert(hand_e, now);
            }
        }
        emit(gun_e, &name, &gun, gt, events, &mut out, &mut ejected);
    }
}

/// Releasing a spring-driven handle lets it slam home.
pub fn release_handle(gun: &mut Gun) -> Vec<FireEvent> {
    gun.firearm.release_bolt()
}

/// Face buttons and the selector, from the hand holding the gun.
fn buttons(
    players: Query<&RigInput, With<Player>>,
    mut guns: Query<(Entity, &mut Gun, &Held, &GlobalTransform, &Item)>,
    mut hands: Query<&mut Hand>,
    mut last: Local<std::collections::HashMap<Entity, (bool, bool, bool)>>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
    mut drop_mag: MessageWriter<DropMagazine>,
    catalog: Res<Catalog>,
) {
    for (e, mut gun, held, gt, item) in &mut guns {
        let Ok(input) = players.get(held.player) else {
            continue;
        };
        let hand = input.hand(held.side);
        let (p0, s0, c0) = last.get(&e).copied().unwrap_or_default();
        let (p, s, c) = (hand.primary, hand.secondary, hand.stick_click);
        last.insert(e, (p, s, c));
        let name = catalog.get(item.kind()).id.clone();
        let mut events = Vec::new();
        if p && !p0 {
            match gun.firearm.def.action {
                ActionKind::BreakAction => {
                    let open = gun.firearm.bolt_travel < 0.5;
                    events.extend(gun.firearm.set_break_open(open));
                }
                _ => {
                    drop_mag.write(DropMagazine { gun: e });
                }
            }
        }
        if s && !s0 {
            events.extend(gun.firearm.release_slide_stop());
        }
        if c && !c0 {
            events.extend(gun.firearm.cycle_mode());
        }
        emit(e, &name, &gun, gt, events, &mut out, &mut ejected);
        // A hand that let go of the handle this tick: spring return.
        let _ = &mut hands;
    }
}

/// Mag release: the magazine falls out of the well.
#[derive(Message, Clone, Copy, Debug)]
pub struct DropMagazine {
    pub gun: Entity,
}

/// A round or case leaving the port.
#[derive(Message, Clone, Debug)]
pub struct Ejection {
    pub round: Round,
    pub cartridge: String,
    pub at: Vec3,
    pub velocity: Vec3,
}

fn triggers(
    players: Query<&RigInput, With<Player>>,
    mut guns: Query<(Entity, &mut Gun, Option<&Held>, &GlobalTransform, &Item)>,
    time: Res<Time>,
    mut rng: ResMut<GunRng>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
    mut shots: MessageWriter<ballistics::Shot>,
    catalog: Res<Catalog>,
) {
    let dt = time.delta_secs();
    for (e, mut gun, held, gt, item) in &mut guns {
        let trigger = held
            .and_then(|h| players.get(h.player).ok().map(|i| i.hand(h.side).trigger))
            .unwrap_or(0.0);
        gun.trigger = trigger;
        let events = gun.firearm.update(dt, trigger > 0.55, &mut || rng.0.f32());
        let name = catalog.get(item.kind()).id.clone();
        for ev in &events {
            if let FireEvent::Fired { barrel } = ev {
                let def = &gun.firearm.def;
                let mut muzzle = gun.layout.muzzle;
                if def.barrels > 1 {
                    muzzle.x += if *barrel == 0 { -0.0128 } else { 0.0128 };
                }
                shots.write(ballistics::Shot {
                    gun: e,
                    shooter: held.map(|h| h.player),
                    origin: gt.transform_point(muzzle),
                    direction: gt.rotation() * Vec3::NEG_Z,
                    speed: def.muzzle_velocity,
                    damage: def.damage,
                    projectiles: def.projectiles,
                    spread: gun.firearm.spread_radians(),
                    recoil: def.recoil,
                    seed: rng.0.next_u64(),
                });
            }
        }
        emit(e, &name, &gun, gt, events, &mut out, &mut ejected);
    }
}

/// Turns mechanism events into gun events and ejections.
fn emit(
    gun_e: Entity,
    name: &str,
    gun: &Gun,
    gt: &GlobalTransform,
    events: Vec<FireEvent>,
    out: &mut MessageWriter<GunEvent>,
    ejected: &mut MessageWriter<Ejection>,
) {
    for ev in events {
        if let FireEvent::Ejected(round) = ev {
            let dir = gt.rotation() * gun.layout.eject.normalize_or_zero();
            ejected.write(Ejection {
                round,
                cartridge: gun.firearm.def.cartridge.clone(),
                at: gt.transform_point(gun.layout.port),
                velocity: dir * 2.6,
            });
        }
        out.write(GunEvent {
            gun: gun_e,
            item: name.to_owned(),
            event: ev,
            at: gt.translation(),
        });
    }
}

fn age_casings(mut commands: Commands, mut casings: Query<(Entity, &mut Casing)>, time: Res<Time>) {
    for (e, mut c) in &mut casings {
        c.life -= time.delta_secs();
        if c.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

/// Spawns what came out of the port: a brass case, or a live round you can
/// pick back up.
pub fn spawn_ejections(
    mut commands: Commands,
    mut ejections: MessageReader<Ejection>,
    catalog: Res<Catalog>,
) {
    for ej in ejections.read() {
        let spin = Vec3::new(ej.velocity.z, 0.0, -ej.velocity.x) * 8.0;
        match ej.round {
            Round::Spent => {
                commands.spawn((
                    Name::new("Case"),
                    Casing { life: 40.0 },
                    Visibility::default(),
                    Transform::from_translation(ej.at)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    RigidBody::Dynamic,
                    Collider::cylinder(0.005, 0.022),
                    Mass(0.005),
                    NoAutoMass,
                    CollisionLayers::new(Layer::Item, [Layer::World]),
                    LinearVelocity(ej.velocity),
                    AngularVelocity(spin),
                    Restitution::new(0.4),
                ));
            }
            Round::Live => {
                let id = match ej.cartridge.as_str() {
                    "9x18" => "round_9x18",
                    "5.45x39" => "round_545",
                    "7.62x54R" => "round_762r",
                    "7.62x39" => "round_762x39",
                    "12ga" => "shell_12ga",
                    _ => continue,
                };
                let Some(kind) = catalog.kind(id) else {
                    continue;
                };
                let e = spawn_item(
                    &mut commands,
                    &catalog,
                    SpawnItem {
                        kind,
                        transform: Transform::from_translation(ej.at),
                        condition: 1.0,
                    },
                );
                commands
                    .entity(e)
                    .insert((LinearVelocity(ej.velocity), AngularVelocity(spin)));
            }
        }
    }
}

/// Mag release: unseat the magazine and let it fall.
pub fn drop_magazines(
    mut commands: Commands,
    mut requests: MessageReader<DropMagazine>,
    mut guns: Query<(&mut Gun, &LinearVelocity)>,
    mut sockets: Query<&mut Socket>,
    mut mags: Query<(&mut Item, &GlobalTransform), With<Mag>>,
    mut out: MessageWriter<GunEvent>,
) {
    for req in requests.read() {
        let Ok((mut gun, vel)) = guns.get_mut(req.gun) else {
            continue;
        };
        let Some(well) = gun.magwell else {
            continue;
        };
        let Ok(mut socket) = sockets.get_mut(well) else {
            continue;
        };
        let Some(mag_e) = socket.occupant.take() else {
            continue;
        };
        let removed = gun.firearm.remove_magazine();
        let Ok((mut it, gt)) = mags.get_mut(mag_e) else {
            continue;
        };
        if let Some(m) = removed {
            it.rounds = m.rounds;
        }
        let world = gt.compute_transform();
        crate::interact::physics_on(
            &mut commands,
            mag_e,
            world,
            vel.0 + world.down() * 0.6,
            Vec3::ZERO,
            true,
        );
        out.write(GunEvent {
            gun: req.gun,
            item: String::new(),
            event: FireEvent::MagazineRemoved,
            at: world.translation,
        });
    }
}

/// Handles let go: spring-driven slides and carriers slam home.
pub fn handle_released(
    mut events: MessageReader<InteractEvent>,
    hands: Query<&Hand>,
    mut guns: Query<(Entity, &mut Gun, &GlobalTransform, &Item)>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
    mut watching: Local<Vec<Entity>>,
) {
    // Guns whose handle is held this tick.
    let held_handles: Vec<Entity> = hands
        .iter()
        .filter_map(|h| h.handle.map(|(g, _)| g))
        .collect();
    for g in watching.clone() {
        if !held_handles.contains(&g) {
            if let Ok((e, mut gun, gt, item)) = guns.get_mut(g) {
                let ev = release_handle(&mut gun);
                let name = catalog.get(item.kind()).id.clone();
                emit(e, &name, &gun, gt, ev, &mut out, &mut ejected);
            }
        }
    }
    *watching = held_handles;
    events.clear();
}

/// Fire-mode name for HUDs.
pub fn mode_name(mode: FireMode) -> &'static str {
    match mode {
        FireMode::Safe => "safe",
        FireMode::Semi => "semi",
        FireMode::Auto => "auto",
    }
}

/// Registers the systems that need `Commands` and run after the mechanism.
pub struct WeaponEffectsPlugin;

impl Plugin for WeaponEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Ejection>()
            .add_message::<DropMagazine>()
            .add_systems(
                FixedUpdate,
                (
                    handle_released,
                    drop_magazines,
                    spawn_ejections,
                    feel::kick_guns,
                )
                    .chain()
                    .after(triggers),
            );
    }
}

/// Brass on spent cases (client side).
pub fn casing_visuals(
    mut commands: Commands,
    added: Query<Entity, Added<Casing>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut brass: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    for e in &added {
        let (mesh, mat) = brass
            .get_or_insert_with(|| {
                (
                    meshes.add(Cylinder::new(0.0048, 0.022)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.78, 0.6, 0.3),
                        metallic: 1.0,
                        perceptual_roughness: 0.35,
                        ..default()
                    }),
                )
            })
            .clone();
        commands
            .entity(e)
            .insert((Mesh3d(mesh), MeshMaterial3d(mat)));
    }
}

/// True for items that are guns (by catalog category), for UI.
pub fn is_gun(catalog: &Catalog, item: &Item) -> bool {
    catalog.get(item.kind()).category == Category::Firearm
}

/// Rig hands and the guns they hold: used by flat macros and the HUD.
pub fn held_gun<'a>(hands: &'a Query<(&RigHand, &Hand)>, side: crate::rig::Side) -> Option<Entity> {
    hands
        .iter()
        .find(|(rh, _)| rh.side == side)
        .and_then(|(_, h)| h.holding)
}
