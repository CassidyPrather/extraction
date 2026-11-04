//! Gunsmithing at the vice (`extraction_core::gunsmith`): what a gun has
//! been made into, applied to the gun.
//!
//! A gun keeps what it was made as ([`Factory`]): its definition and its
//! layout. Whenever its build changes (a cut, a stock off or on, a part
//! swapped), they're worked out afresh from that ([`apply_builds`]): the
//! muzzle speed and group, the kick, where the muzzle and the butt are,
//! its mass and collider, and how much it flashes ([`Built`]).

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::firearm::FirearmDef;
use extraction_core::gunsmith::{self as gs, Build};

use super::Gun;
use super::layout::{GunLayout, Shape};
use crate::items::{Catalog, Item};

pub struct GunsmithPlugin;

impl Plugin for GunsmithPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Worked>()
            .add_observer(setup_hacksaw)
            .add_systems(PreUpdate, (apply_builds, follow_muzzles).chain())
            .add_systems(Update, fit_jaws)
            .add_systems(
                FixedUpdate,
                (
                    clamp_guns,
                    fit_parts,
                    saw_barrels,
                    mark_stock_screws,
                    unscrew_stocks,
                    draw_bolts,
                )
                    .chain()
                    .after(crate::interact::InteractSystems::Follow),
            );
    }
}

/// Something done at the vice, for the ear (`audio`) and anyone else.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Worked {
    pub gun: Entity,
    /// Where it happened.
    pub at: Vec3,
    pub what: Work,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    /// A gun let go of into the jaws, and they closed on it.
    Clamped,
    /// A stroke of the hacksaw across a barrel (one each 15 cm of blade).
    SawStroke,
    /// Through: the end's off and falls (as scrap: it lands as scrap
    /// does).
    Cut,
    /// A quarter turn of the spanner on a stock's screw.
    ScrewTurn,
    StockOff,
    StockOn,
    /// A bolt drawn out past its stop, and one slid home into a receiver.
    BoltOut,
    BoltIn,
}

/// A gun as it was made: kept to work its build out from.
#[derive(Component, Clone, Debug)]
pub struct Factory {
    pub def: FirearmDef,
    pub layout: GunLayout,
    pub mass: f32,
    pub length: f32,
    pub barrel_mm: f32,
    /// The build last applied.
    pub applied: Build,
}

/// A gun as built, against as made: what the systems that don't keep
/// their own copy read.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Built {
    pub mass: f32,
    pub length: f32,
    /// The muzzle flash's size, against as made.
    pub flash: f32,
    /// The barrel (mm).
    pub barrel_mm: f32,
}

/// Cut a gun's parts to `front..back` along Z (gun space: the muzzle to
/// -Z): what lies wholly outside goes, what straddles is shortened.
fn clip_parts(layout: &mut GunLayout, front: f32, back: f32) {
    layout.parts.retain_mut(|p| {
        let z = p.at.translation.z;
        let half = match p.shape {
            Shape::Box(s) => s.z * 0.5,
            Shape::Cylinder(_, l) => l * 0.5,
        };
        let (lo, hi) = ((z - half).max(front), (z + half).min(back));
        if hi <= lo {
            return false;
        }
        let len = hi - lo;
        match &mut p.shape {
            Shape::Box(s) => s.z = len,
            Shape::Cylinder(_, l) => *l = len,
        }
        p.at.translation.z = (lo + hi) * 0.5;
        true
    });
}

/// A gun's build, worked out afresh when it changes.
#[allow(clippy::type_complexity)]
pub fn apply_builds(
    mut commands: Commands,
    mut guns: Query<(Entity, &mut Gun, &Item, Option<&mut Factory>)>,
    catalog: Res<Catalog>,
) {
    for (e, mut gun, item, factory) in &mut guns {
        let def = catalog.get(item.kind());
        let Some(barrel_mm) = extraction_core::report::barrel_mm(&def.id) else {
            continue;
        };
        let build = gun.firearm.build;
        let Some(mut factory) = factory else {
            let f = Factory {
                def: gun.firearm.def.clone(),
                layout: gun.layout.clone(),
                mass: def.mass,
                length: def.size_m()[2],
                barrel_mm,
                applied: Build::default(),
            };
            // Its build (if not as made) applies next tick.
            commands.entity(e).insert(f);
            continue;
        };
        if factory.applied == build {
            continue;
        }
        let id = def.id.as_str();
        let cartridge = factory.def.cartridge.clone();
        let mm = gs::barrel_now(&build, factory.barrel_mm);
        let mass = gs::mass_kg(id, &cartridge, factory.mass, &build, factory.barrel_mm);
        let (climb, push) = gs::kick_factor(mass, factory.mass, &build);
        let mut d = factory.def.clone();
        d.muzzle_velocity = gs::muzzle_speed(
            &cartridge,
            factory.def.muzzle_velocity,
            factory.barrel_mm,
            mm,
        );
        d.spread_moa *= gs::spread_factor(&build, factory.barrel_mm);
        d.kick.climb *= climb;
        d.kick.push *= push;
        gun.firearm.def = d;
        // The muzzle comes back by what was cut; what lay ahead of it
        // goes, and the stock with its butt.
        let mut layout = factory.layout.clone();
        let cut = (factory.barrel_mm - mm) / 1000.0;
        layout.muzzle.z += cut;
        let (_, hi) = factory.layout.bounds();
        let back = if build.stock {
            hi.z
        } else {
            layout.grip.translation.z + 0.06
        };
        let front = layout.muzzle.z;
        clip_parts(&mut layout, front, back);
        if let Some(s) = layout.support.as_mut() {
            // The other hand on what's left of the forend.
            s.z = s.z.max(layout.muzzle.z + 0.06);
        }
        commands.entity(e).try_insert((
            super::gun_collider(&layout),
            Mass(mass),
            Built {
                mass,
                length: gs::length_m(id, factory.length, &build, factory.barrel_mm),
                flash: gs::flash_factor(&cartridge, factory.barrel_mm, mm),
                barrel_mm: mm,
            },
        ));
        if build.barrel_mm.is_some() {
            // The bayonet's lug went with the muzzle.
            commands.entity(e).try_remove::<super::melee::Bayonet>();
        }
        gun.layout = layout;
        gun.mass = Some(mass);
        factory.applied = build;
    }
}

