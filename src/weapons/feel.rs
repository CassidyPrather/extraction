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
/// The bore's height over where the gun bears at which a shot climbs by
/// [`KICK_RISE`], metres; the climb goes with the lever, between these
/// shares of it.
pub const LEVER_REF: f32 = 0.05;
pub const LEVER_MIN: f32 = 0.5;
pub const LEVER_MAX: f32 = 1.6;
/// A straighter stock turns climb into push: push gained per share of
/// climb under [`LEVER_REF`]'s.
pub const STRAIGHT_PUSH: f32 = 0.8;
/// A self-loader's action, as shares of the shot's kick: the carrier
/// hitting the back of its travel (a little more of the same) and slamming
/// home (a dip forward), at these shares of the cycle; an open bolt's slam
/// home before the round goes. And what each feels like in the hand.
pub const CARRIER_BACK: f32 = 0.15;
pub const CARRIER_HOME: f32 = 0.12;
pub const CARRIER_BACK_AT: f32 = 0.3;
pub const CARRIER_HOME_AT: f32 = 0.75;
pub const OPEN_SLAM: f32 = 0.18;
pub const OPEN_LEAD: f32 = 0.008;
pub const BUZZ_BACK: f32 = 0.35;
pub const BUZZ_HOME: f32 = 0.25;
pub const BUZZ_SLAM: f32 = 0.5;
/// A shove spread over time comes in steps this long, seconds.
pub const SHOVE_STEP: f32 = 0.004;
/// A pump or bolt worked hard against its stops jolts the gun: momentum
/// over the gun's, N s, back at the rear stop and forward going home.
pub const WORK_JOLT: f32 = 0.8;
/// On a deployed bipod the spring's damping: it hops and settles.
pub const BIPOD_DAMPING: f32 = 0.45;
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
/// Resting on something: the spring's frequency, the mass it lends, and
/// what's left of sway, droop and lag.
pub const RECOVER_BRACED: f32 = 45.0;
pub const BRACE_REST: f32 = 8.0;
pub const BRACED_SWAY: f32 = 0.15;
pub const BRACED_LAG: f32 = 0.5;
/// How near the handguard must be to something solid to rest on it, and
/// how long a rest outlasts losing touch (so it doesn't flicker), s. A
/// hand-driven gun stops 4 to 5 cm short of what it's pressed onto, so the
/// reach is that and a margin.
pub const BRACE_REACH: f32 = 0.07;
pub const BRACE_HOLD: f32 = 0.15;
/// How near a static collider's bounds must come before the gun's own
/// shapes are tested against it, metres.
pub const BRACE_BROAD: f32 = 0.1;
/// Crouched (or kneeling): sway's share, and the spring's.
pub const CROUCH_SWAY: f32 = 0.6;
pub const CROUCH_RECOVER: f32 = 1.15;
/// Head height under which you're crouched, metres over the floor.
pub const CROUCH_HEAD: f32 = 1.2;
/// How close the stock's butt must be to the head to count as shouldered
/// (a cheek weld or the shoulder pocket), metres.
pub const SHOULDER_REACH: f32 = 0.3;

/// How a gun is being held this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hold {
    pub hands: u8,
    pub shouldered: bool,
    /// The handguard rests on something solid (a wall top, a sill).
    pub braced: bool,
    pub crouched: bool,
    /// Resting on its own deployed bipod.
    pub bipod: bool,
}

/// How many hands count as holding the gun: a second hand on it; or,
/// played one-handed (`Comfort::one_handed`), a rest under it.
pub fn hands_holding(support: bool, braced: bool, one_handed_play: bool) -> u8 {
    if support || (one_handed_play && braced) {
        2
    } else {
        1
    }
}

impl Hold {
    /// The spring's natural frequency.
    pub fn recover(self) -> f32 {
        let w = if self.braced {
            RECOVER_BRACED
        } else if self.shouldered {
            RECOVER_SHOULDER
        } else if self.hands >= 2 {
            RECOVER_TWO
        } else {
            RECOVER_ONE
        };
        w * if self.crouched { CROUCH_RECOVER } else { 1.0 }
    }

    /// The spring's damping ratio.
    pub fn damping(self) -> f32 {
        if self.bipod && self.braced {
            BIPOD_DAMPING
        } else {
            RECOVER_DAMPING
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
        } + if self.braced { BRACE_REST } else { 0.0 }
    }

    /// Follow lag for a gun of `mass` kg.
    pub fn lag(self, mass: f32) -> f32 {
        mass * if self.shouldered {
            LAG_SHOULDER
        } else if self.hands >= 2 {
            LAG_TWO
        } else {
            LAG_ONE
        } * if self.braced { BRACED_LAG } else { 1.0 }
    }

    /// What's left of sway.
    pub fn steadiness(self) -> f32 {
        (if self.braced { BRACED_SWAY } else { 1.0 })
            * if self.crouched { CROUCH_SWAY } else { 1.0 }
    }
}

/// How often the rest test reached its shape queries (a gauge: they're the
/// costly part, so most held-gun ticks shouldn't).
#[derive(Resource, Debug, Default)]
pub struct BraceChecks {
    /// Ticks the rest was looked for.
    pub looked: u32,
    /// Of those, ticks something static was near enough to test shapes.
    pub shapes: u32,
}

