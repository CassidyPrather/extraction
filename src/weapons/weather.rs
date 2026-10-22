//! Guns in the weather: cold, wet, ash and snow, and the field care that
//! answers them.
//!
//! Every gun and scope carries a [`Weathering`]. It takes the air where it
//! is: carried (held, slung, in a pocket), its carrier's (`world::cold`'s
//! [`Chill`]: under a roof or out in the bite); lying loose, its own (a
//! roof looked for over it).
//!
//! - **Cold** soaks into the metal, slowly; a roof (the truck, a hut) warms
//!   it again. Past [`STIFF_FROM`] the oil stiffens (`Firearm::stiff`): the
//!   action cycles slower and jams more. A bolt-action left out cold and wet
//!   for [`FREEZE_SECS`] freezes shut: slap the handle up hard to break it
//!   (as a dirty one sticks, `faults`), or warm it.
//! - **Glass fogs**: a scope cold from outside brought under a roof fogs
//!   over, and clears as it warms. A glove wiped across the lens clears it
//!   for a while ([`wipe_guns`]).
//! - **Bare metal bites**: taking hold of a gun cold-soaked past
//!   [`STING_COLD`] (or anything metal out in a hard frost) stings the hand.
//! - **Wet and ash** get into the action as grime (`Firearm::grime`): rain
//!   and melting snow on an uncovered gun, a dunking, ash drifting down in
//!   the dry. Grime jams the action and makes each shot foul it faster. A
//!   wet gun shines; a grimy one goes dull and dark (client side).
//! - **The cure** is the drill you'd use anyway: clear the jam by hand (tap,
//!   rack, a look in the port), and run a glove along the receiver: each
//!   stroke takes off some grime and some of the wet ([`wipe_guns`]). The
//!   cleaning kit takes the rest, fouling and all.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::ammo::Load;
use extraction_core::cold::{self, Weather};
use extraction_core::firearm::ActionKind;
use extraction_core::map::MarkerKind;
use std::collections::HashMap;

use super::Gun;
use crate::gadgets::{MountedOn, Optic};
use crate::interact::{Hand, Held, InteractEvent};
use crate::rig::RigHand;
use crate::rig::haptics::Haptic;
use crate::world::Zone;
use crate::world::cold::Chill;
use crate::world::weather::{Conditions, WeatherKind};

/// Seconds for a warm gun to soak through to the air's cold, and for a
/// cold one to warm under a roof (to about two thirds of the way).
pub const SOAK_SECS: f32 = 240.0;
pub const WARM_SECS: f32 = 90.0;
/// A gun carried on the body takes this share of the air's bite.
pub const CARRIED: f32 = 0.85;
/// The oil starts to stiffen at this cold, and is as stiff as it gets by
/// [`STIFF_FULL`].
pub const STIFF_FROM: f32 = 0.3;
pub const STIFF_FULL: f32 = 0.9;
/// A bolt-action left out (not carried) this cold and this wet for
/// [`FREEZE_SECS`] freezes shut; it thaws by [`THAW`].
pub const FREEZE_COLD: f32 = 0.75;
pub const FREEZE_WET: f32 = 0.25;
pub const FREEZE_SECS: f32 = 90.0;
pub const THAW: f32 = 0.45;
/// Glass this cold brought under a roof fogs; it fogs over in about
/// [`FOG_IN`] seconds, more slowly just after a wipe ([`FOG_AFTER_WIPE`]),
/// and clears in about [`FOG_OUT`] once warm.
pub const FOG_COLD: f32 = 0.35;
pub const FOG_IN: f32 = 3.0;
pub const FOG_AFTER_WIPE: f32 = 30.0;
pub const FOG_OUT: f32 = 40.0;
/// How long a wipe slows the fog's return, seconds.
pub const WIPED_FOR: f32 = 20.0;
/// Bare metal this cold stings the hand that takes it.
pub const STING_COLD: f32 = 0.6;
/// Seconds of full rain to soak an uncovered gun through, and to dry it
/// under a roof or in the open.
pub const WET_SECS: f32 = 90.0;
pub const DRY_IN: f32 = 240.0;
pub const DRY_OUT: f32 = 900.0;
/// Grime a soaked gun gathers each second; ash at full fall each second;
/// mud and water, under.
pub const WET_GRIME: f32 = 1.0 / 1800.0;
pub const ASH_GRIME: f32 = 1.0 / 1200.0;
pub const SUNK_GRIME: f32 = 1.0 / 120.0;
/// A stroke of a glove along the action, metres; what each takes off.
pub const WIPE_STROKE: f32 = 0.12;
pub const WIPE_GRIME: f32 = 0.2;
pub const WIPE_WET: f32 = 0.25;
/// How near the gun (outside its blockout's bounds) a wiping glove must be,
/// metres; and how near a lens.
pub const WIPE_REACH: f32 = 0.05;
pub const WIPE_LENS: f32 = 0.06;
/// How often a loose thing looks for a roof over it, seconds.
const PROBE_EVERY: f32 = 1.0;
/// A roof counts this far up, metres.
const ROOF_REACH: f32 = 12.0;