// --- The vice -------------------------------------------------------------

/// The vice's jaws: a gun let go of within [`CLAMP_REACH`] is clamped in
/// them, its receiver between the jaws and its barrel out over the end of
/// the bench.
#[derive(Component, Debug, Default)]
pub struct ViceJaws;

/// A gun clamped in a vice.
#[derive(Component, Debug)]
pub struct InVice {
    pub vice: Entity,
    /// Turns of the spanner on its stock screw so far (radians).
    pub screw: f32,
    /// The hacksaw's work at one spot: (mm of barrel left there, metres
    /// of blade drawn across it).
    pub sawing: Option<(f32, f32)>,
}

/// A gun let go of this near the jaws is clamped (m).
pub const CLAMP_REACH: f32 = 0.2;
/// Blade drawn across one spot to cut a rifle's barrel through (m), and
/// a double's two.
pub const SAW_RIFLE: f32 = 1.2;
pub const SAW_DOUBLE: f32 = 2.0;
/// A stroke counts only this fast along the blade (m/s)...
pub const SAW_SPEED: f32 = 0.15;
/// ...and only this near the barrel (m), and stays on its spot this near.
pub const SAW_TOUCH: f32 = 0.025;
pub const SAW_SPOT: f32 = 0.03;
/// Turns of the spanner to have a stock's screw out (radians).
pub const STOCK_SCREW: f32 = std::f32::consts::TAU * 2.0;
/// A part brought this near where it goes, and let go, is fitted (m).
pub const FIT_REACH: f32 = 0.1;
/// Drawn this far past the bolt's stop, a Mosin's bolt comes out (m).
pub const BOLT_PAST: f32 = 0.04;

/// In the hacksaw's space: the handle, and the blade's two ends.
pub const SAW_GRIP: Vec3 = Vec3::new(0.0, -0.01, 0.16);
pub const BLADE_A: Vec3 = Vec3::new(0.0, -0.045, 0.06);
pub const BLADE_B: Vec3 = Vec3::new(0.0, -0.045, -0.2);

/// The stock's screw, marked for the spanner while the gun's in the vice.
#[derive(Component, Debug)]
pub struct StockScrew;

/// The item a gun's part comes off as.
pub fn part_item(id: &str, part: &str) -> String {
    format!("{part}_{id}")
}

/// Where a gun is held in the jaws (gun space): its receiver, just ahead
/// of the grip.
pub fn clamp_point(layout: &GunLayout) -> Vec3 {
    layout.grip.translation + Vec3::new(0.0, 0.05, -0.12)
}

/// The breech (gun space): where the barrel starts, a little ahead of
/// the receiver.
pub fn breech(layout: &GunLayout) -> Vec3 {
    Vec3::new(
        layout.muzzle.x,
        layout.muzzle.y,
        layout.grip.translation.z - 0.2,
    )
}

/// Where a stock's screw is (gun space): through the tang behind the grip.
pub fn screw_point(layout: &GunLayout) -> Vec3 {
    layout.grip.translation + Vec3::new(0.0, -0.02, 0.07)
}

/// Where a stock goes back on (gun space): behind the grip.
pub fn stock_point(layout: &GunLayout) -> Vec3 {
    layout.grip.translation + Vec3::new(0.0, 0.0, 0.18)
}

/// The jaws on a fitted vice.
pub fn fit_jaws(
    mut commands: Commands,
    props: Query<
        (Entity, &crate::progression::TruckUpgradeProp),
        Added<crate::progression::TruckUpgradeProp>,
    >,
) {
    for (e, p) in &props {
        if p.0 == extraction_core::progression::Upgrade::BenchVice {
            commands.entity(e).with_child((
                Name::new("Vice jaws"),
                ViceJaws,
                Transform::from_translation(Vec3::Y * 0.09),
                Visibility::default(),
            ));
        }
    }
}

/// A gun let go of at the jaws is clamped in them; taken up, it's free.
#[allow(clippy::type_complexity)]
pub fn clamp_guns(
    mut commands: Commands,
    mut released: RemovedComponents<crate::interact::Held>,
    guns: Query<(&Gun, &GlobalTransform), (Without<crate::interact::Held>, Without<InVice>)>,
    taken: Query<Entity, (With<InVice>, Added<crate::interact::Held>)>,
    jaws: Query<(Entity, &GlobalTransform), With<ViceJaws>>,
    mut worked: MessageWriter<Worked>,
) {
    for e in &taken {
        commands
            .entity(e)
            .remove::<InVice>()
            .insert(RigidBody::Dynamic);
    }
    for e in released.read() {
        let Ok((gun, gt)) = guns.get(e) else {
            continue;
        };
        let at = gt.transform_point(clamp_point(&gun.layout));
        let Some((vice, jgt)) = jaws
            .iter()
            .find(|(_, j)| j.translation().distance(at) < CLAMP_REACH)
        else {
            continue;
        };
        worked.write(Worked {
            gun: e,
            at: jgt.translation(),
            what: Work::Clamped,
        });
        // Level, the muzzle out along the bench, the receiver in the jaws.
        let rot = jgt.rotation();
        let pos = jgt.translation() - rot * clamp_point(&gun.layout);
        commands.entity(e).insert((
            InVice {
                vice,
                screw: 0.0,
                sawing: None,
            },
            RigidBody::Static,
            Transform::from_translation(pos).with_rotation(rot),
            LinearVelocity::ZERO,
            AngularVelocity::ZERO,
        ));
    }
}

