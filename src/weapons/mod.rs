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

pub mod armour;
pub mod armoury;
pub mod ballistics;
pub mod carry;
pub mod crossbow;
pub mod faults;
pub mod feel;
pub mod flares;
pub mod flat;
pub mod grenades;
pub mod gunsmith;
pub mod impacts;
pub mod layout;
pub mod melee;
pub mod muzzle;
pub mod optics;
pub mod range;
pub mod report;
pub mod reverb;
#[cfg(test)]
pub mod signature;
pub mod sling;
pub mod tracers;
pub mod visuals;
pub mod wear;
pub mod weather;
pub mod zero;

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
    /// The gun's own dice (jams, duds, spread, its kick's yaw), seeded from
    /// what it is and where it first lay ([`gun_seed`]): what happens to
    /// one gun never hangs on how many others are lying about, nor on the
    /// order things were spawned in, and a drill replays.
    pub dice: Rng,
    /// Its mass as built, if it's been worked at the vice (`gunsmith`);
    /// else the catalog's.
    pub mass: Option<f32>,
}

impl Gun {
    /// Its mass (kg): as built, or `catalog` as made.
    pub fn mass(&self, catalog: f32) -> f32 {
        self.mass.unwrap_or(catalog)
    }
}

/// A gun's seed: its kind and where it was first put (to the centimetre).
pub fn gun_seed(id: &str, at: Vec3) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    for c in at.to_array() {
        h = (h ^ ((c * 100.0).round() as i64 as u64)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A magazine item: what it fits and holds. Rounds live in `Item.rounds`.
#[derive(Component, Clone, Debug)]
pub struct Mag {
    pub family: String,
    pub capacity: u32,
    pub height: f32,
}

/// A spent case or hull thrown from the gun: it tumbles, lands with a
/// clink ([`CasingLanded`]) and lies there a while ([`CASING_LIFE`]); the
/// oldest goes once there are [`CASING_CAP`].
#[derive(Component)]
pub struct Casing {
    pub life: f32,
    /// Radius and length, metres.
    pub size: (f32, f32),
    /// A shotgun hull rather than brass.
    pub hull: bool,
    /// Vertical speed last tick, and landings so far.
    fall: f32,
    bounces: u8,
}

/// How long a case lies on the ground, seconds.
pub const CASING_LIFE: f32 = 120.0;
/// Cases on the ground at once.
pub const CASING_CAP: usize = 64;

/// What a case landed on, for its clink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CasingFloor {
    Metal,
    Wood,
    /// Concrete, asphalt, rock, gravel.
    Hard,
    /// Grass, earth, mud, the forest floor.
    Soft,
    Water,
}

/// A case hit the floor (each of its first few bounces), for the clink.
#[derive(Message, Clone, Copy, Debug)]
pub struct CasingLanded {
    pub at: Vec3,
    pub floor: CasingFloor,
    /// How fast it came down, m/s.
    pub speed: f32,
    /// Its first landing (later ones are the bounces, quieter).
    pub first: bool,
    pub hull: bool,
}

/// Something a gun did, with where it did it: for audio and effects.
#[derive(Message, Clone, Debug)]
pub struct GunEvent {
    pub gun: Entity,
    pub item: String,
    pub event: FireEvent,
    pub at: Vec3,
}

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
        // The gun held by its grip, in either hand (a left-handed player's
        // is in the left); the right's first if both hands hold one.
        let gun = crate::rig::Side::BOTH.iter().rev().find_map(|side| {
            children.iter().find_map(|c| {
                hands
                    .get(c)
                    .ok()
                    .filter(|(rh, h)| rh.side == *side && h.supporting.is_none())
                    .and_then(|(_, h)| h.holding)
                    .and_then(|e| guns.get(e).ok())
            })
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
            .add_message::<ActionGrind>()
            .add_observer(setup_gun_items)
            .add_observer(|add: On<Add, Player>, mut commands: Commands| {
                commands.entity(add.entity).insert(AimHint::default());
            })
            .add_systems(
                PreUpdate,
                (
                    update_aim_hints.before(crate::rig::RigDriverSystems),
                    fit_gun_colliders,
                ),
            )
            .add_plugins((
                armour::ArmourPlugin,
                ballistics::BallisticsPlugin,
                zero::ZeroPlugin,
                faults::FaultsPlugin,
                optics::OpticsPlugin,
                feel::FeelPlugin,
                impacts::ImpactPlugin,
                grenades::GrenadePlugin,
                melee::MeleePlugin,
                armoury::ArmouryPlugin,
                flares::FlaresPlugin,
                crossbow::CrossbowPlugin,
                carry::CarryPlugin,
                weather::WeatherPlugin,
                report::ReportPlugin,
            ))
            .add_plugins(gunsmith::GunsmithPlugin)
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
    places: Query<&Transform>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    let def = catalog.get(item.kind());
    let first_at = places.get(add.entity).map_or(Vec3::ZERO, |t| t.translation);
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
    let seed = gun_seed(&def.id, first_at);
    let mut firearm = Firearm::new(fdef.clone(), item.condition);
    firearm.fault_dice = seed ^ 0xFA17;
    let mut points = vec![GrabPoint::new(layout.grip, GripRole::Primary, 0.12)];
    if let Some(s) = layout.support {
        points.push(GrabPoint::new(
            Transform::from_translation(s),
            GripRole::Support,
            0.14,
        ));
    }
    // A handgun: the other palm cupped under the grip steadies it (it
    // doesn't aim it: `interact::follow_hands`).
    if let Some(c) = layout::cup(&layout) {
        points.push(GrabPoint::new(
            Transform::from_translation(c),
            GripRole::Support,
            0.08,
        ));
    }
    // (A pump's forend is the support grip: no handle of its own.)
    if let Some(h) = layout
        .handle
        .as_ref()
        .filter(|h| h.kind != HandleKind::Pump)
    {
        points.push(GrabPoint::new(
            Transform::from_translation(h.grab),
            GripRole::Handle(0),
            0.07,
        ));
    }
    // (Its collider is fitted once it's a gun: `fit_gun_colliders`.)
    let mut e = commands.entity(add.entity);
    e.insert((CustomVisual, Grabbable { points }));
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
        dice: Rng::new(seed),
        mass: None,
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
                    loads: it.loads.clone(),
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
                        it.loads = m.loads;
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
                it.loads = m.loads.clone();
            }
        }
    }
}

