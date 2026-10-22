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
        app.add_observer(setup_hacksaw)
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
pub const SAW_GRIP: Vec3 = Vec3::new(0.0, 0.0, 0.12);
pub const BLADE_A: Vec3 = Vec3::new(0.0, -0.05, -0.02);
pub const BLADE_B: Vec3 = Vec3::new(0.0, -0.05, -0.3);

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
        for (mut gun, gitem, ggt, mut vice, factory) in &mut guns {
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
            }
            if drawn < need {
                vice.sawing = Some((spot, drawn));
                continue;
            }
            vice.sawing = None;
            gun.firearm.build.barrel_mm = Some(spot);
            crate::interact::tools::pulse(&mut haptics, held.side, (0.55, 0.06));
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
) {
    for t in turned.read() {
        let Ok(parent) = screws.get(t.target) else {
            continue;
        };
        let Ok((mut gun, item, gt, mut vice)) = guns.get_mut(parent.parent()) else {
            continue;
        };
        vice.screw += t.angle.abs();
        if vice.screw < STOCK_SCREW || !gun.firearm.build.stock {
            continue;
        }
        vice.screw = 0.0;
        gun.firearm.build.stock = false;
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
    mut guns: Query<(&mut Gun, &Item, &GlobalTransform), With<InVice>>,
    catalog: Res<Catalog>,
) {
    use extraction_core::gunsmith::{BOLT_SHARE, fit_part};
    for e in released.read() {
        let Ok((part, pgt)) = parts.get(e) else {
            continue;
        };
        let pid = catalog.get(part.kind()).id.clone();
        let at = pgt.translation();
        for (mut gun, item, ggt) in &mut guns {
            let id = &catalog.get(item.kind()).id;
            let b = gun.firearm.build;
            if pid == part_item(id, "stock")
                && !b.stock
                && ggt.transform_point(stock_point(&gun.layout)).distance(at) < FIT_REACH
            {
                gun.firearm.build.stock = true;
                commands.entity(e).despawn();
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