/// How the weather has got into a gun or a scope.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Weathering {
    /// Cold-soaked, 0 warm ..= 1 as cold as the air gets.
    pub cold: f32,
    /// 0 dry ..= 1 soaked.
    pub wet: f32,
    /// Under a roof (its own, or its carrier's).
    pub sheltered: bool,
    /// Carried on someone.
    pub carried: bool,
    /// Seconds left out cold and wet enough to freeze.
    pub left_out: f32,
    /// Frozen shut by the cold (thaws as it warms).
    pub frozen: bool,
    /// A scope's fog, 0 clear ..= 1 white.
    pub fog: f32,
    /// Seconds since a wipe slows the fog's return.
    pub wiped: f32,
    /// Seconds soaked toward the next round going damp.
    pub soaking: f32,
    /// Rounds gone damp in it so far (which goes next).
    pub spoiled: u32,
    probe: f32,
}

/// Soaked past this, its rounds go damp, one each [`DAMP_EVERY`] seconds.
pub const DAMP_WET: f32 = 0.6;
pub const DAMP_EVERY: f32 = 45.0;

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(weather_guns_too)
            .add_observer(weather_optics_too)
            .add_observer(weather_ammo_too)
            .add_systems(
                FixedUpdate,
                (weather, wipe_guns, sting)
                    .chain()
                    .after(crate::interact::InteractSystems::Grab),
            );
    }
}

fn weather_guns_too(add: On<Add, Gun>, mut commands: Commands) {
    commands
        .entity(add.entity)
        .try_insert(Weathering::default());
}

fn weather_optics_too(add: On<Add, Optic>, mut commands: Commands) {
    commands
        .entity(add.entity)
        .try_insert(Weathering::default());
}

/// Ammunition (boxes, loose rounds, magazines) takes the weather too.
fn weather_ammo_too(
    add: On<Add, crate::items::Item>,
    items: Query<&crate::items::Item>,
    catalog: Res<crate::items::Catalog>,
    mut commands: Commands,
) {
    let Ok(it) = items.get(add.entity) else {
        return;
    };
    use extraction_core::items::Category;
    if matches!(
        catalog.get(it.kind()).category,
        Category::Ammo | Category::Magazine
    ) {
        commands
            .entity(add.entity)
            .try_insert(Weathering::default());
    }
}