/// Is anything solid (not the ground) at the handguard, against the gun
/// itself, or under a seated magazine's foot (a rifle rests on its
/// magazine too)? A broad bounds check first: only near something static
/// are the gun's shapes tested. The bare ground doesn't count: resting on
/// it is lying prone.
#[allow(clippy::too_many_arguments)]
fn touching(
    spatial: &avian3d::prelude::SpatialQuery,
    statics: &Query<Option<&avian3d::prelude::CollisionLayers>, With<avian3d::prelude::Collider>>,
    terrain: Option<Entity>,
    gt: &GlobalTransform,
    layout: &super::GunLayout,
    rest_at: Vec3,
    mag_foot: Option<Vec3>,
    bipod_feet: Option<Vec3>,
    checks: &mut BraceChecks,
) -> bool {
    use avian3d::prelude::{Collider, ColliderAabb, SpatialQueryFilter};
    checks.looked += 1;
    // A deployed bipod's feet rest on anything, the bare ground too: it's
    // what they're for (`armoury::Bipod`).
    if bipod_feet.is_some_and(|feet| {
        !spatial
            .shape_intersections(
                &Collider::sphere(BRACE_REACH),
                feet,
                Quat::IDENTITY,
                &SpatialQueryFilter::from_mask(crate::world::Layer::World),
            )
            .is_empty()
    }) {
        return true;
    }
    let (lo, hi) = layout.bounds();
    // A hand-driven gun stops a little short of what it's pressed onto,
    // so "resting" allows the same reach as the handguard.
    let pad = Vec3::splat(BRACE_REACH);
    let size = hi - lo + pad * 2.0;
    let centre = gt.transform_point((lo + hi) * 0.5);
    let half = gt.rotation() * (size * 0.5);
    let reach = half.abs() + Vec3::splat(BRACE_BROAD);
    let world = crate::world::Layer::World;
    let mut near = false;
    spatial.aabb_intersections_with_aabb_callback(
        ColliderAabb::from_min_max(centre - reach, centre + reach),
        |e| {
            near = Some(e) != terrain
                && statics
                    .get(e)
                    .is_ok_and(|l| l.is_none_or(|l| l.memberships.has_all(world)));
            !near
        },
    );
    if !near {
        return false;
    }
    checks.shapes += 1;
    let filter = SpatialQueryFilter::from_mask(world).with_excluded_entities(terrain);
    let hits = |at: Vec3, shape: &Collider, rot: Quat| {
        !spatial
            .shape_intersections(shape, at, rot, &filter)
            .is_empty()
    };
    hits(rest_at, &Collider::sphere(BRACE_REACH), Quat::IDENTITY)
        || hits(
            centre,
            &Collider::cuboid(size.x, size.y, size.z),
            gt.rotation(),
        )
        || mag_foot.is_some_and(|foot| hits(foot, &Collider::sphere(0.04), Quat::IDENTITY))
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
    /// Seconds a rest outlasts losing touch.
    brace_left: f32,
    /// A visual-only stroke of the bolt or slide after a self-loader
    /// fires, 1 at the shot down to 0.
    pub cycle: f32,
    /// Iron sights brought to the eye: the turn about the front post that
    /// lines the rear sight up with it (item space), eased.
    pub sight_fix: Quat,
    /// How far the eye is down behind the iron sights, 0..1, eased: the
    /// front post goes soft as it rises (`optics`' post blur).
    pub at_eye: f32,
    /// What's still to come of the last shot: its shove and its action's
    /// beats.
    beats: Vec<Beat>,
    /// The hardest beat felt in the last [`Self::settle`], 0 if none.
    pub buzz: f32,
    /// The action's moments heard since, for `handle_guns` to send.
    pub sounds: Vec<FireEvent>,
}

/// A share of a shot's kick that lands later: in `at` seconds, these
/// rates, felt as `buzz` in the hand.
#[derive(Clone, Debug, Default)]
struct Beat {
    at: f32,
    rise: Vec2,
    push: f32,
    buzz: f32,
    sound: Option<FireEvent>,
}

/// How a gun's shot kicks, from its shape, its traits and its action
/// ([`shot_of`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub climb: f32,
    pub push: f32,
    pub walk: f32,
    pub shove: f32,
    pub action: Cycling,
}

/// What a gun's action does after (or before) the shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cycling {
    /// Worked by hand, or nothing to work.
    None,
    /// A gas or blowback self-loader's carrier, over a cycle of so many
    /// seconds.
    Carrier(f32),
    /// An open bolt: it slams home and fires, then runs back.
    OpenBolt(f32),
}

impl Shot {
    /// No character: the momentum alone.
    pub const PLAIN: Shot = Shot {
        climb: 1.0,
        push: 1.0,
        walk: 0.0,
        shove: 0.0,
        action: Cycling::None,
    };
}

/// A gun's shot as it's held: the climb from the bore's height over where
/// it bears, scaled by its traits (`FirearmDef::kick`), and its action.
pub fn shot_of(gun: &Gun, shouldered: bool) -> Shot {
    let def = &gun.firearm.def;
    let g = (lever(gun, shouldered) / LEVER_REF).clamp(LEVER_MIN, LEVER_MAX);
    let cycle = 60.0 / def.rpm.max(60.0);
    Shot {
        climb: g * def.kick.climb,
        push: (1.0 + STRAIGHT_PUSH * (1.0 - g).max(0.0)) * def.kick.push,
        walk: def.kick.walk.clamp(-1.0, 1.0),
        shove: def.kick.shove.max(0.0),
        action: match def.action {
            ActionKind::SemiAuto if def.open_bolt => Cycling::OpenBolt(cycle),
            ActionKind::SemiAuto => Cycling::Carrier(cycle),
            _ => Cycling::None,
        },
    }
}

impl Handling {
    /// A shot's momentum, with a yaw share in -1..1.
    pub fn kick(&mut self, impulse: f32, gun_mass: f32, yaw: f32) {
        self.shot(impulse, gun_mass, yaw, Shot::PLAIN);
    }

    /// A shot's momentum as `shot` delivers it, with a yaw share in -1..1.
    pub fn shot(&mut self, impulse: f32, gun_mass: f32, yaw: f32, shot: Shot) {
        let v = impulse / self.hold.mass(gun_mass).max(0.1);
        let walk = shot.walk + (1.0 - shot.walk.abs()) * yaw;
        let up = KICK_RISE * v * shot.climb;
        let rise = Vec2::new(up, up * KICK_YAW * walk);
        let push = KICK_BACK * v * shot.push;
        let lead = match shot.action {
            Cycling::OpenBolt(_) => {
                // The bolt's mass slams forward first: a dip, then the shot.
                self.rise_rate -= rise * OPEN_SLAM;
                self.push_rate -= push * OPEN_SLAM;
                self.buzz = self.buzz.max(BUZZ_SLAM);
                self.sounds.push(FireEvent::OpenBoltSlam);
                OPEN_LEAD
            }
            _ => 0.0,
        };
        // The shove, all at once or spread over its time.
        let n = (shot.shove / SHOVE_STEP).ceil().max(1.0) as usize;
        for i in 0..n {
            let at = lead + shot.shove * i as f32 / n as f32;
            let k = 1.0 / n as f32;
            if at <= 0.0 {
                self.rise_rate += rise * k;
                self.push_rate += push * k;
            } else {
                self.beats.push(Beat {
                    at,
                    rise: rise * k,
                    push: push * k,
                    ..default()
                });
            }
        }
        if let Cycling::Carrier(cycle) | Cycling::OpenBolt(cycle) = shot.action {
            self.beats.push(Beat {
                at: lead + cycle * CARRIER_BACK_AT,
                rise: rise * CARRIER_BACK,
                push: push * CARRIER_BACK,
                buzz: BUZZ_BACK,
                sound: None,
            });
        }
        if let Cycling::Carrier(cycle) = shot.action {
            self.beats.push(Beat {
                at: cycle * CARRIER_HOME_AT,
                rise: -rise * CARRIER_HOME,
                push: -push * CARRIER_HOME,
                buzz: BUZZ_HOME,
                sound: Some(FireEvent::CarrierHome),
            });
        }
    }