/// A hand on a slide, charging handle, bolt or barrels works the action.
/// A stiff action (cold oil, `Firearm::stiff`) in the hand: its part
/// follows the hand no faster than [`STROKE_RATE`] m/s, slowed by
/// [`STROKE_DRAG`] times the stiffness, and not at all unless the hand pulls
/// at [`FIRM_PULL`] m/s times the stiffness (a timid pull just slides on
/// it). A stiff bolt brought back short and pushed home again has
/// short-stroked ([`SHORT_STIFF`] and up).
pub const STROKE_RATE: f32 = 3.0;
pub const STROKE_DRAG: f32 = 3.0;
pub const FIRM_PULL: f32 = 0.35;
pub const SHORT_STIFF: f32 = 0.3;

/// Where a hand-worked part goes this tick, from `now` toward `want`
/// (travel, 0..1, over `travel_m` metres), as stiff as the action is and as
/// hard as the hand pulls (m/s along the stroke).
pub fn drag(now: f32, want: f32, stiff: f32, pull: f32, dt: f32, travel_m: f32) -> f32 {
    if stiff <= 0.01 {
        return want;
    }
    if pull.abs() < FIRM_PULL * stiff {
        return now;
    }
    let max = dt * STROKE_RATE / (1.0 + STROKE_DRAG * stiff) / travel_m.max(1e-3);
    now + (want - now).clamp(-max, max)
}