/// The next round to go damp among `count` (from the top, turning about).
fn next_round(spoiled: u32, count: u32) -> u32 {
    if count == 0 { 0 } else { (spoiled * 7) % count }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How stiff a gun's oil is at `cold`.
pub fn stiffness(cold: f32) -> f32 {
    smoothstep(STIFF_FROM, STIFF_FULL, cold)
}

/// Who carries `e`, if anyone: the player holding it or one it hangs from
/// (a sling, a pouch, a scope on a gun in someone's hands).
fn carrier(
    e: Entity,
    held: &Query<&Held>,
    parents: &Query<&ChildOf>,
    mounted: &Query<&MountedOn>,
    chills: &Query<&Chill>,
) -> Option<Entity> {
    let mut cur = e;
    for _ in 0..10 {
        if let Ok(h) = held.get(cur) {
            return Some(h.player);
        }
        if chills.contains(cur) {
            return Some(cur);
        }
        cur = match (mounted.get(cur), parents.get(cur)) {
            (Ok(m), _) if m.0 != cur => m.0,
            (_, Ok(p)) => p.parent(),
            _ => return None,
        };
    }
    None
}

/// The valley's floor for the cold's exposure (as `world::cold` has it).
fn floor(zone: &Zone) -> f32 {
    zone.map
        .markers_of(&MarkerKind::Insertion)
        .next()
        .map_or(zone.map.base_height, |m| zone.ground(m.at[0], m.at[1]))
}

fn weather_of(c: &Conditions) -> Weather {
    match c.kind {
        WeatherKind::Clearing => Weather::Clear,
        WeatherKind::Drizzle => Weather::Ordinary,
        WeatherKind::PeaSouper => Weather::Fog,
        WeatherKind::Downpour => Weather::Rain,
    }
}

/// Each gun and scope takes the weather where it is.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn weather(
    zone: Res<Zone>,
    conditions: Option<Res<Conditions>>,
    snow: Option<Res<crate::world::snow::Snow>>,
    time: Res<Time>,
    spatial: SpatialQuery,
    mut things: Query<(
        Entity,
        &GlobalTransform,
        &mut Weathering,
        Option<&mut Gun>,
        Has<Optic>,
        Option<&mut crate::items::Item>,
    )>,
    held: Query<&Held>,
    seated: Query<&crate::interact::InSocket>,
    sockets: Query<&crate::interact::Socket>,
    catalog: Res<crate::items::Catalog>,
    parents: Query<&ChildOf>,
    mounted: Query<&MountedOn>,
    chills: Query<&Chill>,
    mut low: Local<Option<f32>>,
) {
    let dt = time.delta_secs();
    let valley = zone.map.cold;
    let ash = zone.map.ash;
    let low = *low.get_or_insert_with(|| floor(&zone));
    let (weather, rain) = conditions
        .as_ref()
        .map_or((Weather::Ordinary, 0.0), |c| (weather_of(c), c.rain));
    let filter = SpatialQueryFilter::from_mask([crate::world::Layer::World]);
    for (e, gt, mut w, gun, optic, item) in &mut things {
        let at = gt.translation();
        // A roof over it, looked for now and then (the carrier's own look
        // is only kept up in a cold valley).
        w.probe -= dt;
        if w.probe <= 0.0 {
            w.probe = PROBE_EVERY;
            w.sheltered = spatial
                .cast_ray_predicate(at, Dir3::Y, ROOF_REACH, true, &filter, &|h| h != e)
                .is_some();
        }
        let who = carrier(e, &held, &parents, &mounted, &chills);
        w.carried = who.is_some();
        let bite = match who.and_then(|p| chills.get(p).ok()) {
            Some(c) => c.bite * CARRIED,
            None if w.sheltered => 0.0,
            None => valley * cold::exposure(at.y - low) * weather.factor(),
        };
        // Cold soaks in slowly and warms out under a roof faster.
        let tau = if bite > w.cold { SOAK_SECS } else { WARM_SECS };
        w.cold += (bite - w.cold) * (dt / tau).min(1.0);
        // Wet: rain and melting snow on it uncovered, or under water.
        let sunk = zone.hm.water_level([at.x, at.z]).is_some_and(|l| at.y < l);
        let snowed = snow
            .as_ref()
            .is_some_and(|s| s.falling > 0.2 && s.above(at.y) > 0.5);
        let open = !w.sheltered;
        if sunk {
            w.wet = 1.0;
        } else if open && (rain > 0.05 || snowed) {
            let fall = rain.max(if snowed { 0.5 } else { 0.0 });
            w.wet = (w.wet + fall * dt / WET_SECS).min(1.0);
        } else {
            let dry = if w.sheltered { DRY_IN } else { DRY_OUT };
            w.wet = (w.wet - dt / dry).max(0.0);
        }
        // A scope's glass fogs coming in cold; clears as it warms.
        if optic {
            w.wiped = (w.wiped - dt).max(0.0);
            let want = if w.sheltered && w.cold > FOG_COLD {
                1.0
            } else {
                0.0
            };
            let tau = if want > w.fog {
                if w.wiped > 0.0 {
                    FOG_AFTER_WIPE
                } else {
                    FOG_IN
                }
            } else {
                FOG_OUT
            };
            w.fog += (want - w.fog) * (dt / tau).min(1.0);
        }
        // Soaked, a round goes damp now and then.
        let spoil = if w.wet > DAMP_WET {
            w.soaking += dt;
            if w.soaking >= DAMP_EVERY {
                w.soaking = 0.0;
                true
            } else {
                false
            }
        } else {
            w.soaking = 0.0;
            false
        };
        let Some(mut gun) = gun else {
            // A box, loose rounds, a magazine (one in a gun is the gun's).
            let in_gun = seated
                .get(e)
                .ok()
                .and_then(|s| sockets.get(s.socket).ok())
                .is_some_and(|s| matches!(s.kind, crate::interact::SocketKind::MagWell(_)));
            if let (true, Some(mut it), false) = (spoil, item, in_gun) {
                let base = catalog.get(it.kind()).load;
                let rounds = it.rounds;
                let k = next_round(w.spoiled, rounds);
                if it.loads.spoil(rounds, base, k) {
                    w.spoiled += 1;
                }
            }
            continue;
        };
        if spoil {
            let k = w.spoiled;
            let f = &mut gun.firearm;
            let done = if let Some(m) = &mut f.magazine {
                let n = m.rounds;
                m.loads.spoil(n, Load::Standard, next_round(k, n))
            } else {
                let n = f.internal_rounds;
                f.internal_loads.spoil(n, Load::Standard, next_round(k, n))
            };
            if done {
                w.spoiled += 1;
            }
        }
        if (gun.firearm.ammo_cold - w.cold).abs() > 0.01 {
            gun.firearm.ammo_cold = w.cold;
        }
        // Grime: soaked, dunked, or ash settling in the dry.
        let mut grime = 0.0;
        if open && w.wet > 0.3 {
            grime += WET_GRIME * w.wet;
        }
        if sunk {
            grime += SUNK_GRIME;
        }
        if open && rain < 0.05 {
            grime += ASH_GRIME * ash;
        }
        if grime > 0.0 {
            let f = &mut gun.firearm;
            f.grime = (f.grime + grime * dt).min(1.0);
        }
        let stiff = stiffness(w.cold);
        if (gun.firearm.stiff - stiff).abs() > 0.005 {
            gun.firearm.stiff = stiff;
        }
        // A bolt left out in the cold and wet freezes shut.
        let bolt = gun.firearm.def.action == ActionKind::BoltAction;
        if !w.carried && w.cold > FREEZE_COLD && w.wet > FREEZE_WET {
            w.left_out += dt;
        } else if w.carried {
            w.left_out = 0.0;
        }
        if bolt && !w.frozen && w.left_out > FREEZE_SECS && gun.firearm.is_closed() {
            w.frozen = true;
            gun.firearm.bolt_stuck = true;
        }
        if w.frozen && (w.cold < THAW || !gun.firearm.bolt_stuck) {
            w.frozen = false;
            w.left_out = 0.0;
            gun.firearm.bolt_stuck = false;
        }
    }
}