    /// The action worked hard against a stop: `back` toward the stock, or
    /// home (forward, the muzzle dipping).
    pub fn jolt(&mut self, gun_mass: f32, back: bool) {
        let v = WORK_JOLT / self.hold.mass(gun_mass).max(0.1);
        let s = if back { 1.0 } else { -1.0 };
        self.push_rate += KICK_BACK * v * s;
        self.rise_rate.x += KICK_RISE * v * 0.3 * s;
    }

    /// Integrates the spring for `dt` seconds.
    pub fn settle(&mut self, dt: f32) {
        let w = self.hold.recover();
        let c = 2.0 * self.hold.damping() * w;
        // Semi-implicit Euler in small steps: stable for these stiffnesses.
        let n = (dt / 0.004).ceil().max(1.0) as usize;
        let h = dt / n as f32;
        for _ in 0..n {
            let mut i = 0;
            while i < self.beats.len() {
                self.beats[i].at -= h;
                if self.beats[i].at <= 0.0 {
                    let b = self.beats.swap_remove(i);
                    self.rise_rate += b.rise;
                    self.push_rate += b.push;
                    self.buzz = self.buzz.max(b.buzz);
                    self.sounds.extend(b.sound);
                } else {
                    i += 1;
                }
            }
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
        app.init_resource::<BraceChecks>()
            .add_observer(add_handling)
            .add_systems(
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

/// The bore's height over where the gun bears against the holder (the
/// butt, shouldered; else the grip), metres: the lever a shot's push
/// climbs the muzzle by.
pub fn lever(gun: &Gun, shouldered: bool) -> f32 {
    let bears = match butt(gun) {
        Some(b) if shouldered => b,
        _ => gun.layout.grip.translation,
    };
    gun.layout.muzzle.y - bears.y
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
        Entity,
        &Gun,
        &Item,
        &GlobalTransform,
        &mut Handling,
        &mut HoldFeel,
        Option<&Held>,
        Option<&Supported>,
        Option<&super::armoury::Bipod>,
    )>,
    heads: Query<(&ChildOf, &GlobalTransform), With<RigHead>>,
    bodies: Query<&Body>,
    rigs: Query<&crate::rig::RigInput>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    spatial: avian3d::prelude::SpatialQuery,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
    mut gun_events: MessageWriter<GunEvent>,
    sockets: Query<&crate::interact::Socket>,
    mags: Query<(&GlobalTransform, &super::Mag)>,
    statics: Query<Option<&avian3d::prelude::CollisionLayers>, With<avian3d::prelude::Collider>>,
    terrain: Query<Entity, With<crate::world::TerrainCollider>>,
    mut checks: ResMut<BraceChecks>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    scoped: Query<&crate::gadgets::MountedOn, With<crate::gadgets::Optic>>,
) {
    let dt = time.delta_secs();
    let terrain = terrain.single().ok();
    // Played one-handed, a gun rested on something is held as well as two
    // hands hold it: what the surface is for.
    let one_handed = comfort.as_ref().is_some_and(|c| c.one_handed);
    for (gun_e, gun, item, gt, mut handling, mut feel, held, support, bipod) in &mut guns {
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
        let mag_foot = gun
            .magwell
            .and_then(|w| sockets.get(w).ok())
            .and_then(|s| s.occupant)
            .and_then(|m| mags.get(m).ok())
            .map(|(mgt, mag)| mgt.translation() - mgt.rotation() * Vec3::Y * (mag.height * 0.5));
        // Resting: anything solid within reach of the handguard (or the
        // middle of the barrel, on a pistol).
        let rest_at = gt.transform_point(
            gun.layout
                .support
                .unwrap_or((gun.layout.grip.translation + gun.layout.muzzle) * 0.5),
        );
        // Every tick: the broad check is cheap, and a check on some ticks
        // only fell in and out of step with a gun bouncing on its rest.
        let touching = touching(
            &spatial,
            &statics,
            terrain,
            gt,
            &gun.layout,
            rest_at,
            mag_foot,
            bipod
                .filter(|b| b.swing >= 1.0)
                .map(|b| gt.transform_point(b.feet_now())),
            &mut checks,
        );
        let was_braced = handling.hold.braced;
        handling.brace_left = if touching {
            BRACE_HOLD
        } else {
            (handling.brace_left - dt).max(0.0)
        };
        let braced = handling.brace_left > 0.0;
        // Crouched: the flat key, or a head brought low in the room.
        let crouched = rigs
            .get(held.player)
            .is_ok_and(|i| i.crouch || i.head.translation.y < CROUCH_HEAD);
        handling.hold = Hold {
            hands: hands_holding(support.is_some(), braced, one_handed),
            shouldered,
            braced,
            crouched,
            bipod: bipod.is_some_and(|b| b.swing >= 1.0),
        };
        // It takes: a settle in the hand.
        if braced && !was_braced {
            haptics.write(crate::rig::haptics::Haptic {
                side: held.side,
                amplitude: 0.3,
                seconds: 0.025,
            });
        }
        handling.buzz = 0.0;
        handling.settle(dt);
        // The action's beats, heard from the gun and felt in the gun hand.
        for event in handling.sounds.drain(..) {
            gun_events.write(GunEvent {
                gun: gun_e,
                item: catalog.get(item.kind()).id.clone(),
                event,
                at: gt.transform_point(gun.layout.muzzle),
            });
        }
        if handling.buzz > 0.0 {
            haptics.write(crate::rig::haptics::Haptic {
                side: held.side,
                amplitude: handling.buzz,
                seconds: 0.015,
            });
        }
        handling.cycle = (handling.cycle - dt / 0.07).max(0.0);
        handling.clock += dt;
        let hold = handling.hold;
        // Strain: droop and sway, eased so changing grip doesn't snap.
        let one_handed = hold.hands < 2 && !hold.shouldered;
        let m = moment(gun, mass);
        let droop_to = if one_handed && !hold.braced {
            DROOP * m
        } else {
            0.0
        };
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
        let amp = amp * hold.steadiness();
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
        let kick = Transform::from_translation(pivot - r * pivot + Vec3::Z * handling.push)
            .with_rotation(r);
        // Iron sights at the cheek: a turn about the front post puts the
        // rear sight on the eye's line to it. Worked out where the gun would
        // be without it (so it settles rather than chasing itself).
        let front = front_post(gun);
        let about_front = |q: Quat| Transform::from_translation(front - q * front).with_rotation(q);
        let (want, lined) = match head {
            Some(eye) if !scoped.iter().any(|m| m.0 == gun_e) => {
                let eye_now = gt.affine().inverse().transform_point3(eye);
                let eye_bare = about_front(handling.sight_fix).transform_point(eye_now);
                (sight_fix(gun, eye_bare), at_sights(gun, eye_bare))
            }
            _ => (Quat::IDENTITY, false),
        };
        let ease = 1.0 - (-dt / SIGHT_EASE).exp();
        handling.sight_fix = handling.sight_fix.slerp(want, ease);
        handling.at_eye += (if lined { 1.0 } else { 0.0 } - handling.at_eye) * ease;
        feel.offset = kick * about_front(handling.sight_fix);
        feel.lag = hold.lag(mass);
    }
}

/// How near behind the rear sight (metres along the bore, and off its line)
/// the eye must be for the sights to line up at the cheek, the most they
/// turn the gun for it (radians), and how quickly (seconds).
pub const SIGHT_DEPTH: (f32, f32) = (-0.02, 0.16);
pub const SIGHT_OFF: f32 = 0.05;
pub const SIGHT_MAX: f32 = 0.06;
pub const SIGHT_EASE: f32 = 0.08;

/// The front post, in gun space: on the sight line at the muzzle.
pub fn front_post(gun: &Gun) -> Vec3 {
    let s = gun.layout.sight.translation;
    Vec3::new(s.x, s.y, gun.layout.muzzle.z + 0.03)
}

/// Whether an eye at `eye` (gun space) is just behind the rear sight.
pub fn at_sights(gun: &Gun, eye: Vec3) -> bool {
    let rel = eye - gun.layout.sight.translation;
    (SIGHT_DEPTH.0..SIGHT_DEPTH.1).contains(&rel.z) && rel.truncate().length() <= SIGHT_OFF
}

/// The turn about the front post (gun space) that puts the rear sight on
/// the line from an eye at `eye` (gun space, the gun as held without it) to
/// the front post: none unless the eye is just behind the rear sight.
pub fn sight_fix(gun: &Gun, eye: Vec3) -> Quat {
    if !at_sights(gun, eye) {
        return Quat::IDENTITY;
    }
    let rear = gun.layout.sight.translation;
    let front = front_post(gun);
    let q = Quat::from_rotation_arc(
        (rear - front).normalize(),
        (eye - front).normalize_or((rear - front).normalize()),
    );
    let (axis, angle) = q.to_axis_angle();
    if angle > SIGHT_MAX {
        Quat::from_axis_angle(axis, SIGHT_MAX)
    } else {
        q
    }
}

/// Shots kick the gun that fired them, and start the bolt's stroke.
pub fn kick_guns(
    mut events: MessageReader<GunEvent>,
    mut guns: Query<(&mut Gun, &Item, &mut Handling)>,
    catalog: Res<Catalog>,
) {
    for ev in events.read() {
        let Ok((mut gun, item, mut handling)) = guns.get_mut(ev.gun) else {
            continue;
        };
        let mass = catalog.get(item.kind()).mass;
        let worked = matches!(
            gun.firearm.def.action,
            ActionKind::Pump | ActionKind::BoltAction
        );
        match ev.event {
            // A pump or bolt run back to its stop, and slammed home.
            FireEvent::Cocked if worked => handling.jolt(mass, true),
            FireEvent::BoltClosed if worked => handling.jolt(mass, false),
            _ => {}
        }
        if !matches!(ev.event, FireEvent::Fired { .. }) {
            continue;
        }
        let yaw = gun.dice.f32() * 2.0 - 1.0;
        let shot = shot_of(&gun, handling.hold.shouldered);
        handling.shot(gun.firearm.def.recoil, mass, yaw, shot);
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
        "22lr" => (0.0029, 0.0156),
        "26mm" => (0.0135, 0.078),
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
    mut hull: Local<Option<Handle<StandardMaterial>>>,
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
        // (A pump's short stroke jams a shell on the case: its double
        // feed.)
        if !matches!(
            gun.firearm.def.action,
            ActionKind::SemiAuto | ActionKind::Pump
        ) {
            continue;
        }
        let pump = gun.firearm.def.action == ActionKind::Pump;
        // A shotgun's is a red plastic hull, not brass.
        let shell = if pump {
            hull.get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.08, 0.06),
                    perceptual_roughness: 0.6,
                    ..default()
                })
            })
            .clone()
        } else {
            brass.clone()
        };
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
        // (A shell is fat: it sits down in the port, under the receiver's
        // top.)
        let bore = Vec3::new(
            port.x * 0.6,
            if pump { port.y - r * 0.3 } else { port.y + r },
            port.z,
        );
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
            MeshMaterial3d(shell.clone()),
            Transform::from_xyz(0.0, 0.0, -len * 0.3)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, Vec3::NEG_Z)),
            ChildOf(pair),
        ));
        let tip = Quat::from_rotation_x(0.6) * Vec3::NEG_Z;
        let round = commands
            .spawn((
                Mesh3d(case),
                MeshMaterial3d(shell.clone()),
                Transform::from_xyz(0.0, -r * 0.5, len * 0.5)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, tip)),
                ChildOf(pair),
            ))
            .id();
        if !pump {
            commands.spawn((
                Mesh3d(bullet),
                MeshMaterial3d(copper.clone()),
                Transform::from_xyz(0.0, len * 0.65, 0.0),
                ChildOf(round),
            ));
        }
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
mod one_hand_tests {
    use super::*;