#[allow(clippy::too_many_arguments)]
fn work_handles(
    hands: Query<(Entity, &Hand)>,
    rig: Query<&RigHand>,
    mut guns: Query<(Entity, &mut Gun, &GlobalTransform, &Item)>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
    mut grinds: MessageWriter<ActionGrind>,
    // Per gun: the furthest its bolt came back this stroke (for a short
    // stroke), and when it last ground under a hand.
    mut strokes: Local<std::collections::HashMap<Entity, (f32, f32)>>,
    // Per hand: the gun, and its bolt travel and handle lift when the hand
    // took hold. The hand moves the part from there, so taking a slide that
    // is locked back, a bolt left open or a handle left up doesn't snap it
    // to where the part rests.
    mut taken: Local<std::collections::HashMap<Entity, (Entity, f32, f32)>>,
    // Per supporting hand on a pump: the gun, the forend's travel and the
    // hand's place along the bore when it took hold.
    mut pumping: Local<std::collections::HashMap<Entity, (Entity, f32, f32)>>,
) {
    let dt = time.delta_secs();
    let now_s = time.elapsed_secs();
    pumping.retain(|h, (g, ..)| {
        hands
            .get(*h)
            .is_ok_and(|(_, hand)| hand.supporting == Some(*g))
    });
    strokes.retain(|g, _| guns.contains(*g));
    for (hand_e, hand) in &hands {
        let Some(gun_e) = hand.supporting else {
            continue;
        };
        let Ok((e, mut gun, gt, item)) = guns.get_mut(gun_e) else {
            continue;
        };
        let Some(h) = gun
            .layout
            .handle
            .clone()
            .filter(|h| h.kind == HandleKind::Pump)
        else {
            continue;
        };
        let z = gt
            .affine()
            .inverse()
            .transform_point3(hand.world.translation)
            .z;
        let travel = gun.firearm.bolt_travel;
        let (_, start, z0) = *pumping.entry(hand_e).or_insert((gun_e, travel, z));
        let want = start + (z - z0) / h.travel.max(1e-3);
        // Cold oil: it drags, and a timid hand slides on it.
        let pull = hand.velocity.dot(gt.rotation() * Vec3::Z);
        let stiff = gun.firearm.stiff;
        let to = drag(travel, want.clamp(0.0, 1.0), stiff, pull, dt, h.travel);
        let events = gun.firearm.set_bolt_travel(to);
        grind(
            &mut haptics,
            &mut grinds,
            gun_e,
            hand.world.translation,
            rig.get(hand_e).ok().map(|r| r.side),
            stiff,
            to != travel,
            now_s,
            strokes.entry(gun_e).or_default(),
        );
        let now = gun.firearm.bolt_travel;
        if (now - want).abs() > 1e-3 {
            // At a stop, held shut by the lock or stopped on a jam: the
            // hand slides on the forend from where it is.
            pumping.insert(hand_e, (gun_e, now, z));
        }
        let name = catalog.get(item.kind()).id.clone();
        emit(e, &name, &mut gun, gt, events, &mut out, &mut ejected);
    }
    taken.retain(|h, (g, ..)| {
        hands
            .get(*h)
            .is_ok_and(|(_, hand)| hand.handle.map(|x| x.0) == Some(*g))
    });
    for (hand_e, hand) in &hands {
        // (A scope's turret is worked in `zero`.)
        let Some((gun_e, 0)) = hand.handle else {
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
        // How hard the hand pulls along the stroke, and which hand.
        let pull = hand.velocity.dot(gt.rotation() * Vec3::Z);
        let side = rig.get(hand_e).ok().map(|r| r.side);
        let events = match h.kind {
            HandleKind::String => {
                let travel = (start + d.z / h.travel.max(1e-3)).clamp(0.0, 1.0);
                gun.firearm.set_bolt_travel(travel)
            }
            HandleKind::Slide | HandleKind::ChargingHandle => {
                if d.z > 0.003 {
                    gun.firearm.take_off_stop();
                }
                let want = (start + d.z / h.travel.max(1e-3)).clamp(0.0, 1.0);
                let was = gun.firearm.bolt_travel;
                let stiff = gun.firearm.stiff;
                let to = drag(was, want, stiff, pull, dt, h.travel);
                if to != want {
                    // The hand slides on the handle from where it got to.
                    taken.insert(hand_e, (gun_e, to - d.z / h.travel.max(1e-3), start_lift));
                }
                grind(
                    &mut haptics,
                    &mut grinds,
                    gun_e,
                    hand.world.translation,
                    side,
                    stiff,
                    to != was,
                    now_s,
                    strokes.entry(gun_e).or_default(),
                );
                gun.firearm.set_bolt_travel(to)
            }
            HandleKind::TurnBolt => {
                let mut ev = Vec::new();
                // A stuck bolt: only a palm slapped up under the handle
                // breaks it free.
                if gun.firearm.bolt_stuck
                    && hand.velocity.dot(gt.rotation() * Vec3::Y) > faults::STICKY_SLAP
                {
                    ev.extend(gun.firearm.free_bolt());
                }
                if gun.firearm.bolt_travel < 0.05 {
                    let lift = (start_lift + d.y / 0.035).clamp(0.0, 1.0);
                    ev.extend(gun.firearm.set_handle_lift(lift));
                }
                if gun.firearm.handle_lift >= 0.95 {
                    let want = (start + d.z / h.travel.max(1e-3)).clamp(0.0, 1.0);
                    let was = gun.firearm.bolt_travel;
                    let stiff = gun.firearm.stiff;
                    let to = drag(was, want, stiff, pull, dt, h.travel);
                    if to != want {
                        taken.insert(hand_e, (gun_e, to - d.z / h.travel.max(1e-3), start_lift));
                    }
                    let stroke = strokes.entry(gun_e).or_default();
                    grind(
                        &mut haptics,
                        &mut grinds,
                        gun_e,
                        hand.world.translation,
                        side,
                        stiff,
                        to != was,
                        now_s,
                        stroke,
                    );
                    ev.extend(gun.firearm.set_bolt_travel(to));
                    // Came back short and going home again: a short stroke.
                    let now = gun.firearm.bolt_travel;
                    if now > was {
                        stroke.0 = stroke.0.max(now);
                    } else if now < was && stiff >= SHORT_STIFF && (0.1..0.95).contains(&stroke.0) {
                        ev.extend(gun.firearm.short_stroke());
                        stroke.0 = 0.0;
                    }
                    if now < 0.05 || now >= 0.98 {
                        stroke.0 = 0.0;
                    }
                }
                ev
            }
            // (Worked by the supporting hand, above.)
            HandleKind::Pump => Vec::new(),
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
        emit(e, &name, &mut gun, gt, events, &mut out, &mut ejected);
    }
}

/// A stiff action ground under a hand (for Audio): about every 60 ms while
/// it moves.
#[derive(Message, Clone, Copy, Debug)]
pub struct ActionGrind {
    pub gun: Entity,
    pub at: Vec3,
    pub stiff: f32,
}

/// A stiff action grinds in the hand that works it: a tick as it moves,
/// harder the stiffer, and heard.
#[allow(clippy::too_many_arguments)]
fn grind(
    haptics: &mut MessageWriter<crate::rig::haptics::Haptic>,
    grinds: &mut MessageWriter<ActionGrind>,
    gun: Entity,
    at: Vec3,
    side: Option<crate::rig::Side>,
    stiff: f32,
    moved: bool,
    now: f32,
    stroke: &mut (f32, f32),
) {
    let Some(side) = side else {
        return;
    };
    if stiff < 0.1 || !moved || now - stroke.1 < 0.06 {
        return;
    }
    stroke.1 = now;
    haptics.write(crate::rig::haptics::Haptic {
        side,
        amplitude: 0.08 + 0.3 * stiff,
        seconds: 0.015,
    });
    grinds.write(ActionGrind { gun, at, stiff });
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
            emit(e, &name, &mut gun, gt, events, &mut out, &mut ejected);
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
                let n = it.rounds;
                let moved = gun.firearm.strip_clip_of(n, &mut it.loads);
                it.rounds -= moved;
                events.extend((0..moved).map(|_| FireEvent::RoundLoaded));
                clips.remove(&hand_e);
            }
        } else if def.rounds <= 1 {
            // A loose round: thumbed in, out of the hand.
            let load = it.loads.pop_or(1, def.load);
            if gun.firearm.load_round_of(load) {
                events.push(FireEvent::RoundLoaded);
                hand.holding = None;
                commands.entity(held).despawn();
            }
        } else if boxes.get(&hand_e).is_none_or(|t| now - t >= BOX_FEED) {
            // A box at the port: a round at a time, as fast as fingers.
            let mut left = it.loads.clone();
            let load = left.pop_or(it.rounds, def.load);
            if gun.firearm.load_round_of(load) {
                it.loads = left;
                it.rounds -= 1;
                events.push(FireEvent::RoundLoaded);
                boxes.insert(hand_e, now);
            }
        }
        emit(gun_e, &name, &mut gun, gt, events, &mut out, &mut ejected);
    }
}