/// A glove run along a gun's action takes off grime and wet, a stroke at a
/// time; across a scope's lens, its fog.
#[allow(clippy::type_complexity)]
pub fn wipe_guns(
    hands: Query<(Entity, &Hand, &RigHand)>,
    mut things: Query<(
        Entity,
        &GlobalTransform,
        &mut Weathering,
        Option<&mut Gun>,
        Option<&Optic>,
    )>,
    mut haptics: MessageWriter<Haptic>,
    mut strokes: Local<HashMap<(Entity, Entity), (Vec3, f32)>>,
) {
    let mut seen = Vec::new();
    for (hand_e, hand, rig) in &hands {
        // An empty glove: one holding something isn't wiping.
        if hand.holding.is_some() || hand.handle.is_some() {
            continue;
        }
        let at = hand.world.translation;
        for (e, gt, mut w, gun, optic) in &mut things {
            if gt.translation().distance(at) > 1.2 {
                continue;
            }
            let local = gt.affine().inverse().transform_point3(at);
            // Near the lens: the fog off it.
            if let Some(o) = optic {
                let lenses = [o.eye, o.eye * Vec3::new(1.0, 1.0, -1.0)];
                if w.fog > 0.05 && lenses.iter().any(|l| l.distance(local) < WIPE_LENS) {
                    w.fog = 0.0;
                    w.wiped = WIPED_FOR;
                    haptics.write(Haptic {
                        side: rig.side,
                        amplitude: 0.12,
                        seconds: 0.04,
                    });
                }
            }
            let Some(mut gun) = gun else {
                continue;
            };
            let (lo, hi) = gun.layout.bounds();
            let near = (local.clamp(lo, hi) - local).length() < WIPE_REACH;
            if !near {
                continue;
            }
            seen.push((hand_e, e));
            let (last, run) = strokes.get(&(hand_e, e)).copied().unwrap_or((local, 0.0));
            let run = run + (local - last).length();
            if run >= WIPE_STROKE {
                gun.firearm.wipe(WIPE_GRIME);
                w.wet = (w.wet - WIPE_WET).max(0.0);
                haptics.write(Haptic {
                    side: rig.side,
                    amplitude: 0.15,
                    seconds: 0.03,
                });
                strokes.insert((hand_e, e), (local, 0.0));
            } else {
                strokes.insert((hand_e, e), (local, run));
            }
        }
    }
    strokes.retain(|k, _| seen.contains(k));
}

/// Bare metal deep in the cold stings the hand that takes it.
fn sting(
    mut events: MessageReader<InteractEvent>,
    weathered: Query<&Weathering>,
    items: Query<&crate::items::Item>,
    held: Query<&Held>,
    chills: Query<&Chill>,
    catalog: Res<crate::items::Catalog>,
    mut haptics: MessageWriter<Haptic>,
) {
    for ev in events.read() {
        let InteractEvent::Grabbed { item, side } = *ev else {
            continue;
        };
        let cold = match weathered.get(item) {
            Ok(w) => w.cold,
            // Anything else metal: as cold as the air it was in.
            Err(_) => {
                let metal = items
                    .get(item)
                    .is_ok_and(|i| catalog.get(i.kind()).has_tag("metal"));
                let air = held
                    .get(item)
                    .ok()
                    .and_then(|h| chills.get(h.player).ok())
                    .map_or(0.0, |c| c.bite);
                if metal { air } else { 0.0 }
            }
        };
        if cold > STING_COLD {
            let k = ((cold - STING_COLD) / (1.0 - STING_COLD)).clamp(0.0, 1.0);
            haptics.write(Haptic {
                side,
                amplitude: 0.55 + 0.4 * k,
                seconds: 0.02,
            });
            haptics.write(Haptic {
                side,
                amplitude: 0.3 + 0.3 * k,
                seconds: 0.06,
            });
        }
    }
}

/// A weathered gun's look (client side): its materials made its own, then
/// darkened and dulled by grime, and glossed by the wet.
#[derive(Component)]
pub struct WeatheredLook {
    shown: (u8, u8),
    mats: Vec<(Handle<StandardMaterial>, Color, f32)>,
}

/// The grime's colour, and how far it darkens toward it at the worst.
const GRIME_COLOUR: Vec3 = Vec3::new(0.16, 0.13, 0.1);
const GRIME_TINT: f32 = 0.55;