/// The closest points of segments `a0..a1` and `b0..b1`: (on a, on b).
fn closest(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (a1 - a0, b1 - b0, a0 - b0);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (s, t) = if a <= 1e-9 && e <= 1e-9 {
        (0.0, 0.0)
    } else if a <= 1e-9 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= 1e-9 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let den = a * e - b * b;
            let mut s = if den > 1e-9 {
                ((b * f - c * e) / den).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    (a0 + d1 * s, b0 + d2 * t)
}

/// A held hacksaw drawn back and forth across a clamped gun's barrel cuts
/// it there, once enough blade has gone across the one spot; the end
/// that's cut off drops as scrap.
#[allow(clippy::too_many_arguments)]
pub fn saw_barrels(
    mut commands: Commands,
    saws: Query<(&Item, &GlobalTransform, &crate::interact::Held)>,
    mut guns: Query<(
        Entity,
        &mut Gun,
        &Item,
        &GlobalTransform,
        &mut InVice,
        Option<&Factory>,
    )>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut last: Local<std::collections::HashMap<Entity, Vec3>>,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
    mut worked: MessageWriter<Worked>,
) {
    let dt = time.delta_secs().max(1e-4);
    let mut seen = Vec::new();
    for (item, sgt, held) in &saws {
        if catalog.get(item.kind()).id != "hacksaw" {
            continue;
        }
        let (a, b) = (sgt.transform_point(BLADE_A), sgt.transform_point(BLADE_B));
        let axis = (b - a).normalize_or_zero();
        let mid = (a + b) * 0.5;
        let key = held.hand;
        seen.push(key);
        let moved = last
            .insert(key, mid)
            .map_or(0.0, |p| (mid - p).dot(axis).abs());
        for (gun_e, mut gun, gitem, ggt, mut vice, factory) in &mut guns {
            let id = &catalog.get(gitem.kind()).id;
            if !extraction_core::gunsmith::workable(id).0 {
                continue;
            }
            let Some(factory) = factory else {
                continue;
            };
            let (b0, b1) = (
                ggt.transform_point(breech(&gun.layout)),
                ggt.transform_point(gun.layout.muzzle),
            );
            let (on_blade, on_barrel) = closest(a, b, b0, b1);
            if on_blade.distance(on_barrel) > SAW_TOUCH {
                continue;
            }
            let now_mm =
                extraction_core::gunsmith::barrel_now(&gun.firearm.build, factory.barrel_mm);
            let left = now_mm - on_barrel.distance(b1) * 1000.0;
            if left < extraction_core::gunsmith::SHORTEST || left > now_mm - 20.0 {
                continue;
            }
            if moved / dt < SAW_SPEED {
                continue;
            }
            let (spot, drawn) = match vice.sawing {
                Some((s, d)) if (s - left).abs() < SAW_SPOT * 1000.0 => (s, d + moved),
                _ => (left, moved),
            };
            let need = if gun.firearm.def.barrels > 1 {
                SAW_DOUBLE
            } else {
                SAW_RIFLE
            };
            if (drawn / 0.15).floor() > ((drawn - moved) / 0.15).floor() {
                crate::interact::tools::pulse(&mut haptics, held.side, (0.25, 0.02));
                worked.write(Worked {
                    gun: gun_e,
                    at: on_barrel,
                    what: Work::SawStroke,
                });
            }
            if drawn < need {
                vice.sawing = Some((spot, drawn));
                continue;
            }
            vice.sawing = None;
            gun.firearm.build.barrel_mm = Some(spot);
            crate::interact::tools::pulse(&mut haptics, held.side, (0.55, 0.06));
            worked.write(Worked {
                gun: gun_e,
                at: on_barrel,
                what: Work::Cut,
            });
            // The end comes away: scrap.
            if let Some(scrap) = catalog.kind("scrap_metal") {
                crate::items::spawn_item(
                    &mut commands,
                    &catalog,
                    crate::items::SpawnItem {
                        kind: scrap,
                        transform: Transform::from_translation(b1 + Vec3::Y * 0.05),
                        condition: 1.0,
                    },
                );
            }
        }
    }
    last.retain(|k, _| seen.contains(k));
}

/// The stock's screw is there for the spanner while a gun that can lose
/// its stock is in the vice with it on.
pub fn mark_stock_screws(
    mut commands: Commands,
    guns: Query<(Entity, &Gun, &Item, Option<&InVice>, Option<&Children>)>,
    screws: Query<(), With<StockScrew>>,
    catalog: Res<Catalog>,
) {
    for (e, gun, item, vice, children) in &guns {
        let has = children.is_some_and(|c| c.iter().any(|c| screws.contains(c)));
        let want = vice.is_some()
            && gun.firearm.build.stock
            && extraction_core::gunsmith::workable(&catalog.get(item.kind()).id).1;
        if want && !has {
            commands.entity(e).with_child((
                Name::new("Stock screw"),
                StockScrew,
                crate::destruction::hooks::Unboltable {
                    needs: STOCK_SCREW,
                    axis: Vec3::Y,
                    turned: 0.0,
                },
                Transform::from_translation(screw_point(&gun.layout)),
            ));
        } else if !want && has {
            for c in children.into_iter().flatten() {
                if screws.contains(*c) {
                    commands.entity(*c).despawn();
                }
            }
        }
    }
}

/// Turns of the spanner on a stock's screw: two, and the stock comes off.
pub fn unscrew_stocks(
    mut commands: Commands,
    mut turned: MessageReader<crate::destruction::hooks::Turned>,
    screws: Query<&ChildOf, With<StockScrew>>,
    mut guns: Query<(&mut Gun, &Item, &GlobalTransform, &mut InVice)>,
    catalog: Res<Catalog>,
    mut worked: MessageWriter<Worked>,
) {
    for t in turned.read() {
        let Ok(parent) = screws.get(t.target) else {
            continue;
        };
        let Ok((mut gun, item, gt, mut vice)) = guns.get_mut(parent.parent()) else {
            continue;
        };
        let quarter = std::f32::consts::FRAC_PI_2;
        let was = vice.screw;
        vice.screw += t.angle.abs();
        let at = gt.transform_point(screw_point(&gun.layout));
        if (vice.screw / quarter).floor() > (was / quarter).floor() {
            worked.write(Worked {
                gun: parent.parent(),
                at,
                what: Work::ScrewTurn,
            });
        }
        if vice.screw < STOCK_SCREW || !gun.firearm.build.stock {
            continue;
        }
        vice.screw = 0.0;
        gun.firearm.build.stock = false;
        worked.write(Worked {
            gun: parent.parent(),
            at,
            what: Work::StockOff,
        });
        let id = part_item(&catalog.get(item.kind()).id, "stock");
        if let Some(kind) = catalog.kind(&id) {
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        gt.transform_point(stock_point(&gun.layout)) + Vec3::Y * 0.05,
                    )
                    .with_rotation(gt.rotation()),
                    condition: gun.firearm.condition,
                },
            );
        }
    }
}