/// Releasing a spring-driven handle lets it slam home.
pub fn release_handle(gun: &mut Gun) -> Vec<FireEvent> {
    gun.firearm.release_bolt()
}

/// Face buttons and the selector, from the hand holding the gun.
fn buttons(
    players: Query<&RigInput, With<Player>>,
    mut macros: Query<&mut flat::GunMacro>,
    mut guns: Query<(Entity, &mut Gun, &Held, &GlobalTransform, &Item)>,
    mut hands: Query<&mut Hand>,
    mut last: Local<std::collections::HashMap<Entity, (bool, bool, bool)>>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
    mut drop_mag: MessageWriter<DropMagazine>,
    catalog: Res<Catalog>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut racks: Local<Vec<(Entity, Entity)>>,
) {
    let one_handed = comfort.as_ref().is_some_and(|c| c.one_handed);
    racks.clear();
    // Only guns in hand: every gun ever held stayed in here.
    last.retain(|e, _| guns.contains(*e));
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
        let revolver = gun.firearm.def.action == ActionKind::Revolver;
        if p && !p0 {
            match gun.firearm.def.action {
                ActionKind::BreakAction => {
                    let open = gun.firearm.bolt_travel < 0.5;
                    events.extend(gun.firearm.set_break_open(open));
                }
                // A pump's: the action release.
                ActionKind::Pump => {
                    events.extend(gun.firearm.release_action());
                }
                // A revolver's thumb button swings the loading gate.
                ActionKind::Revolver => {
                    let open = !gun.firearm.gate_open;
                    events.extend(gun.firearm.set_gate(open));
                }
                _ => {
                    drop_mag.write(DropMagazine { gun: e });
                }
            }
        }
        if s && !s0 {
            if revolver {
                // Gate open: turn the cylinder a chamber; shut: thumb the
                // hammer back.
                if gun.firearm.gate_open {
                    events.extend(gun.firearm.rotate_cylinder());
                } else {
                    events.extend(gun.firearm.cock_hammer());
                }
            } else if one_handed
                && !gun.firearm.locked_back
                && gun.firearm.def.action != ActionKind::BreakAction
            {
                // One hand: the button racks it (there's no other hand for
                // the slide); locked back, it lets the slide go as ever.
                racks.push((held.player, e));
            } else {
                events.extend(gun.firearm.release_slide_stop());
            }
        }
        if c && !c0 {
            if revolver {
                // The ejector rod, at the gate.
                events.extend(gun.firearm.eject_at_gate());
            } else {
                events.extend(gun.firearm.cycle_mode());
            }
        }
        emit(e, &name, &mut gun, gt, events, &mut out, &mut ejected);
        // A hand that let go of the handle this tick: spring return.
        let _ = &mut hands;
    }
    for (player, gun) in racks.drain(..) {
        let (Ok(g), Ok(mut mac)) = (guns.get(gun), macros.get_mut(player)) else {
            continue;
        };
        if !mac.busy() {
            mac.start(gun, flat::macro_steps(flat::MacroKind::Rack, &g.1));
        }
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
    /// A live round's load (which loose round it lands as).
    pub load: extraction_core::ammo::Load,
    pub at: Vec3,
    pub velocity: Vec3,
}