    /// Played one-handed, a rifle rested on a surface is held as two hands
    /// hold it rested: the same spring, mass, lag and steadiness, and no
    /// one-handed droop or sway. Without the rest it's one hand.
    #[test]
    fn one_hand_on_a_rest_is_two() {
        let hold = |hands: u8| Hold {
            hands,
            shouldered: false,
            braced: true,
            ..default()
        };
        let one = hold(hands_holding(false, true, true));
        let two = hold(hands_holding(true, true, false));
        assert_eq!(one, two);
        assert_eq!(one.recover(), two.recover());
        assert_eq!(one.mass(3.3), two.mass(3.3));
        assert_eq!(one.lag(3.3), two.lag(3.3));
        assert_eq!(one.steadiness(), two.steadiness());
        assert!(one.hands >= 2, "no one-handed droop or sway");
        assert_eq!(hands_holding(false, false, true), 1, "unrested: one hand");
        assert_eq!(
            hands_holding(false, true, false),
            1,
            "two-handed play: as before"
        );
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
            ..default()
        };
        let two = Hold {
            hands: 2,
            shouldered: false,
            ..default()
        };
        let shoulder = Hold {
            hands: 2,
            shouldered: true,
            ..default()
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
pub(crate) mod vr_tests {
    use super::*;
    use crate::interact::GripRole;
    use crate::rig::Side;
    use crate::vr_harness::Vr;
    use crate::weapons::ballistics::Projectile;
    use extraction_core::ammo::Load;
    use extraction_core::firearm::{FireMode, Magazine};
    use std::collections::HashSet;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);
    const EYE: Vec3 = Vec3::new(0.0, 1.65, 0.0);

    pub(crate) struct Range {
        pub(crate) vr: Vr,
        pub(crate) gun: Entity,
        pub(crate) feet: Vec3,
    }

    /// Stand in the open with `id` in the right hand, held out in front
    /// at chest height (or with the stock to the cheek), the left hand on
    /// the handguard if `two`.
    pub(crate) fn hold(id: &str, two: bool, shoulder: bool) -> Range {
        hold_after(id, two, shoulder, 0)
    }

    /// [`hold`], with `extra` bare entities spawned first (whatever else a
    /// game spawns shifts every entity's index).
    pub(crate) fn hold_after(id: &str, two: bool, shoulder: bool, extra: usize) -> Range {
        let mut vr = Vr::new("training_yard");
        for _ in 0..extra {
            vr.app.world_mut().spawn(Transform::IDENTITY);
        }
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

    pub(crate) fn gun_mut(r: &mut Range) -> Mut<'_, Gun> {
        r.vr.app.world_mut().get_mut::<Gun>(r.gun).unwrap()
    }

    /// A full magazine in the mechanism, a round chambered, on `mode`.
    pub(crate) fn load(r: &mut Range, family: &str, rounds: u32, mode: FireMode) {
        let mut g = gun_mut(r);
        let f = &mut g.firearm;
        f.insert_magazine(Magazine {
            family: family.into(),
            capacity: rounds,
            rounds,
            ..Default::default()
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
    pub(crate) fn fire(r: &mut Range, n: usize, pulse: bool) -> Vec<Vec3> {
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

    /// Takes `id` from where it lies into the `side` hand.
    pub(crate) fn take_in(r: &mut Range, side: Side, id: &str, at: Vec3) -> Entity {
        let e = r.vr.spawn(id, Transform::from_translation(at));
        r.vr.app
            .world_mut()
            .entity_mut(e)
            .insert(avian3d::prelude::RigidBody::Static);
        r.vr.hand(side, at, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(2);
        r.vr.grip(side, 1.0);
        r.vr.step(3);
        r.vr.app
            .world_mut()
            .entity_mut(e)
            .insert(avian3d::prelude::RigidBody::Dynamic);
        assert_eq!(r.vr.hand_state(side).holding, Some(e), "took {id}");
        e
    }

    /// Two AP rounds thumbed onto a magazine of ball, by hand: seated and
    /// fired, the AP comes out first, then the ball; each shot is the round
    /// that fired.
    #[test]
    fn ap_thumbed_on_top_of_ball_fires_first() {
        let mut r = hold("ak74", false, false);
        // Set the rifle down where it is (held still in the air) to free
        // the hand.
        r.vr.app
            .world_mut()
            .entity_mut(r.gun)
            .insert(avian3d::prelude::RigidBody::Static);
        let grip = r.vr.hand_state(Side::Right).world;
        r.vr.grip(Side::Right, 0.0);
        r.vr.step(3);
        let mag = take(&mut r, "ak_mag", 28);
        let at_mag = r.vr.hand_state(Side::Left).world.translation;
        for k in 0..2 {
            let spot = r.feet + Vec3::new(0.4, 1.0, -0.2 - 0.1 * k as f32);
            take_in(&mut r, Side::Right, "round_545_ap", spot);
            glide_side(&mut r, Side::Right, spot, at_mag, 10);
            r.vr.step(3);
            r.vr.move_hand(Side::Right, spot);
            r.vr.step(3);
        }
        {
            let it = r.vr.app.world().get::<Item>(mag).unwrap();
            assert_eq!(it.rounds, 30);
            assert_eq!(it.loads.0, vec![Load::Ap, Load::Ap], "two AP on top");
        }
        // The rifle back in hand.
        r.vr.hand(Side::Right, grip.translation, grip.rotation, 0.0, 0.0);
        r.vr.step(2);
        r.vr.grip(Side::Right, 1.0);
        r.vr.step(3);
        r.vr.app
            .world_mut()
            .entity_mut(r.gun)
            .insert(avian3d::prelude::RigidBody::Dynamic);
        r.vr.step(10);
        assert_eq!(r.vr.hand_state(Side::Right).holding, Some(r.gun));
        let well = {
            let w = gun_mut(&mut r).magwell.unwrap();
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
            while g.firearm.fire_mode() != FireMode::Semi {
                g.firearm.cycle_mode();
            }
        }
        #[derive(Resource, Default)]
        struct Shots(Vec<Load>);
        r.vr.app.init_resource::<Shots>().add_systems(
            FixedPostUpdate,
            |new: Query<&Projectile, Added<Projectile>>, mut shots: ResMut<Shots>| {
                shots.0.extend(new.iter().map(|p| p.load))
            },
        );
        rack(&mut r);
        for _ in 0..3 {
            assert_eq!(fire(&mut r, 1, true).len(), 1);
        }
        let loads = r.vr.app.world().resource::<Shots>().0.clone();
        assert_eq!(loads, vec![Load::Ap, Load::Ap, Load::Standard]);
    }

    /// A slug in one barrel and buckshot in the other, by hand: the slug
    /// is one heavy ball, the buckshot its spread of pellets.
    #[test]
    fn a_slug_and_a_shell_in_the_double() {
        #[derive(Resource, Default)]
        struct Shots(Vec<(Load, f32)>);
        let mut r = hold("toz_double", false, false);
        r.vr.app.init_resource::<Shots>().add_systems(
            FixedPostUpdate,
            |new: Query<&Projectile, Added<Projectile>>, mut shots: ResMut<Shots>| {
                shots.0.extend(new.iter().map(|p| (p.load, p.mass_g)))
            },
        );
        r.vr.primary(Side::Right, true);
        r.vr.step(2);
        r.vr.primary(Side::Right, false);
        r.vr.step(2);
        for id in ["slug_12ga", "shell_12ga"] {
            take(&mut r, id, 1);
            let (port, _) = load_port(&mut r);
            let away = r.vr.hand_state(Side::Left).world.translation;
            glide(&mut r, away, port, 15);
            r.vr.step(2);
            r.vr.grip(Side::Left, 0.0);
            r.vr.step(2);
        }
        r.vr.primary(Side::Right, true);
        r.vr.step(2);
        r.vr.primary(Side::Right, false);
        r.vr.step(2);
        assert_eq!(gun_mut(&mut r).firearm.live_rounds(), 2);
        fire(&mut r, 1, true);
        fire(&mut r, 1, true);
        let shots = r.vr.app.world().resource::<Shots>().0.clone();
        let slugs: Vec<_> = shots.iter().filter(|(l, _)| *l == Load::Slug).collect();
        assert_eq!(slugs.len(), 1, "{shots:?}");
        assert_eq!(slugs[0].1, extraction_core::ammo::SLUG_MASS_G);
        let pellets = shots.iter().filter(|(l, _)| *l == Load::Standard).count();
        assert!(pellets > 4, "buckshot spreads: {shots:?}");
    }

    /// A squib's bullet in the PM's bore: a cleaning rod pushed down it
    /// from the muzzle knocks it out (and the next shot is clean). Left in,
    /// the next shot bulges the barrel and hurts the hand.
    #[test]
    fn a_squib_rodded_out_or_fired_on() {
        for rod in [true, false] {
            let mut r = hold("pm_pistol", false, false);
            load(&mut r, "pm", 8, FireMode::Semi);
            gun_mut(&mut r).firearm.squib = Some(0);
            if rod {
                let (muzzle, out) = {
                    let g = r.vr.app.world().get::<Gun>(r.gun).unwrap();
                    let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
                    (gt.transform_point(g.layout.muzzle), gt.forward().as_vec3())
                };
                // The rod along the bore, held at its middle, out in front.
                let at = muzzle + out * 0.5;
                let rod_e = r.vr.spawn(
                    "cleaning_rod",
                    Transform::from_translation(at).looking_to(out, Vec3::Y),
                );
                r.vr.app
                    .world_mut()
                    .entity_mut(rod_e)
                    .insert(avian3d::prelude::RigidBody::Static);
                let rot = Transform::IDENTITY.looking_to(out, Vec3::Y).rotation;
                r.vr.hand(Side::Left, at, rot, 0.0, 0.0);
                r.vr.step(2);
                r.vr.grip(Side::Left, 1.0);
                r.vr.step(3);
                r.vr.app
                    .world_mut()
                    .entity_mut(rod_e)
                    .insert(avian3d::prelude::RigidBody::Dynamic);
                assert_eq!(r.vr.hand_state(Side::Left).holding, Some(rod_e));
                // In at the muzzle, down the bore: the rod's near end goes
                // 15 cm past the muzzle.
                let from = r.vr.hand_state(Side::Left).world.translation;
                glide_side(&mut r, Side::Left, from, muzzle + out * (0.3 - 0.15), 30);
                r.vr.step(3);
                assert_eq!(gun_mut(&mut r).firearm.squib, None, "rodded out");
            }
            let before =
                r.vr.app
                    .world()
                    .get::<crate::health::Health>(r.vr.player)
                    .unwrap()
                    .hp;
            fire(&mut r, 1, true);
            r.vr.step(3);
            let after =
                r.vr.app
                    .world()
                    .get::<crate::health::Health>(r.vr.player)
                    .unwrap()
                    .hp;
            let cond = gun_mut(&mut r).firearm.condition;
            if rod {
                assert!(cond > 0.9, "a clean shot: {cond}");
                assert!(before - after < 1.0, "unhurt: {before} {after}");
            } else {
                assert!(cond < 0.35, "bulged: {cond}");
                assert!(before - after > 10.0, "hurt: {before} {after}");
            }
        }
    }

    /// A filthy Mosin's bolt stuck shut: lifting the handle slowly only
    /// grinds; slapped up hard, it breaks free and lifts.
    #[test]
    fn a_stuck_mosin_bolt_wants_a_slap() {
        let mut r = hold("mosin", false, false);
        {
            let mut g = gun_mut(&mut r);
            g.firearm.fouling = 1.0;
            g.firearm.bolt_stuck = true;
        }
        let handle = r.vr.point(r.gun, |p| matches!(p, GripRole::Handle(0)));
        // The hand comes to the handle and rests there before taking it
        // (arriving fast from below would be the slap).
        r.vr.hand(Side::Left, handle, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(30);
        r.vr.trigger(Side::Left, 1.0);
        r.vr.step(2);
        assert!(
            r.vr.hand_state(Side::Left).handle.is_some(),
            "on the handle"
        );
        let up =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .up()
                .as_vec3();
        // Slowly: 5 cm over a second and a half.
        glide_side(&mut r, Side::Left, handle, handle + up * 0.05, 135);
        assert!(gun_mut(&mut r).firearm.bolt_stuck, "still stuck");
        assert!(
            gun_mut(&mut r).firearm.handle_lift < 0.05,
            "it doesn't lift"
        );
        // Back down, then a slap: 6 cm in two ticks.
        glide_side(&mut r, Side::Left, handle + up * 0.05, handle, 20);
        glide_side(&mut r, Side::Left, handle, handle + up * 0.06, 2);
        r.vr.step(3);
        assert!(!gun_mut(&mut r).firearm.bolt_stuck, "knocked free");
        assert!(gun_mut(&mut r).firearm.handle_lift > 0.9, "and up");
    }

    /// A worn, filthy SKS: rack it and the round goes off as the bolt slams
    /// home, trigger untouched (its dice set to roll low). A sound one
    /// never does, whatever the roll.
    #[test]
    fn a_worn_sks_slam_fires_as_the_bolt_closes() {
        #[derive(Resource, Default)]
        struct Shots(usize);
        for worn in [true, false] {
            let mut r = hold("sks", false, false);
            r.vr.app.init_resource::<Shots>().add_systems(
                FixedPostUpdate,
                |new: Query<(), Added<Projectile>>, mut n: ResMut<Shots>| n.0 += new.iter().count(),
            );
            {
                let mut g = gun_mut(&mut r);
                g.firearm.condition = if worn { 0.1 } else { 0.9 };
                g.firearm.fouling = 1.0;
                g.firearm.internal_rounds = 10;
                // Its own dice set so the bolt let go by hand rolls low (the
                // worn gun's slam chance is 0.6; a sound one's is none).
                g.firearm.fault_dice = (0u64..)
                    .find(|s| extraction_core::noise::Rng::new(*s).f32() < 0.3)
                    .unwrap();
            }
            rack(&mut r);
            r.vr.step(4);
            let shots = r.vr.app.world().resource::<Shots>().0;
            if worn {
                assert!(shots > 0, "a worn SKS slam-fired");
            } else {
                assert_eq!(shots, 0, "a sound one never does");
            }
        }
    }

    /// A fouled, worn AK throws its rounds wider. The cleaning kit, brought
    /// to the receiver in the other hand, takes the fouling out and the
    /// spread closes (the wear stays); three strokes of the rod down the
    /// bore take some out too.
    #[test]
    fn cleaning_by_hand_closes_the_spread() {
        let mut r = hold("ak74", false, false);
        {
            let mut g = gun_mut(&mut r);
            g.firearm.fouling = 1.0;
            g.firearm.condition = 0.5;
        }
        let dirty = gun_mut(&mut r).firearm.spread_radians();
        // The rod: in at the muzzle and back out, three times.
        let (muzzle, out) = {
            let g = r.vr.app.world().get::<Gun>(r.gun).unwrap();
            let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            (gt.transform_point(g.layout.muzzle), gt.forward().as_vec3())
        };
        let at = muzzle + out * 0.5;
        let rod = r.vr.spawn(
            "cleaning_rod",
            Transform::from_translation(at).looking_to(out, Vec3::Y),
        );
        r.vr.app
            .world_mut()
            .entity_mut(rod)
            .insert(avian3d::prelude::RigidBody::Static);
        let rot = Transform::IDENTITY.looking_to(out, Vec3::Y).rotation;
        r.vr.hand(Side::Left, at, rot, 0.0, 0.0);
        r.vr.step(2);
        r.vr.grip(Side::Left, 1.0);
        r.vr.step(3);
        r.vr.app
            .world_mut()
            .entity_mut(rod)
            .insert(avian3d::prelude::RigidBody::Dynamic);
        let away = muzzle + out * 0.5;
        let deep = muzzle + out * 0.15;
        for _ in 0..3 {
            glide_side(&mut r, Side::Left, away, deep, 20);
            glide_side(&mut r, Side::Left, deep, away, 20);
        }
        let stroked = gun_mut(&mut r).firearm.fouling;
        assert!(
            (stroked - (1.0 - 3.0 * super::super::faults::ROD_STROKE)).abs() < 0.01,
            "three strokes: {stroked}"
        );
        // Rod down, kit up, to the receiver.
        r.vr.grip(Side::Left, 0.0);
        r.vr.step(3);
        let receiver =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .translation();
        let kit_at = receiver + Vec3::new(-0.35, -0.1, 0.1);
        take_in(&mut r, Side::Left, "cleaning_kit", kit_at);
        glide_side(
            &mut r,
            Side::Left,
            kit_at,
            receiver + Vec3::new(-0.08, 0.0, 0.0),
            20,
        );
        r.vr.step(3);
        let g = gun_mut(&mut r);
        assert_eq!(g.firearm.fouling, 0.0, "cleaned");
        assert_eq!(g.firearm.condition, 0.5, "the wear stays");
        let clean = g.firearm.spread_radians();
        assert!(clean < dirty * 0.8, "the spread closes: {dirty} -> {clean}");
    }

    /// A worn, dirty AK's burst: what happened to it, in order (shots,
    /// jams, duds), with `loot` other guns lying about the yard and as many
    /// bare entities spawned first.
    fn worn_burst(loot: usize) -> Vec<String> {
        #[derive(Resource, Default)]
        struct Seen(Vec<String>);
        let mut r = hold_after("ak74", false, false, loot);
        for k in 0..loot {
            let at = r.feet + Vec3::new(3.0 + k as f32, 0.3, 4.0);
            let id = if k % 2 == 0 { "sks" } else { "ak74" };
            let e = r.vr.spawn(id, Transform::from_translation(at));
            r.vr.app.world_mut().get_mut::<Item>(e).unwrap().condition = 0.2;
        }
        let gun = r.gun;
        r.vr.app.init_resource::<Seen>().add_systems(
            FixedPostUpdate,
            move |mut ev: MessageReader<GunEvent>, mut seen: ResMut<Seen>| {
                seen.0.extend(
                    ev.read()
                        .filter(|e| e.gun == gun)
                        .map(|e| format!("{:?}", e.event)),
                );
            },
        );
        load(&mut r, "ak545", 30, FireMode::Semi);
        {
            let mut g = gun_mut(&mut r);
            g.firearm.condition = 0.3;
            g.firearm.fouling = 0.6;
        }
        fire(&mut r, 20, true);
        r.vr.app.world().resource::<Seen>().0.clone()
    }

    /// How a gun in your hands rolls is its own: guns lying about the
    /// valley, and whatever else was spawned first, don't change a drill.
    #[test]
    fn a_drill_doesnt_hang_on_other_loot() {
        let alone = worn_burst(0);
        assert!(
            alone.iter().any(|e| e.starts_with("Malfunctioned")),
            "a worn gun jams: {alone:?}"
        );
        assert_eq!(worn_burst(4), alone, "with loot about");
    }

    /// The rear sight and front post of the held gun, in the world.
    fn sights(r: &mut Range) -> (Vec3, Vec3, Quat) {
        let g = r.vr.app.world().get::<Gun>(r.gun).unwrap();
        let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
        (
            gt.transform_point(g.layout.sight.translation),
            gt.transform_point(front_post(g)),
            gt.rotation(),
        )
    }

    /// Brought to the cheek with the eye a little high of the sight line,
    /// an AK settles so the eye, the rear sight and the front post line up;
    /// with the eye nowhere near, the sights are left alone.
    #[test]
    fn iron_sights_line_up_at_the_cheek() {
        let mut r = hold("ak74", true, true);
        r.vr.step(30);
        assert_eq!(
            r.vr.app.world().get::<Handling>(r.gun).unwrap().sight_fix,
            Quat::IDENTITY,
            "the eye nowhere near the sights"
        );
        let (rear, front, rot) = sights(&mut r);
        let eye = rear + rot * Vec3::new(0.0, 0.012, 0.06);
        let line_miss = |rear: Vec3, front: Vec3| {
            let d = (rear - front).normalize();
            let v = eye - front;
            (v - d * v.dot(d)).length()
        };
        assert!(line_miss(rear, front) > 0.009, "starts off the line");
        let root =
            r.vr.app
                .world()
                .get::<Transform>(r.vr.player)
                .unwrap()
                .translation;
        r.vr.head(eye - root);
        r.vr.step(60);
        let (rear, front, _) = sights(&mut r);
        let miss = line_miss(rear, front);
        assert!(miss < 0.004, "eye, rear sight, front post: {miss} m off");
    }

    /// Iron sights brought to the eye, the front post goes soft (the eye
    /// is on the target), unless the comfort setting is off; taken down,
    /// it's sharp again.
    #[test]
    fn the_front_post_softens_at_the_cheek() {
        use crate::weapons::optics::{POST_BLUR, post_blur};
        let mut r = hold("ak74", true, true);
        r.vr.step(30);
        let at_eye = |r: &Range| r.vr.app.world().get::<Handling>(r.gun).unwrap().at_eye;
        assert!(at_eye(&r) < 0.01, "the eye nowhere near: {}", at_eye(&r));
        let (rear, _, rot) = sights(&mut r);
        let root =
            r.vr.app
                .world()
                .get::<Transform>(r.vr.player)
                .unwrap()
                .translation;
        let away =
            r.vr.app
                .world()
                .get::<crate::rig::RigInput>(r.vr.player)
                .unwrap()
                .head
                .translation;
        r.vr.head(rear + rot * Vec3::new(0.0, 0.01, 0.06) - root);
        r.vr.step(60);
        let a = at_eye(&r);
        assert!(a > 0.95, "down behind the sights: {a}");
        assert!((post_blur(a, true) - POST_BLUR).abs() < 0.05);
        assert_eq!(post_blur(a, false), 0.0, "the comfort setting off");
        r.vr.head(away);
        r.vr.step(60);
        assert!(at_eye(&r) < 0.05, "taken down: {}", at_eye(&r));
    }

    pub(crate) fn glide_side(r: &mut Range, side: Side, a: Vec3, b: Vec3, steps: usize) {
        for i in 1..=steps {
            r.vr.move_hand(side, a.lerp(b, i as f32 / steps as f32));
            r.vr.step(1);
        }
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
    pub(crate) fn rack(r: &mut Range) {
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
    pub(crate) fn seat_mag(r: &mut Range, id: &str, rounds: u32) -> Entity {
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
    pub(crate) fn take(r: &mut Range, id: &str, rounds: u32) -> Entity {
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
    pub(crate) fn load_port(r: &mut Range) -> (Vec3, Vec3) {
        let port = gun_mut(r).layout.load_port;
        let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
        (gt.transform_point(port), gt.up().as_vec3())
    }

    /// Glides the left hand from `a` to `b`.
    pub(crate) fn glide(r: &mut Range, a: Vec3, b: Vec3, steps: usize) {
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
    pub(crate) fn press(r: &mut Range, which: &str) {
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

    /// How far the gun's bore wanders over three seconds held still, in
    /// degrees (max from its mean).
    fn wander(r: &mut Range) -> f32 {
        let mut dirs = Vec::new();
        for _ in 0..270 {
            r.vr.step(1);
            let g = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            dirs.push(g.forward().as_vec3());
        }
        let mean = dirs.iter().copied().sum::<Vec3>().normalize();
        dirs.iter()
            .map(|d| d.angle_between(mean))
            .fold(0.0f32, f32::max)
            .to_degrees()
    }

    /// An AK held out in one hand sways; rest its handguard on a block
    /// of concrete and it all but stops, with a settle in the hand as it
    /// takes.
    #[test]
    fn a_rested_gun_is_steady() {
        rested_gun_is_steady(0);
    }

    /// The same, whatever else is spawned before the gun (an entity's index
    /// once set which ticks a rest was looked for).
    #[test]
    fn a_rest_doesnt_hang_on_entity_order() {
        for extra in 1..=3 {
            rested_gun_is_steady(extra);
        }
    }

    fn rested_gun_is_steady(extra: usize) {
        #[derive(Resource, Default)]
        struct Felt(Vec<crate::rig::haptics::Haptic>);
        let mut r = hold_after("ak74", false, false, extra);
        r.vr.step(60);
        *r.vr.app.world_mut().resource_mut::<BraceChecks>() = BraceChecks::default();
        let free = wander(&mut r);
        // Held up in the open, over the ground: the rest is looked for each
        // tick, and nothing is near enough to test shapes against (they're
        // the costly part).
        let checks = std::mem::take(&mut *r.vr.app.world_mut().resource_mut::<BraceChecks>());
        let ticks = 270.0;
        assert!(
            checks.looked as f32 >= ticks - 1.0,
            "looked every tick: {checks:?}"
        );
        assert_eq!(checks.shapes, 0, "no shapes in the open: {checks:?}");
        r.vr.app.init_resource::<Felt>().add_systems(
            Update,
            |mut h: MessageReader<crate::rig::haptics::Haptic>, mut f: ResMut<Felt>| {
                f.0.extend(h.read().copied())
            },
        );
        // Where the handguard is.
        let (support, down) = {
            let s = gun_mut(&mut r).layout.support.unwrap();
            let g = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            (g.transform_point(s), g.down().as_vec3())
        };
        // Clear below the gun's collider; then the hand lowers it on.
        let (lo, _) = gun_mut(&mut r).layout.bounds();
        let s_y = gun_mut(&mut r).layout.support.unwrap().y;
        let top = support + down * (s_y - lo.y + 0.08);
        r.vr.app.world_mut().spawn((
            Transform::from_translation(top - Vec3::Y * 0.3),
            avian3d::prelude::RigidBody::Static,
            avian3d::prelude::Collider::cuboid(0.4, 0.6, 0.12),
            avian3d::prelude::CollisionLayers::new(
                crate::world::Layer::World,
                avian3d::prelude::LayerMask::ALL,
            ),
        ));
        r.vr.step(5);
        let hand = r.vr.hand_state(Side::Right).world.translation;
        for i in 1..=20 {
            r.vr.move_hand(Side::Right, hand + Vec3::NEG_Y * 0.006 * i as f32);
            r.vr.step(1);
        }
        r.vr.step(30);
        assert!(
            r.vr.app.world().get::<Handling>(r.gun).unwrap().hold.braced,
            "resting on the block"
        );
        assert!(
            r.vr.app.world().resource::<BraceChecks>().shapes > 0,
            "the block is tested"
        );
        // Let the droop ease off before measuring.
        r.vr.step(120);
        let rested = wander(&mut r);
        // Most of it gone (what's left is the gun pressed on the block).
        assert!(
            rested < free * 0.4,
            "free {free:.2} deg, rested {rested:.2}"
        );
        let settles =
            r.vr.app
                .world()
                .resource::<Felt>()
                .0
                .iter()
                .filter(|h| h.amplitude <= 0.35)
                .count();
        assert_eq!(settles, 1, "one settle as it takes");
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