/// A part let go of where it goes on a clamped gun of its model is fitted:
/// a stock behind the grip, a bolt into the open receiver.
#[allow(clippy::too_many_arguments)]
pub fn fit_parts(
    mut commands: Commands,
    mut released: RemovedComponents<crate::interact::Held>,
    parts: Query<(&Item, &GlobalTransform)>,
    mut guns: Query<(Entity, &mut Gun, &Item, &GlobalTransform), With<InVice>>,
    catalog: Res<Catalog>,
    mut worked: MessageWriter<Worked>,
) {
    use extraction_core::gunsmith::{BOLT_SHARE, fit_part};
    for e in released.read() {
        let Ok((part, pgt)) = parts.get(e) else {
            continue;
        };
        let pid = catalog.get(part.kind()).id.clone();
        let at = pgt.translation();
        for (gun_e, mut gun, item, ggt) in &mut guns {
            let id = &catalog.get(item.kind()).id;
            let b = gun.firearm.build;
            if pid == part_item(id, "stock")
                && !b.stock
                && ggt.transform_point(stock_point(&gun.layout)).distance(at) < FIT_REACH
            {
                gun.firearm.build.stock = true;
                commands.entity(e).despawn();
                worked.write(Worked {
                    gun: gun_e,
                    at,
                    what: Work::StockOn,
                });
                break;
            }
            let way = gun
                .layout
                .handle
                .as_ref()
                .map(|h| ggt.transform_point(h.grab + Vec3::Z * h.travel));
            if pid == part_item(id, "bolt")
                && b.bolt_out
                && way.is_some_and(|w| w.distance(at) < FIT_REACH)
            {
                let f = &mut gun.firearm;
                let (cond, dev) = fit_part(f.condition, b.bolt_dev, part.condition, BOLT_SHARE);
                f.condition = cond;
                f.build.bolt_dev = dev;
                f.build.bolt_out = false;
                f.set_bolt_travel(1.0);
                commands.entity(e).despawn();
                worked.write(Worked {
                    gun: gun_e,
                    at,
                    what: Work::BoltIn,
                });
                break;
            }
        }
    }
}

/// A Mosin's bolt, drawn back to its stop in the vice and on past it, comes
/// out into the hand that drew it, as itself (its condition with it).
#[allow(clippy::too_many_arguments)]
pub fn draw_bolts(
    mut commands: Commands,
    mut hands: Query<(
        Entity,
        &mut crate::interact::Hand,
        &crate::rig::RigHand,
        &ChildOf,
    )>,
    mut guns: Query<(Entity, &mut Gun, &Item, &GlobalTransform), With<InVice>>,
    catalog: Res<Catalog>,
    mut actions: MessageWriter<crate::interact::HandAction>,
    mut worked: MessageWriter<Worked>,
) {
    use extraction_core::gunsmith::part_condition;
    for (_, mut hand, rig, player) in &mut hands {
        let Some((gun_e, _)) = hand.handle else {
            continue;
        };
        let Ok((_, mut gun, item, gt)) = guns.get_mut(gun_e) else {
            continue;
        };
        let id = catalog.get(item.kind()).id.clone();
        if !extraction_core::gunsmith::workable(&id).2 || gun.firearm.bolt_travel < 0.99 {
            continue;
        }
        let Some(h) = gun.layout.handle.as_ref() else {
            continue;
        };
        let stop = gt.transform_point(h.grab + Vec3::Z * h.travel);
        let back = gt.rotation() * Vec3::Z;
        if (hand.world.translation - stop).dot(back) < BOLT_PAST {
            continue;
        }
        let Some(kind) = catalog.kind(&part_item(&id, "bolt")) else {
            continue;
        };
        worked.write(Worked {
            gun: gun_e,
            at: stop,
            what: Work::BoltOut,
        });
        let f = &mut gun.firearm;
        f.build.bolt_out = true;
        // A round on the bolt face goes with it, onto the bench.
        let condition = part_condition(f.condition, f.build.bolt_dev);
        hand.handle = None;
        let bolt = crate::items::spawn_item(
            &mut commands,
            &catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::from_translation(hand.world.translation)
                    .with_rotation(gt.rotation()),
                condition,
            },
        );
        actions.write(crate::interact::HandAction::Grab {
            player: player.parent(),
            side: rig.side,
            item: bolt,
            point: None,
        });
    }
}