/// The loose round of `cartridge` loaded with `load` (ball if there's no
/// such round).
pub fn loose_round(
    catalog: &extraction_core::items::ItemCatalog,
    cartridge: &str,
    load: extraction_core::ammo::Load,
) -> Option<extraction_core::items::ItemKind> {
    let find = |load| {
        catalog
            .iter()
            .find(|(_, d)| {
                d.category == Category::Ammo
                    && d.rounds == 1
                    && d.magazine.is_none()
                    && d.cartridge.as_deref() == Some(cartridge)
                    && d.load == load
            })
            .map(|(k, _)| k)
    };
    find(load).or_else(|| find(extraction_core::ammo::Load::Standard))
}

/// A gun's collider: the bounds of its blockout, as one box where the gun
/// is (not the catalog's box about its origin, which `spawn_item` gives
/// every item and which left an AK's last 20 cm of barrel going into walls
/// and a phantom 20 cm behind its butt). Fitted after the spawn's own
/// physics have gone in, so it isn't overwritten.
pub fn gun_collider(layout: &GunLayout) -> Collider {
    let (lo, hi) = layout.bounds();
    let centre = (lo + hi) * 0.5;
    let size = (hi - lo).max(Vec3::splat(0.01));
    Collider::compound(vec![(
        centre,
        Quat::IDENTITY,
        Collider::cuboid(size.x, size.y, size.z),
    )])
}