#[allow(clippy::type_complexity)]
pub fn weather_looks(
    mut commands: Commands,
    mut guns: Query<(Entity, &Gun, &Weathering, Option<&mut WeatheredLook>)>,
    children: Query<&Children>,
    meshes: Query<&MeshMaterial3d<StandardMaterial>>,
    skip: Query<
        (),
        Or<(
            With<crate::items::Item>,
            With<super::wear::Crown>,
            With<super::optics::Vignette>,
            With<super::optics::PostBlur>,
            With<crate::render::gun_rest::RestMesh>,
        )>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, gun, w, look) in &mut guns {
        let grime = gun.firearm.grime.clamp(0.0, 1.0);
        let wet = w.wet.clamp(0.0, 1.0);
        let shown = ((grime * 20.0) as u8, (wet * 20.0) as u8);
        match look {
            None => {
                if shown == (0, 0) {
                    continue;
                }
                // Its own copies of its materials, to weather.
                let mut mats = Vec::new();
                let mut stack: Vec<Entity> = children
                    .get(e)
                    .map(|c| c.iter().collect())
                    .unwrap_or_default();
                while let Some(c) = stack.pop() {
                    if skip.contains(c) {
                        continue;
                    }
                    if let Ok(m) = meshes.get(c)
                        && let Some(orig) = materials.get(&m.0).cloned()
                    {
                        let (base, rough) = (orig.base_color, orig.perceptual_roughness);
                        let own = materials.add(orig);
                        commands.entity(c).insert(MeshMaterial3d(own.clone()));
                        mats.push((own, base, rough));
                    }
                    if let Ok(cs) = children.get(c) {
                        stack.extend(cs.iter());
                    }
                }
                if mats.is_empty() {
                    continue;
                }
                commands.entity(e).insert(WeatheredLook {
                    shown: (u8::MAX, u8::MAX),
                    mats,
                });
            }
            Some(mut look) => {
                if look.shown == shown {
                    continue;
                }
                look.shown = shown;
                for (h, base, rough) in &look.mats {
                    let Some(mut m) = materials.get_mut(h) else {
                        continue;
                    };
                    let b = base.to_linear();
                    let c = Vec3::new(b.red, b.green, b.blue)
                        .lerp(GRIME_COLOUR, GRIME_TINT * grime)
                        * (1.0 - 0.15 * wet);
                    m.base_color = Color::LinearRgba(LinearRgba::new(c.x, c.y, c.z, b.alpha));
                    m.perceptual_roughness = (rough + 0.35 * grime - 0.55 * wet).clamp(0.08, 1.0);
                }
            }
        }
    }
}

/// `--gallery <gun>+grime+wet` and `scopeview:<scope>+fog` (dev): the
/// things shown as filthy, soaked or fogged, held so.
pub fn gallery_weather(
    opts: Res<crate::launch::LaunchOptions>,
    mut things: Query<(&mut Weathering, Option<&mut Gun>)>,
) {
    let Some(spec) = opts.gallery.as_deref() else {
        return;
    };
    let first = spec.split(',').next().unwrap_or("");
    let (grime, wet, fog) = (
        first.contains("+grime"),
        first.contains("+wet"),
        first.contains("+fog"),
    );
    if !(grime || wet || fog) {
        return;
    }
    for (mut w, gun) in &mut things {
        if wet {
            w.wet = 1.0;
        }
        if fog {
            w.fog = 1.0;
        }
        if let (true, Some(mut g)) = (grime, gun) {
            g.firearm.grime = 1.0;
        }
    }
}

/// A fogged scope's glass (client side): a white bloom over the lens, as
/// thick as the fog.
#[derive(Component)]
pub struct LensFog {
    material: Handle<StandardMaterial>,
}

pub fn add_lens_fog(
    mut commands: Commands,
    optics: Query<(Entity, &Optic), Added<Optic>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, o) in &optics {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.85, 0.87, 0.9, 0.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            Name::new("Lens fog"),
            LensFog {
                material: material.clone(),
            },
            Mesh3d(meshes.add(Circle::new(0.018))),
            MeshMaterial3d(material),
            // Just in front of the vignette.
            Transform::from_translation(o.eye + Vec3::Z * 0.0026),
            Visibility::Hidden,
            bevy::light::NotShadowCaster,
            ChildOf(e),
        ));
    }
}