/// A hacksaw: held by its handle.
fn setup_hacksaw(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    if catalog.get(item.kind()).id != "hacksaw" {
        return;
    }
    commands
        .entity(add.entity)
        .insert(crate::interact::Grabbable {
            points: vec![crate::interact::GrabPoint::new(
                Transform::from_translation(SAW_GRIP),
                crate::interact::GripRole::Primary,
                0.12,
            )],
        });
}

/// A muzzle mount follows the muzzle back when the barrel's cut.
pub fn follow_muzzles(
    guns: Query<(&Gun, &Children), Changed<Gun>>,
    mut sockets: Query<(&crate::interact::Socket, &mut Transform)>,
) {
    use crate::interact::SocketKind;
    use extraction_core::items::Mount;
    for (gun, children) in &guns {
        for c in children.iter() {
            if let Ok((s, mut t)) = sockets.get_mut(c) {
                if s.kind == SocketKind::Mount(Mount::Muzzle) && t.translation != gun.layout.muzzle
                {
                    t.translation = gun.layout.muzzle;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::GripRole;
    use crate::rig::Side;
    use crate::weapons::feel::vr_tests::{Range, glide_side, gun_mut, hold, take_in};
    use extraction_core::gunsmith::{BOLT_SHARE, fit_part};

    /// Jaws at where `r`'s gun's receiver is now, the gun let go into them.
    fn clamp(r: &mut Range, gun: Entity, side: Side) -> Entity {
        let gt = *r.vr.app.world().get::<GlobalTransform>(gun).unwrap();
        let layout = r.vr.app.world().get::<Gun>(gun).unwrap().layout.clone();
        let jaws =
            r.vr.app
                .world_mut()
                .spawn((
                    ViceJaws,
                    Transform::from_translation(gt.transform_point(clamp_point(&layout)))
                        .with_rotation(gt.rotation()),
                ))
                .id();
        r.vr.step(2);
        r.vr.grip(side, 0.0);
        r.vr.step(4);
        assert!(
            r.vr.app.world().get::<InVice>(gun).is_some(),
            "clamped in the vice"
        );
        jaws
    }

    /// Something lying loose, taken into the `side` hand where it is.
    fn pick_up(r: &mut Range, side: Side, item: Entity) {
        let at =
            r.vr.app
                .world()
                .get::<GlobalTransform>(item)
                .unwrap()
                .translation();
        r.vr.app
            .world_mut()
            .entity_mut(item)
            .insert(RigidBody::Static);
        r.vr.hand(side, at, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(3);
        r.vr.grip(side, 1.0);
        r.vr.step(3);
        r.vr.app
            .world_mut()
            .entity_mut(item)
            .insert(RigidBody::Dynamic);
        assert_eq!(r.vr.hand_state(side).holding, Some(item), "picked up");
    }

    /// A gun model's top-level node names.
    fn nodes(id: &str) -> Vec<String> {
        let path = crate::asset_root_path().join(format!("models/guns/{id}.glb"));
        let b = std::fs::read(&path).unwrap_or_else(|_| panic!("{path:?}"));
        let n = u32::from_le_bytes([b[12], b[13], b[14], b[15]]) as usize;
        let j: serde_json::Value = serde_json::from_slice(&b[20..20 + n]).unwrap();
        j["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["name"].as_str().map(str::to_owned))
            .collect()
    }

    /// Every gun that can be worked at the vice draws what's been done to
    /// it: its model has a barrel node to draw short if it can be cut, a
    /// butt to take away and a stock item to come off as if its stock
    /// comes off, and a bolt node and item if its bolt comes out.
    #[test]
    fn every_workable_gun_shows_its_work() {
        let catalog = Catalog::load();
        let mut bad = Vec::new();
        for (_, def) in catalog.iter() {
            if def.firearm.is_none() {
                continue;
            }
            let id = def.id.as_str();
            let (cut, stock, bolt) = extraction_core::gunsmith::workable(id);
            if !(cut || stock || bolt) {
                continue;
            }
            let names = nodes(id);
            let has = |n: &str| names.iter().any(|m| m == n);
            if cut && !(has("static_barrel") || has("barrels")) {
                bad.push(format!("{id}: no barrel node to cut"));
            }
            if stock && !has("static_butt") {
                bad.push(format!("{id}: no butt node"));
            }
            if stock && catalog.kind(&part_item(id, "stock")).is_none() {
                bad.push(format!("{id}: no stock item"));
            }
            if bolt && !has("turnbolt") {
                bad.push(format!("{id}: no bolt node"));
            }
            if bolt && catalog.kind(&part_item(id, "bolt")).is_none() {
                bad.push(format!("{id}: no bolt item"));
            }
        }
        // And no part for a gun it can't come off.
        for (_, def) in catalog.iter().filter(|(_, d)| d.has_tag("gun_part")) {
            let from = catalog.iter().any(|(_, g)| {
                let (_, stock, bolt) = extraction_core::gunsmith::workable(&g.id);
                (stock && part_item(&g.id, "stock") == def.id)
                    || (bolt && part_item(&g.id, "bolt") == def.id)
            });
            if !from {
                bad.push(format!("{}: comes off no gun", def.id));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Everything the vice needs or makes, and the rounds and cans that
    /// came with it, can be had in play: found (a loot table), given (a
    /// fitting's gift), made at the bench, or taken off a gun; a loose
    /// round if its box can.
    #[test]
    fn what_the_vice_needs_and_makes_can_be_had() {
        use extraction_core::fabrication::{Make, every_recipe};
        use extraction_core::progression::Upgrade;
        let catalog = Catalog::load();
        let loot = std::fs::read_to_string(crate::asset_root_path().join("data/loot.ron")).unwrap();
        let had = |id: &str| -> bool {
            loot.contains(&format!("id: \"{id}\""))
                || Upgrade::ALL
                    .iter()
                    .any(|u| crate::progression::gifts(*u).iter().any(|g| g.0 == id))
                || every_recipe().any(
                    |r| matches!(r.makes, Make::Items(out) if out.iter().any(|(o, _)| *o == id)),
                )
                || catalog.iter().any(|(_, g)| {
                    let (_, stock, bolt) = extraction_core::gunsmith::workable(&g.id);
                    (stock && part_item(&g.id, "stock") == id)
                        || (bolt && part_item(&g.id, "bolt") == id)
                })
        };
        let mut ids: Vec<String> = vec![
            "hacksaw".into(),
            "can_suppressor".into(),
            "ammo_545_us".into(),
            "ammo_762x39_us".into(),
        ];
        ids.extend(
            catalog
                .iter()
                .filter(|(_, d)| d.has_tag("gun_part"))
                .map(|(_, d)| d.id.clone()),
        );
        let mut bad: Vec<String> = ids.iter().filter(|id| !had(id)).cloned().collect();
        // A loose round comes out of its box.
        for round in ["round_545_us", "round_762x39_us"] {
            let r = catalog.get(catalog.kind(round).unwrap());
            let boxed = catalog.iter().any(|(_, b)| {
                b.rounds > 1 && b.cartridge == r.cartridge && b.load == r.load && had(&b.id)
            });
            if !boxed {
                bad.push(round.into());
            }
        }
        assert!(bad.is_empty(), "can't be had: {bad:?}");
    }

    /// Whatever the hand holds, let go (the grip closed on it, then opened:
    /// a bolt drawn out came into the hand with the grip open).
    fn let_go(r: &mut Range, side: Side) {
        r.vr.grip(side, 1.0);
        r.vr.step(2);
        r.vr.grip(side, 0.0);
        r.vr.step(5);
    }

    /// The first loose item of `id`.
    fn find(r: &mut Range, id: &str) -> Option<Entity> {
        let catalog = r.vr.app.world().resource::<Catalog>().clone();
        let mut q = r.vr.app.world_mut().query::<(Entity, &Item)>();
        q.iter(r.vr.app.world())
            .find(|(_, i)| catalog.get(i.kind()).id == id)
            .map(|(e, _)| e)
    }

    fn count(r: &mut Range, id: &str) -> usize {
        let catalog = r.vr.app.world().resource::<Catalog>().clone();
        let mut q = r.vr.app.world_mut().query::<&Item>();
        q.iter(r.vr.app.world())
            .filter(|i| catalog.get(i.kind()).id == id)
            .count()
    }

    /// A Mosin clamped in the vice, the right hand saws across its barrel
    /// 250 mm out from the breech: through, it's an obrez: the barrel's
    /// that long, the bullet leaves slower, the muzzle's come back, and
    /// the end lies on the bench as scrap.
    #[test]
    fn sawing_a_mosin_down_to_an_obrez() {
        let mut r = hold("mosin", false, false);
        let gun = r.gun;
        clamp(&mut r, gun, Side::Right);
        let before = gun_mut(&mut r).layout.muzzle;
        // (The yard's buildings' loot may have scrap in it already.)
        let scrap_before = count(&mut r, "scrap_metal");
        let near = r.feet + Vec3::new(0.35, 1.0, 0.0);
        let saw = r.vr.spawn("hacksaw", Transform::from_translation(near));
        r.vr.app
            .world_mut()
            .entity_mut(saw)
            .insert(RigidBody::Static);
        r.vr.step(2);
        let handle = r.vr.point(saw, |p| *p == GripRole::Primary);
        r.vr.hand(Side::Right, handle, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(3);
        r.vr.grip(Side::Right, 1.0);
        r.vr.step(3);
        r.vr.app
            .world_mut()
            .entity_mut(saw)
            .insert(RigidBody::Dynamic);
        assert_eq!(
            r.vr.hand_state(Side::Right).holding,
            Some(saw),
            "took the saw"
        );
        r.vr.step(5);
        // The saw across the barrel, 480 mm back from the muzzle.
        let gt = *r.vr.app.world().get::<GlobalTransform>(gun).unwrap();
        let muzzle = gt.transform_point(before);
        let back = gt.rotation() * Vec3::Z;
        let spot = muzzle + back * 0.48;
        let across = gt.rotation() * Vec3::X;
        let saw_rot = gt.rotation() * Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
        let mid = (BLADE_A + BLADE_B) * 0.5;
        let offset =
            r.vr.app
                .world()
                .get::<crate::interact::Held>(saw)
                .unwrap()
                .offset;
        let hand_at = |shift: f32| {
            let s = Transform::from_translation(spot - saw_rot * mid + across * shift)
                .with_rotation(saw_rot);
            let h = s.compute_affine() * offset.compute_affine().inverse();
            let (_, rot, at) = h.to_scale_rotation_translation();
            (at, rot)
        };
        let (at, rot) = hand_at(0.0);
        r.vr.hand(Side::Right, at, rot, 1.0, 0.0);
        r.vr.step(20);
        // Back and forth, ±6 cm, a stroke every 0.4 s, for four seconds.
        for i in 0..360 {
            let shift = 0.06 * (i as f32 / 36.0 * std::f32::consts::TAU).sin();
            let (at, rot) = hand_at(shift);
            r.vr.hand(Side::Right, at, rot, 1.0, 0.0);
            r.vr.step(1);
            if gun_mut(&mut r).firearm.build.barrel_mm.is_some() {
                break;
            }
        }
        r.vr.step(5);
        let mut g = gun_mut(&mut r);
        let cut = g.firearm.build.barrel_mm.expect("sawn through");
        println!(
            "obrez: barrel {cut:.0} mm, {:.0} m/s, muzzle z {:.2} (was {:.2})",
            g.firearm.def.muzzle_velocity, g.layout.muzzle.z, before.z
        );
        assert!((cut - 250.0).abs() < 40.0, "{cut}");
        assert!(g.firearm.def.muzzle_velocity < 0.85 * 865.0);
        assert!(g.layout.muzzle.z > before.z + 0.4, "the muzzle's come back");
        let _ = &mut g;
        assert!(
            count(&mut r, "scrap_metal") == scrap_before + 1,
            "the end's off"
        );
    }

    /// Clamped, a Mosin's stock screw takes the spanner: two turns and the
    /// stock's off (lighter, nothing to shoulder); let go behind the grip
    /// again, it's back on.
    #[test]
    fn the_stock_comes_off_with_the_spanner_and_goes_back_on() {
        use crate::interact::tools::spanner::{GRIP as S_GRIP, HEAD as S_HEAD, Spanner};
        let mut r = hold("mosin", false, false);
        let gun = r.gun;
        clamp(&mut r, gun, Side::Right);
        r.vr.step(3);
        let screw = {
            let mut q =
                r.vr.app
                    .world_mut()
                    .query_filtered::<Entity, With<StockScrew>>();
            q.iter(r.vr.app.world()).next().expect("the stock's screw")
        };
        let at =
            r.vr.app
                .world()
                .get::<GlobalTransform>(screw)
                .unwrap()
                .translation();
        let axis = Vec3::Y;
        let near = r.feet + Vec3::new(0.35, 1.0, 0.0);
        let tool = take_in(&mut r, Side::Right, "spanner", near);
        let grip = r.vr.hand_state(Side::Right).world.translation;
        let reach = S_HEAD - S_GRIP;
        let to = at - reach;
        glide_side(&mut r, Side::Right, grip, to, 20);
        r.vr.step(4);
        assert!(
            r.vr.app.world().get::<Spanner>(tool).unwrap().on.is_some(),
            "the spanner's on the screw"
        );
        let flat = (to - at) - axis * (to - at).dot(axis);
        let (rad, d0) = (flat.length(), flat.normalize());
        let side = axis.cross(d0);
        let turns = 2.3;
        let n = (90.0 * turns) as usize;
        for i in 1..=n {
            let a = i as f32 / n as f32 * turns * std::f32::consts::TAU;
            let p = at + (to - at).dot(axis) * axis + (d0 * a.cos() + side * a.sin()) * rad;
            r.vr.move_hand(Side::Right, p);
            r.vr.step(1);
        }
        r.vr.step(5);
        let catalog = r.vr.app.world().resource::<Catalog>().clone();
        let mass = catalog.get(catalog.kind("mosin").unwrap()).mass;
        {
            let g = gun_mut(&mut r);
            assert!(!g.firearm.build.stock, "the stock's off");
            assert!(g.mass(mass) < mass - 0.5, "lighter: {}", g.mass(mass));
            assert!(
                super::super::feel::butt(&g).is_none(),
                "nothing to shoulder"
            );
        }
        let stock = find(&mut r, "stock_mosin").expect("the stock lies by it");
        // Back on: the spanner down, the stock to behind the grip.
        r.vr.grip(Side::Right, 0.0);
        r.vr.step(3);
        r.vr.app.world_mut().entity_mut(tool).despawn();
        pick_up(&mut r, Side::Left, stock);
        let gt = *r.vr.app.world().get::<GlobalTransform>(gun).unwrap();
        let layout = gun_mut(&mut r).layout.clone();
        let goes = gt.transform_point(stock_point(&layout));
        let from = r.vr.hand_state(Side::Left).world.translation;
        let held =
            r.vr.app
                .world()
                .get::<GlobalTransform>(stock)
                .unwrap()
                .translation();
        glide_side(&mut r, Side::Left, from, from + (goes - held), 20);
        r.vr.grip(Side::Left, 0.0);
        r.vr.step(5);
        assert!(gun_mut(&mut r).firearm.build.stock, "back on");
        assert!(
            r.vr.app.world().get_entity(stock).is_err(),
            "the stock's on the gun"
        );
    }

    /// Two Mosins in two vices, one worn, one good: each one's bolt drawn
    /// out past its stop comes out into the hand as itself; the good bolt
    /// let go into the worn rifle's receiver goes in, and the worn rifle
    /// comes up by the bolt's share.
    #[test]
    fn a_good_bolt_into_a_worn_mosin() {
        let draw = |r: &mut Range, gun: Entity| -> Entity {
            let handle = r.vr.point(gun, |p| *p == GripRole::Handle(0));
            let (up, back) = {
                let gt = r.vr.app.world().get::<GlobalTransform>(gun).unwrap();
                (gt.up().as_vec3(), gt.back().as_vec3())
            };
            let travel =
                r.vr.app
                    .world()
                    .get::<Gun>(gun)
                    .unwrap()
                    .layout
                    .handle
                    .as_ref()
                    .unwrap()
                    .travel;
            r.vr.hand(Side::Left, handle, Quat::IDENTITY, 0.0, 0.0);
            r.vr.step(30);
            r.vr.trigger(Side::Left, 1.0);
            r.vr.step(2);
            assert!(r.vr.hand_state(Side::Left).handle.is_some(), "on the bolt");
            let lifted = handle + up * 0.045;
            glide_side(r, Side::Left, handle, lifted, 15);
            glide_side(r, Side::Left, lifted, lifted + back * (travel + 0.1), 30);
            r.vr.step(5);
            let held =
                r.vr.hand_state(Side::Left)
                    .holding
                    .expect("the bolt in the hand");
            r.vr.trigger(Side::Left, 0.0);
            held
        };
        let mut r = hold("mosin", false, false);
        let worn = r.gun;
        gun_mut(&mut r).firearm.condition = 0.3;
        clamp(&mut r, worn, Side::Right);
        // The good one, beside it.
        let at = r.feet + Vec3::new(-0.45, 1.2, -0.45);
        let good = r.vr.spawn("mosin", Transform::from_translation(at));
        r.vr.app
            .world_mut()
            .entity_mut(good)
            .insert(RigidBody::Static);
        r.vr.step(3);
        r.vr.app
            .world_mut()
            .get_mut::<Gun>(good)
            .unwrap()
            .firearm
            .condition = 0.9;
        let grip = r.vr.point(good, |p| *p == GripRole::Primary);
        let rake =
            r.vr.app
                .world()
                .get::<Gun>(good)
                .unwrap()
                .layout
                .grip
                .rotation;
        r.vr.hand(Side::Right, grip, rake, 1.0, 0.0);
        r.vr.step(3);
        r.vr.app
            .world_mut()
            .entity_mut(good)
            .insert(RigidBody::Dynamic);
        r.vr.step(10);
        clamp(&mut r, good, Side::Right);
        // The worn one's bolt out, and set down out of the way.
        let worn_bolt = draw(&mut r, worn);
        let catalog = r.vr.app.world().resource::<Catalog>().clone();
        let it = r.vr.app.world().get::<Item>(worn_bolt).unwrap();
        assert_eq!(catalog.get(it.kind()).id, "bolt_mosin");
        assert!(
            (it.condition - 0.3).abs() < 0.01,
            "a worn bolt: {}",
            it.condition
        );
        assert!(gun_mut(&mut r).firearm.build.bolt_out);
        let away = r.vr.hand_state(Side::Left).world.translation + Vec3::new(0.0, 0.0, 0.6);
        let from = r.vr.hand_state(Side::Left).world.translation;
        glide_side(&mut r, Side::Left, from, away, 15);
        let_go(&mut r, Side::Left);
        // The good one's out, and into the worn one.
        let good_bolt = draw(&mut r, good);
        let way = {
            let gt = *r.vr.app.world().get::<GlobalTransform>(worn).unwrap();
            let h =
                r.vr.app
                    .world()
                    .get::<Gun>(worn)
                    .unwrap()
                    .layout
                    .handle
                    .clone()
                    .unwrap();
            gt.transform_point(h.grab + Vec3::Z * h.travel)
        };
        let from = r.vr.hand_state(Side::Left).world.translation;
        let held =
            r.vr.app
                .world()
                .get::<GlobalTransform>(good_bolt)
                .unwrap()
                .translation();
        glide_side(&mut r, Side::Left, from, from + (way - held), 20);
        let_go(&mut r, Side::Left);
        let g = gun_mut(&mut r);
        let (want, _) = fit_part(0.3, 0.0, 0.9, BOLT_SHARE);
        println!(
            "worn rifle with the good bolt: {:.2} (want {want:.2})",
            g.firearm.condition
        );
        assert!(!g.firearm.build.bolt_out, "the good bolt's in");
        assert!((g.firearm.condition - want).abs() < 0.01);
        assert!(r.vr.app.world().get_entity(good_bolt).is_err());
    }
}

/// A gun's model as built: its barrel (and the forend with it) drawn
/// back along the bore to its cut length, its butt gone when the stock's
/// off, its bolt gone when it's out.
pub fn show_builds(
    guns: Query<(&Gun, &Children, Option<&Built>, Option<&Factory>)>,
    mut parts: Query<(&Name, &mut Transform, &mut Visibility), With<super::visuals::GunPart>>,
) {
    for (gun, children, built, factory) in &guns {
        let b = gun.firearm.build;
        let k = match (built, factory) {
            (Some(bt), Some(f)) => bt.barrel_mm / f.barrel_mm,
            _ => 1.0,
        };
        for c in children.iter() {
            let Ok((name, mut t, mut vis)) = parts.get_mut(c) else {
                continue;
            };
            let (show, scale) = match name.as_str() {
                // (The double's barrels hinge: they're drawn short too.)
                "static_barrel" | "barrels" => (true, k),
                "static_butt" => (b.stock, 1.0),
                "bayonet" => (b.barrel_mm.is_none(), 1.0),
                "turnbolt" => (!b.bolt_out, 1.0),
                _ => continue,
            };
            t.scale.z = scale;
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