fn fit_gun_colliders(mut commands: Commands, guns: Query<(Entity, &Gun), Added<Gun>>) {
    for (e, gun) in &guns {
        commands.entity(e).try_insert(gun_collider(&gun.layout));
    }
}

/// Where a shot leaves from: the muzzle, unless something solid lies
/// between the grip and the muzzle (a gun pushed against a wall, its muzzle
/// in or past it), when it leaves from just this side of that and spends
/// itself there: you can't shoot through a wall from behind it.
fn muzzle_clear(spatial: &SpatialQuery, gt: &GlobalTransform, grip: Vec3, muzzle: Vec3) -> Vec3 {
    let (from, to) = (gt.transform_point(grip), gt.transform_point(muzzle));
    let Ok(dir) = Dir3::new(to - from) else {
        return to;
    };
    match spatial.cast_ray(
        from,
        dir,
        from.distance(to),
        true,
        &SpatialQueryFilter::from_mask(crate::world::Layer::World),
    ) {
        Some(hit) => from + dir * (hit.distance - 0.002).max(0.0),
        None => to,
    }
}

fn triggers(
    players: Query<&RigInput, With<Player>>,
    mut guns: Query<(Entity, &mut Gun, Option<&Held>, &GlobalTransform, &Item)>,
    time: Res<Time>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
    mut shots: MessageWriter<ballistics::Shot>,
    catalog: Res<Catalog>,
    spatial: SpatialQuery,
) {
    let dt = time.delta_secs();
    for (e, mut gun, held, gt, item) in &mut guns {
        let trigger = held
            .and_then(|h| players.get(h.player).ok().map(|i| i.hand(h.side).trigger))
            .unwrap_or(0.0);
        gun.trigger = trigger;
        let events = {
            let Gun { firearm, dice, .. } = &mut *gun;
            firearm.update(dt, trigger > 0.55, &mut || dice.f32())
        };
        let name = catalog.get(item.kind()).id.clone();
        for ev in &events {
            if let FireEvent::Fired { barrel } = ev {
                let def = &gun.firearm.def;
                let mut muzzle = gun.layout.muzzle;
                if def.barrels > 1 {
                    muzzle.x += if *barrel == 0 { -0.0128 } else { 0.0128 };
                }
                let origin = muzzle_clear(&spatial, gt, gun.layout.grip.translation, muzzle);
                shots.write(ballistics::Shot {
                    gun: e,
                    shooter: held.map(|h| h.player),
                    origin,
                    direction: gt.rotation() * Vec3::NEG_Z,
                    speed: def.muzzle_velocity,
                    damage: def.damage,
                    projectiles: def.projectiles,
                    spread: gun.firearm.spread_radians(),
                    recoil: def.recoil,
                    seed: gun.dice.next_u64(),
                });
            }
        }
        emit(e, &name, &mut gun, gt, events, &mut out, &mut ejected);
    }
}