pub fn fog_lenses(
    mut fogs: Query<(&LensFog, &ChildOf, &mut Visibility)>,
    optics: Query<&Weathering, With<Optic>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (f, parent, mut vis) in &mut fogs {
        let Ok(w) = optics.get(parent.parent()) else {
            continue;
        };
        let a = (w.fog * 0.9).clamp(0.0, 0.9);
        let want = if a > 0.01 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if let Some(m) = materials.get(&f.material)
            && (m.base_color.alpha() - a).abs() < 0.01
        {
            continue;
        }
        if let Some(mut m) = materials.get_mut(&f.material) {
            m.base_color.set_alpha(a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::GripRole;
    use crate::rig::Side;
    use crate::vr_harness::Vr;
    use bevy::ecs::message::Messages;

    fn ground(vr: &Vr, x: f32, z: f32) -> f32 {
        vr.app.world().resource::<Zone>().ground(x, z)
    }

    fn weathering(vr: &Vr, e: Entity) -> Weathering {
        *vr.app.world().get::<Weathering>(e).unwrap()
    }

    fn set(vr: &mut Vr, e: Entity, f: impl FnOnce(&mut Weathering)) {
        f(&mut vr.app.world_mut().get_mut::<Weathering>(e).unwrap());
    }

    /// Something lying still at `at`.
    fn lay(vr: &mut Vr, id: &str, at: Vec3) -> Entity {
        let e = vr.spawn(id, Transform::from_translation(at));
        vr.app.world_mut().entity_mut(e).insert(RigidBody::Static);
        vr.step(3);
        e
    }

    /// A Mosin left out on Rime Fell's col soaks in the cold and its oil
    /// stiffens; left out cold and wet long enough its bolt freezes shut;
    /// brought in under the col hut's roof it warms, and thaws.
    #[test]
    fn a_bolt_left_out_on_the_col_freezes_and_thaws_inside() {
        let mut vr = Vr::new("rime_fell");
        let col = Vec3::new(70.0, ground(&vr, 70.0, -262.0) + 1.0, -262.0);
        vr.stand(col + Vec3::new(3.0, -1.0, 0.0), 0.0);
        let gun = lay(&mut vr, "mosin", col);
        {
            let mut g = vr.app.world_mut().get_mut::<Gun>(gun).unwrap();
            g.firearm.set_bolt_travel(0.0);
        }
        vr.step(90 * 5);
        let w = weathering(&vr, gun);
        assert!(!w.sheltered && !w.carried, "out on the col: {w:?}");
        assert!(w.cold > 0.005, "soaking in the cold: {w:?}");
        // Soaked through, wet, and left a while.
        set(&mut vr, gun, |w| {
            w.cold = 0.95;
            w.wet = 0.5;
            w.left_out = FREEZE_SECS - 0.5;
        });
        vr.step(90 * 2);
        let w = weathering(&vr, gun);
        let g = vr.app.world().get::<Gun>(gun).unwrap();
        assert!(w.frozen && g.firearm.bolt_stuck, "frozen shut: {w:?}");
        assert!(
            (g.firearm.stiff - stiffness(w.cold)).abs() < 0.01 && g.firearm.stiff > 0.9,
            "its oil stiff: {}",
            g.firearm.stiff
        );
        // Into the hut.
        let hut = Vec3::new(20.0, ground(&vr, 20.0, -245.0) + 1.0, -245.0);
        *vr.app.world_mut().get_mut::<Transform>(gun).unwrap() = Transform::from_translation(hut);
        set(&mut vr, gun, |w| w.cold = THAW + 0.03);
        vr.step(90 * 12);
        let w = weathering(&vr, gun);
        let g = vr.app.world().get::<Gun>(gun).unwrap();
        assert!(w.sheltered, "under the roof");
        assert!(!w.frozen && !g.firearm.bolt_stuck, "thawed: {w:?}");
    }

    /// A scope cold from outside, brought into the col hut, fogs over; a
    /// glove across the lens clears it, and it fogs back only slowly.
    #[test]
    fn a_cold_scope_fogs_indoors_and_a_glove_clears_it() {
        let mut vr = Vr::new("rime_fell");
        let hut = Vec3::new(20.0, ground(&vr, 20.0, -245.0), -245.0);
        vr.stand(hut, 0.0);
        let at = hut + Vec3::new(0.3, 1.2, -0.4);
        let scope = lay(&mut vr, "pso_scope", at);
        set(&mut vr, scope, |w| w.cold = 0.8);
        vr.step(90 * 4);
        let w = weathering(&vr, scope);
        assert!(w.sheltered, "{w:?}");
        assert!(w.fog > 0.6, "fogged: {w:?}");
        // A glove across the eyepiece.
        let (eye, o_eye) = {
            let wld = vr.app.world();
            let o = wld.get::<Optic>(scope).unwrap();
            let gt = wld.get::<GlobalTransform>(scope).unwrap();
            (gt.transform_point(o.eye), o.eye)
        };
        let _ = o_eye;
        let side = Vec3::new(0.08, 0.0, 0.0);
        vr.hand(Side::Right, eye + side, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        for i in 0..=8 {
            vr.move_hand(Side::Right, eye + side - side * 2.0 * (i as f32 / 8.0));
            vr.step(1);
        }
        let w = weathering(&vr, scope);
        assert!(w.fog < 0.05, "wiped clear: {w:?}");
        vr.move_hand(Side::Right, hut + Vec3::new(-0.4, 1.0, 0.0));
        vr.step(90 * 3);
        let w = weathering(&vr, scope);
        assert!(w.fog < 0.25, "back only slowly: {w:?}");
    }

    /// In a downpour an uncovered AK soaks and grimes; a glove run along
    /// its receiver a few times wipes most of the grime and the wet off.
    #[test]
    fn rain_soaks_and_grimes_a_gun_and_a_glove_wipes_it() {
        let mut vr = Vr::new("training_yard");
        vr.app
            .world_mut()
            .resource_mut::<crate::launch::LaunchOptions>()
            .weather = Some("downpour".into());
        let feet = Vec3::new(0.0, ground(&vr, 0.0, 30.0), 30.0);
        vr.stand(feet, 0.0);
        let at = feet + Vec3::new(0.0, 1.2, -0.5);
        let gun = vr.spawn(
            "ak74",
            Transform::from_translation(at).looking_to(Vec3::X, Vec3::Y),
        );
        vr.app.world_mut().entity_mut(gun).insert(RigidBody::Static);
        vr.step(90 * 5);
        let w = weathering(&vr, gun);
        assert!(!w.sheltered, "{w:?}");
        assert!(w.wet > 0.03, "the rain on it: {w:?}");
        set(&mut vr, gun, |w| w.wet = 0.9);
        let before = vr.app.world().get::<Gun>(gun).unwrap().firearm.grime;
        vr.step(90 * 10);
        let grime = vr.app.world().get::<Gun>(gun).unwrap().firearm.grime;
        assert!(
            grime > before + 0.003,
            "grime gathering: {before} -> {grime}"
        );
        // Filthy: a glove along the receiver's top, back and forth.
        vr.app
            .world_mut()
            .get_mut::<Gun>(gun)
            .unwrap()
            .firearm
            .grime = 0.8;
        let (a, b) = {
            let wld = vr.app.world();
            let g = wld.get::<Gun>(gun).unwrap();
            let gt = wld.get::<GlobalTransform>(gun).unwrap();
            let (lo, hi) = g.layout.bounds();
            let y = hi.y + 0.02;
            (
                gt.transform_point(Vec3::new(0.0, y, -0.3)),
                gt.transform_point(Vec3::new(0.0, y, 0.05)),
            )
        };
        let _ = feet;
        vr.hand(Side::Left, a, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        for pass in 0..6 {
            let (from, to) = if pass % 2 == 0 { (a, b) } else { (b, a) };
            for i in 1..=10 {
                vr.move_hand(Side::Left, from.lerp(to, i as f32 / 10.0));
                vr.step(1);
            }
        }
        let g = vr.app.world().get::<Gun>(gun).unwrap();
        let w = weathering(&vr, gun);
        assert!(g.firearm.grime < 0.3, "wiped: {}", g.firearm.grime);
        assert!(w.wet < 0.6, "drier: {w:?}");
    }

    /// Taking hold of a gun cold-soaked deep in a frost stings the hand; a
    /// warm one doesn't.
    #[test]
    fn deep_cold_metal_stings_the_hand() {
        let stung = |cold: f32| -> bool {
            let mut vr = Vr::new("training_yard");
            let feet = Vec3::new(0.0, ground(&vr, 0.0, 30.0), 30.0);
            vr.stand(feet, 0.0);
            let gun = lay(&mut vr, "pm_pistol", feet + Vec3::new(0.15, 1.2, -0.3));
            set(&mut vr, gun, |w| w.cold = cold);
            let grip = vr.point(gun, |r| *r == GripRole::Primary);
            vr.hand(Side::Right, grip, Quat::IDENTITY, 0.0, 0.0);
            vr.step(3);
            let mut cursor = vr
                .app
                .world()
                .resource::<Messages<Haptic>>()
                .get_cursor_current();
            vr.grip(Side::Right, 1.0);
            let mut sting = false;
            for _ in 0..4 {
                vr.step(1);
                sting |= cursor
                    .read(vr.app.world().resource::<Messages<Haptic>>())
                    .any(|h| h.side == Side::Right && h.amplitude >= 0.55 && h.seconds <= 0.021);
            }
            assert_eq!(vr.hand_state(Side::Right).holding, Some(gun));
            sting
        };
        assert!(stung(0.9), "deep cold stings");
        assert!(!stung(0.1), "a warm gun doesn't");
    }
    /// The stiff action's gauge: how long a full stroke of a 10 cm bolt
    /// takes at a pull speed, warm and cold; a timid pull doesn't move a
    /// cold one at all.
    #[test]
    fn stiff_strokes_take_longer_and_timid_ones_stall() {
        use crate::weapons::{FIRM_PULL, drag};
        let stroke = |stiff: f32, pull: f32| -> Option<f32> {
            let (mut t, mut at) = (0.0, 0.0);
            let dt = 1.0 / 90.0;
            while at < 1.0 {
                // The hand runs ahead at `pull`; the bolt follows as it can.
                let want = ((t + dt) * pull / 0.1f32).min(1.0);
                let next = drag(at, want, stiff, pull, dt, 0.1);
                if next <= at && want > at {
                    return None;
                }
                at = next;
                t += dt;
                if t > 5.0 {
                    return None;
                }
            }
            Some(t)
        };
        let warm = stroke(0.0, 2.0).unwrap();
        let cold = stroke(1.0, 2.0).unwrap();
        println!("10 cm stroke at 2 m/s: warm {warm:.2} s, cold {cold:.2} s");
        assert!(cold > 1.5 * warm, "{cold} vs {warm}");
        assert!(
            stroke(1.0, FIRM_PULL * 0.5).is_none(),
            "a timid pull stalls"
        );
        assert!(
            stroke(0.0, FIRM_PULL * 0.5).is_some(),
            "warm, any pull will do"
        );
    }

    /// A cold-soaked Mosin's bolt: a timid pull slides on the handle; a
    /// firm one moves it, but brought back short and home again it's
    /// short-stroked and the trigger's dead; worked fully, it's right. A
    /// warm one does the same short stroke without complaint.
    #[test]
    fn a_cold_bolt_drags_and_short_strokes_unless_worked_fully() {
        use crate::weapons::feel::vr_tests::{Range, glide_side, gun_mut, hold};
        use extraction_core::firearm::Malfunction;
        let setup = |cold: f32| -> (Range, Vec3, Vec3, Vec3, f32) {
            let mut r = hold("mosin", false, false);
            set(&mut r.vr, r.gun, |w| w.cold = cold);
            r.vr.step(3);
            let handle = r.vr.point(r.gun, |p| *p == GripRole::Handle(0));
            let (up, back) = {
                let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
                (gt.up().as_vec3(), gt.back().as_vec3())
            };
            let travel = gun_mut(&mut r).layout.handle.as_ref().unwrap().travel;
            r.vr.hand(Side::Left, handle, Quat::IDENTITY, 0.0, 0.0);
            r.vr.step(30);
            r.vr.trigger(Side::Left, 1.0);
            r.vr.step(2);
            assert!(r.vr.hand_state(Side::Left).handle.is_some(), "on the bolt");
            let lifted = handle + up * 0.045;
            glide_side(&mut r, Side::Left, handle, lifted, 15);
            assert!(gun_mut(&mut r).firearm.handle_lift > 0.9, "lifted");
            (r, lifted, back, up, travel)
        };
        let bolt = |r: &mut Range| gun_mut(r).firearm.bolt_travel;

        let (mut r, lifted, back, _, travel) = setup(1.0);
        assert!(gun_mut(&mut r).firearm.stiff > 0.9, "stiff");
        // Timid: half the stroke over two seconds.
        glide_side(
            &mut r,
            Side::Left,
            lifted,
            lifted + back * travel * 0.5,
            180,
        );
        assert!(bolt(&mut r) < 0.1, "a timid pull slides: {}", bolt(&mut r));
        // Firm but short: back a little way, and home again.
        let from = r.vr.hand_state(Side::Left).world.translation;
        glide_side(&mut r, Side::Left, from, from + back * travel * 0.6, 5);
        r.vr.step(3);
        let mid = bolt(&mut r);
        assert!((0.1..0.95).contains(&mid), "partway: {mid}");
        let at = r.vr.hand_state(Side::Left).world.translation;
        glide_side(&mut r, Side::Left, at, at - back * travel * 0.8, 5);
        r.vr.step(3);
        assert_eq!(
            gun_mut(&mut r).firearm.malfunction,
            Some(Malfunction::ShortStroke),
            "short-stroked"
        );
        // All the way, firmly: right again.
        let at = r.vr.hand_state(Side::Left).world.translation;
        glide_side(&mut r, Side::Left, at, at + back * travel * 1.6, 15);
        r.vr.step(5);
        assert!(bolt(&mut r) > 0.97, "all the way: {}", bolt(&mut r));
        assert_eq!(gun_mut(&mut r).firearm.malfunction, None, "worked fully");

        // Warm: the same short stroke is just a short stroke.
        let (mut r, lifted, back, _, travel) = setup(0.0);
        glide_side(&mut r, Side::Left, lifted, lifted + back * travel * 0.5, 5);
        r.vr.step(3);
        let at = r.vr.hand_state(Side::Left).world.translation;
        glide_side(&mut r, Side::Left, at, at - back * travel * 0.6, 5);
        r.vr.step(3);
        assert_eq!(gun_mut(&mut r).firearm.malfunction, None, "warm");
    }

    /// A hang-fire in the hand: the round goes off a moment after the
    /// click, wherever the muzzle has turned to by then; opened on before
    /// it goes, it bursts out of battery into the hand.
    #[test]
    fn a_hang_fire_goes_where_the_muzzle_is_then_or_into_the_hand() {
        use crate::weapons::ballistics::{Hit, Shot};
        use crate::weapons::feel::vr_tests::{gun_mut, hold};
        use extraction_core::firearm::{Hang, Round};
        let hang = |r: &mut crate::weapons::feel::vr_tests::Range| {
            let mut g = gun_mut(r);
            let f = &mut g.firearm;
            f.chambers[0] = Some(Round::Live);
            f.hang = Some(Hang {
                left: 0.5,
                barrel: 0,
                chamber: 0,
                load: Load::Damp,
            });
        };
        let mut r = hold("mosin", false, false);
        r.vr.step(10);
        hang(&mut r);
        // Turned 30 degrees left before it goes.
        let grip = r.vr.hand_state(Side::Right).world;
        let turn = Quat::from_rotation_y(0.52);
        r.vr.hand(
            Side::Right,
            grip.translation,
            turn * grip.rotation,
            1.0,
            0.0,
        );
        let mut shots = r
            .vr
            .app
            .world()
            .resource::<Messages<Shot>>()
            .get_cursor_current();
        let mut shot = None;
        let mut ticks = 0;
        for _ in 0..90 {
            r.vr.step(1);
            ticks += 1;
            if let Some(s) = shots
                .read(r.vr.app.world().resource::<Messages<Shot>>())
                .next()
            {
                shot = Some(s.direction);
                break;
            }
        }
        let shot = shot.expect("it went off");
        assert!(ticks as f32 / 90.0 >= 0.4, "late: {ticks} ticks");
        let bore =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .rotation()
                * Vec3::NEG_Z;
        assert!(
            shot.angle_between(bore) < 0.1,
            "along the bore as it is then"
        );
        assert!(
            shot.angle_between(grip.rotation * Vec3::NEG_Z) > 0.3,
            "not where it pointed at the click"
        );

        // Hanging, the bolt lifted: out of battery, into the hand.
        let mut r = hold("mosin", false, false);
        r.vr.step(10);
        hang(&mut r);
        let mut hurt =
            r.vr.app
                .world()
                .resource::<Messages<Hit>>()
                .get_cursor_current();
        gun_mut(&mut r).firearm.set_handle_lift(1.0);
        let mut harmed = false;
        for _ in 0..5 {
            r.vr.step(1);
            harmed |= hurt
                .read(r.vr.app.world().resource::<Messages<Hit>>())
                .any(|h| h.damage > 0.0 && h.shooter.is_none());
        }
        assert!(harmed, "the hand hurt");
        assert!(gun_mut(&mut r).firearm.condition < 1.0, "the gun hurt");
    }
}