/// Turns mechanism events into gun events and ejections.
fn emit(
    gun_e: Entity,
    name: &str,
    gun: &mut Gun,
    gt: &GlobalTransform,
    events: Vec<FireEvent>,
    out: &mut MessageWriter<GunEvent>,
    ejected: &mut MessageWriter<Ejection>,
) {
    for ev in events {
        if let FireEvent::Ejected(round) = ev {
            let dir = gt.rotation() * gun.layout.eject.normalize_or_zero();
            let load = if round == Round::Live && !gun.firearm.ejected.is_empty() {
                gun.firearm.ejected.remove(0)
            } else {
                Default::default()
            };
            ejected.write(Ejection {
                round,
                cartridge: gun.firearm.def.cartridge.clone(),
                load,
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

/// A case whose fall stopped or turned has landed: say where, on what.
fn land_casings(
    mut casings: Query<(&mut Casing, &LinearVelocity, &GlobalTransform)>,
    spatial: SpatialQuery,
    built: Query<(&GlobalTransform, &crate::world::surfaces::Surfaces)>,
    zone: Option<Res<crate::world::Zone>>,
    mut out: MessageWriter<CasingLanded>,
) {
    for (mut c, v, gt) in &mut casings {
        let was = c.fall;
        c.fall = v.y;
        if was > -0.8 || v.y < was * 0.3 || c.bounces >= 3 {
            continue;
        }
        c.bounces += 1;
        let at = gt.translation();
        let floor = match crate::world::surfaces::underfoot(&spatial, &built, at) {
            Some(m) => match impacts::kind_of(m) {
                impacts::ImpactKind::Metal => CasingFloor::Metal,
                impacts::ImpactKind::Wood => CasingFloor::Wood,
                _ => CasingFloor::Hard,
            },
            None => {
                use extraction_core::terrain::Surface;
                match zone.as_ref().map(|z| z.hm.surface([at.x, at.z], false)) {
                    Some(Surface::Water) => CasingFloor::Water,
                    Some(Surface::Rock | Surface::Gravel | Surface::Asphalt) => CasingFloor::Hard,
                    _ => CasingFloor::Soft,
                }
            }
        };
        out.write(CasingLanded {
            at,
            floor,
            speed: -was,
            first: c.bounces == 1,
            hull: c.hull,
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
    mut pool: Local<std::collections::VecDeque<Entity>>,
) {
    for ej in ejections.read() {
        let spin = Vec3::new(ej.velocity.z, 0.0, -ej.velocity.x) * 8.0;
        match ej.round {
            Round::Spent => {
                let (r, len) = feel::case_size(&ej.cartridge);
                let e = commands
                    .spawn((
                        Name::new("Case"),
                        Casing {
                            life: CASING_LIFE,
                            size: (r, len),
                            hull: ej.cartridge == "12ga",
                            fall: 0.0,
                            bounces: 0,
                        },
                        Visibility::default(),
                        Transform::from_translation(ej.at)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                        RigidBody::Dynamic,
                        Collider::cylinder(r, len),
                        Mass(0.005),
                        NoAutoMass,
                        CollisionLayers::new(Layer::Item, [Layer::World]),
                        LinearVelocity(ej.velocity),
                        AngularVelocity(spin),
                        Restitution::new(0.4),
                    ))
                    .id();
                pool.push_back(e);
                if pool.len() > CASING_CAP {
                    if let Some(old) = pool.pop_front() {
                        commands.entity(old).try_despawn();
                    }
                }
            }
            Round::Live => {
                let Some(kind) = loose_round(&catalog, &ej.cartridge, ej.load) else {
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
            it.loads = m.loads;
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
        .filter_map(|h| h.handle.filter(|(_, id)| *id == 0).map(|(g, _)| g))
        .collect();
    for g in watching.clone() {
        if !held_handles.contains(&g) {
            if let Ok((e, mut gun, gt, item)) = guns.get_mut(g) {
                let ev = release_handle(&mut gun);
                let name = catalog.get(item.kind()).id.clone();
                emit(e, &name, &mut gun, gt, ev, &mut out, &mut ejected);
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
            .add_message::<CasingLanded>()
            .add_systems(
                FixedUpdate,
                (
                    handle_released,
                    drop_magazines,
                    spawn_ejections,
                    land_casings,
                    feel::kick_guns,
                )
                    .chain()
                    .after(triggers),
            );
    }
}

/// Brass on spent cases, a red hull on spent shells (client side): one
/// unit cylinder scaled to each, so they batch.
pub fn casing_visuals(
    mut commands: Commands,
    added: Query<(Entity, &Casing), Added<Casing>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut kit: Local<
        Option<(
            Handle<Mesh>,
            Handle<StandardMaterial>,
            Handle<StandardMaterial>,
        )>,
    >,
) {
    for (e, c) in &added {
        let (mesh, brass, hull) = kit
            .get_or_insert_with(|| {
                (
                    meshes.add(Cylinder::new(1.0, 1.0)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.78, 0.6, 0.3),
                        metallic: 1.0,
                        perceptual_roughness: 0.35,
                        ..default()
                    }),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.55, 0.08, 0.06),
                        perceptual_roughness: 0.6,
                        ..default()
                    }),
                )
            })
            .clone();
        let (r, len) = c.size;
        let child = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(if c.hull { hull } else { brass }),
                Transform::from_scale(Vec3::new(r, len, r)),
            ))
            .id();
        commands.entity(e).add_child(child);
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

#[cfg(test)]
mod casing_tests {
    use super::*;

    /// Brass thrown from a gun tumbles down and clinks once on landing
    /// (then a quieter bounce or two), on the floor it hit; however many
    /// rounds are fired, no more than the cap lie about.
    #[test]
    fn cases_land_with_a_clink_and_are_capped() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        #[derive(Resource, Default)]
        struct Heard(Vec<CasingLanded>);
        app.init_resource::<Heard>().add_systems(
            FixedUpdate,
            (|mut r: MessageReader<CasingLanded>, mut h: ResMut<Heard>| {
                h.0.extend(r.read().copied())
            })
            .after(land_casings),
        );
        app.finish();
        app.cleanup();
        app.update();
        let ground = {
            let zone = app.world().resource::<crate::world::Zone>();
            Vec3::new(0.0, zone.on_ground(0.0, 30.0).y, 30.0)
        };
        app.world_mut().write_message(Ejection {
            round: Round::Spent,
            cartridge: "5.45x39".into(),
            load: Default::default(),
            at: ground + Vec3::Y * 1.3,
            velocity: Vec3::new(2.0, 1.0, 0.0),
        });
        for _ in 0..240 {
            app.update();
        }
        let heard = &app.world().resource::<Heard>().0;
        assert!(!heard.is_empty(), "no clink");
        assert!(heard[0].first && heard[0].speed > 2.0, "{heard:?}");
        assert!(heard.iter().filter(|h| h.first).count() == 1, "{heard:?}");
        assert!(heard.len() <= 3, "{heard:?}");
        assert!(matches!(
            heard[0].floor,
            CasingFloor::Soft | CasingFloor::Hard
        ));
        // A long burst: the oldest cases go.
        for i in 0..100 {
            app.world_mut().write_message(Ejection {
                round: Round::Spent,
                cartridge: "5.45x39".into(),
                load: Default::default(),
                at: ground + Vec3::new(i as f32 * 0.05, 1.3, 0.0),
                velocity: Vec3::X,
            });
            app.update();
        }
        app.update();
        let n = app.world_mut().query::<&Casing>().iter(app.world()).count();
        assert!(n <= CASING_CAP, "{n} cases");
    }
}
