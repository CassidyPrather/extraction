//! Pocket spheres: a faint grey sphere on each of your pouches, pockets,
//! holster, mount and slings, in place of the drawn pouches (#93), so you
//! can see where your slots are, what's in them, where what you hold can
//! go and where it will.
//!
//! - Every slot shows, faint: hollow while empty; full, with what's in it
//!   drawn small and white inside (`render::pockets`), the thing itself
//!   hidden while it's there (`render::kit`). The worn slots (the vest,
//!   the helmet) show only for what goes on them: what's worn is drawn.
//! - Brought nearer with something they'd take, a sphere grows and
//!   brightens a little; the one it would go in if let go now
//!   ([`stow::StowQuery::target`], the release's own answer) is bright
//!   and breathes.
//! - Over the shoulder, a sphere for the pack, when a stow over the
//!   shoulder would go in it; at its mouth when it hangs open.
//! - One-handed, a pouch whose magazine the held gun would take from it
//!   (`seat_from_body`) shows as a place to put the gun.
//!
//! The spheres ride their slots: the slots follow the body (`body::
//! follow_head`), not the head, so they don't swim as you look about,
//! and a sphere that lagged its slot would lie about where the catch is.
//! `Comfort::pocket_spheres` (on by default) chooses them over the drawn
//! pouches; in flat mode (the hands never come to the body) the pouches
//! are drawn ([`PocketsShown`]). This is the state; `render::pockets`
//! draws it.

use bevy::prelude::*;
use extraction_core::inventory::SlotKind;

use super::body::{self, BodySlot, Torso};
use super::stow::{self, StowQuery, StowTarget};
use super::{Hand, Held, Socket, SocketKind, SocketOwner};
use crate::rig::{LocalPlayer, RigHand, Side};

/// A sphere starts growing this far from its catch (m).
pub const NEAR: f32 = 0.35;
/// An empty hand this near a full slot brightens it (m).
pub const REVEAL: f32 = 0.25;
/// How fast a sphere eases to what it shows (1/s): about 150 ms.
pub const EASE: f32 = 12.0;
/// From its slot to where the sphere sits, slot space: at the mouth.
pub const MOUTH: Vec3 = Vec3::new(0.0, 0.03, -0.05);
/// Over the shoulder (torso space, x for the right hand): where a stow
/// over the shoulder is let go.
pub const OVER_SHOULDER: Vec3 = Vec3::new(0.15, 0.22, 0.1);

/// What a sphere shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    /// Radius (m).
    pub radius: f32,
    /// 0 hidden .. 1.
    pub alpha: f32,
    /// Grey level, 0..1.
    pub grey: f32,
}

/// The state of each look, by what the sphere is to what's held.
pub mod looks {
    /// At rest (something held, far off): (alpha, grey).
    pub const REST: (f32, f32) = (0.12, 0.80);
    /// Near: up to this much bigger, alpha and grey added.
    pub const NEAR_GROW: f32 = 0.25;
    pub const NEAR_ALPHA: f32 = 0.12;
    pub const NEAR_GREY: f32 = 0.08;
    /// Where it goes if let go now: (size, alpha, grey).
    pub const HOVER: (f32, f32, f32) = (1.35, 0.42, 0.95);
    /// The breath of the hover: share of its size, and how fast (Hz).
    pub const BREATH: (f32, f32) = (0.04, 1.5);
    /// Full: (alpha, grey); near adds this much alpha.
    pub const FULL: (f32, f32) = (0.16, 0.50);
    pub const FULL_NEAR_ALPHA: f32 = 0.06;
}

/// A sphere's size at rest, from its slot's catch radius (m).
pub fn rest_radius(catch: f32) -> f32 {
    (catch * 0.42).clamp(0.032, 0.06)
}

/// A pocket sphere: on a body slot (`slot`), or the pack's (`None`).
#[derive(Component, Clone, Debug)]
pub struct PocketSphere {
    pub slot: Option<Entity>,
    /// What it shows now (eased).
    pub look: Look,
    /// What it's easing to.
    pub want: Look,
    /// Where it goes if let go now.
    pub hover: bool,
    /// What's in its slot (drawn small inside it).
    pub holds: Option<Entity>,
}

/// Whether the spheres are what your slots look like now (the setting
/// on, not in flat mode): `render::kit` hides the drawn pouches and what's
/// stowed in them while they are.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PocketsShown(pub bool);

/// Shown even in flat mode (the gallery's pictures of them).
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct PocketsInFlat(pub bool);

pub struct PocketsPlugin;

/// Where the spheres' state is set: what draws them comes after.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PocketsSystems;

impl Plugin for PocketsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PocketsInFlat>()
            .init_resource::<PocketsShown>()
            .init_resource::<PocketHints>()
            .add_systems(
                PostUpdate,
                (add_spheres, pocket_spheres)
                    .chain()
                    .in_set(PocketsSystems)
                    .before(bevy::transform::TransformSystems::Propagate),
            )
            // Before this frame's visibility is worked out.
            .add_systems(Update, hide_stowed);
    }
}

/// Whether the pocket spheres show at all, for onboarding to hold back
/// till it teaches pockets (playtest 2026-11-22: the yard's first far
/// grab met the spheres before anything had said what they were). Off,
/// it's as if the spheres were: no spheres, and what's in a slot drawn as
/// itself on the drawn pouches. Stowing and drawing work the same.
#[derive(Resource, Clone, Copy, Debug)]
pub struct PocketHints {
    pub shown: bool,
}

impl Default for PocketHints {
    fn default() -> Self {
        Self { shown: true }
    }
}

/// Something stowed in your own slot, hidden while the spheres stand in
/// (its sphere draws it small, `render::pockets`). Hidden, it's still in
/// reach: `find_grab` takes it out of its slot as if it were drawn.
#[derive(Component)]
pub struct HiddenInSlot;

/// What's in your own slots isn't drawn while the spheres stand in (each
/// sphere draws its own small); taken out, or the spheres off, it is.
/// (Worn armour is the vest's and the helmet's to show, `show_worn`.)
#[allow(clippy::type_complexity)]
pub(crate) fn hide_stowed(
    mut commands: Commands,
    spheres: Option<Res<PocketsShown>>,
    slots: Query<(&Socket, &crate::interact::SocketOwner), With<BodySlot>>,
    locals: Query<(), With<LocalPlayer>>,
    mut stowed: Query<(Entity, &crate::interact::InSocket, &mut Visibility), Without<HiddenInSlot>>,
    mut hidden: Query<
        (Entity, Option<&crate::interact::InSocket>, &mut Visibility),
        With<HiddenInSlot>,
    >,
) {
    let on = spheres.is_some_and(|s| s.0);
    let mine = |socket: Entity| {
        slots.get(socket).is_ok_and(|(s, o)| {
            locals.contains(o.0)
                && !matches!(s.kind, SocketKind::Body(SlotKind::Vest | SlotKind::Head))
        })
    };
    for (e, in_socket, mut v) in &mut hidden {
        if !on || !in_socket.is_some_and(|s| mine(s.socket)) {
            v.set_if_neq(Visibility::Inherited);
            commands.entity(e).remove::<HiddenInSlot>();
        }
    }
    if !on {
        return;
    }
    for (e, in_socket, mut v) in &mut stowed {
        if mine(in_socket.socket) {
            v.set_if_neq(Visibility::Hidden);
            commands.entity(e).insert(HiddenInSlot);
        }
    }
}

/// A sphere for each of the local player's body slots, and one for the
/// pack.
fn add_spheres(
    mut commands: Commands,
    slots: Query<(Entity, &Socket, &SocketOwner, &Transform), Added<BodySlot>>,
    locals: Query<(), With<LocalPlayer>>,
    torsos: Query<(Entity, &ChildOf), Added<Torso>>,
) {
    for (e, socket, owner, place) in &slots {
        if !locals.contains(owner.0) {
            continue;
        }
        commands.spawn((
            Name::new("Pocket sphere"),
            PocketSphere {
                slot: Some(e),
                look: Look::default(),
                want: Look::default(),
                hover: false,
                holds: None,
            },
            Transform::from_translation(sphere_at(&socket.kind, place.translation)),
            Visibility::Hidden,
            ChildOf(e),
        ));
    }
    for (torso, parent) in &torsos {
        if !locals.contains(parent.parent()) {
            continue;
        }
        commands.spawn((
            Name::new("Pack sphere"),
            PocketSphere {
                slot: None,
                look: Look::default(),
                want: Look::default(),
                hover: false,
                holds: None,
            },
            Transform::from_translation(OVER_SHOULDER),
            Visibility::Hidden,
            ChildOf(torso),
        ));
    }
}

/// Where a slot's sphere sits, in the slot's space: at its mouth; a
/// sling's at its shoulder, where a long gun's let go onto it.
pub fn sphere_at(kind: &SocketKind, place: Vec3) -> Vec3 {
    match kind {
        SocketKind::Body(SlotKind::Sling) => body::sling_shoulder(place) - place,
        SocketKind::Body(SlotKind::Vest | SlotKind::Head) => Vec3::ZERO,
        _ => MOUTH,
    }
}

/// How near (0 far .. 1 at its catch) a point `d` from a catch of radius
/// `r` is.
pub fn nearness(d: f32, r: f32) -> f32 {
    nearness_from(d, r, NEAR)
}

/// The same, starting `from` metres off.
pub fn nearness_from(d: f32, r: f32, from: f32) -> f32 {
    (1.0 - (d - r) / (from - r).max(1e-3)).clamp(0.0, 1.0)
}

/// What a slot's sphere wants to show: `takes` what's held (and how
/// `near`), is `full`, is where it goes (`hover`), `t` seconds in.
pub fn slot_look(catch: f32, takes: bool, full: bool, near: f32, hover: bool, t: f32) -> Look {
    let r = rest_radius(catch);
    if hover {
        let (size, alpha, grey) = looks::HOVER;
        let (share, hz) = looks::BREATH;
        let breath = 1.0 + share * (t * std::f32::consts::TAU * hz).sin();
        return Look {
            radius: r * size * breath,
            alpha,
            grey,
        };
    }
    if full {
        let (alpha, grey) = looks::FULL;
        return Look {
            radius: r,
            alpha: alpha + looks::FULL_NEAR_ALPHA * near,
            grey,
        };
    }
    if takes {
        let (alpha, grey) = looks::REST;
        return Look {
            radius: r * (1.0 + looks::NEAR_GROW * near),
            alpha: alpha + looks::NEAR_ALPHA * near,
            grey: grey + looks::NEAR_GREY * near,
        };
    }
    // Empty, and not for what's held (or nothing held): at rest.
    let (alpha, grey) = looks::REST;
    Look {
        radius: r,
        alpha,
        grey,
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn pocket_spheres(
    stow: StowQuery,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    driver: Res<crate::rig::RigDriver>,
    (in_flat, mut shown, hints): (
        Res<PocketsInFlat>,
        ResMut<PocketsShown>,
        Option<Res<PocketHints>>,
    ),
    time: Res<Time>,
    locals: Query<(Entity, &Children), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand)>,
    held: Query<(Entity, &Held, &GlobalTransform)>,
    slots: Query<(&Socket, &GlobalTransform, &Transform), With<BodySlot>>,
    mags: Query<(&crate::weapons::Mag, &GlobalTransform)>,
    wells: Query<(&Socket, &GlobalTransform, &SocketOwner), Without<BodySlot>>,
    packs: Query<(Entity, &body::Backpack, &GlobalTransform)>,
    torsos: Query<&GlobalTransform, With<Torso>>,
    mut spheres: Query<
        (&mut PocketSphere, &mut Transform, &mut Visibility, &ChildOf),
        Without<BodySlot>,
    >,
) {
    let on = comfort.as_ref().is_none_or(|c| c.pocket_spheres)
        && (*driver != crate::rig::RigDriver::Flat || in_flat.0)
        && hints.is_none_or(|h| h.shown);
    if shown.0 != on {
        shown.0 = on;
    }
    let one_handed = comfort.as_ref().is_some_and(|c| c.one_handed);
    let Some((me, children)) = locals.iter().next() else {
        return;
    };
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    // What the local player holds: the thing, where, and where it goes.
    let holding: Vec<(Entity, Vec3, Side, Option<StowTarget>)> = held
        .iter()
        .filter(|(_, h, _)| h.player == me)
        .map(|(e, h, g)| (e, g.translation(), h.side, stow.target(e)))
        .collect();
    let empty_hands: Vec<Vec3> = children
        .iter()
        .filter_map(|c| hands.get(c).ok())
        .filter(|(_, h)| h.holding.is_none())
        .map(|(_, h)| h.world.translation)
        .collect();
    // Held guns' empty wells (family, where), for a one-handed reload.
    let wells: Vec<(String, Vec3)> = wells
        .iter()
        .filter(|(s, _, o)| s.occupant.is_none() && holding.iter().any(|h| h.0 == o.0))
        .filter_map(|(s, g, _)| match &s.kind {
            SocketKind::MagWell(f) => Some((f.clone(), g.translation())),
            _ => None,
        })
        .collect();
    for (mut sphere, mut tf, mut vis, parent) in &mut spheres {
        let mut want = Look::default();
        let mut hover = false;
        let mut holds = None;
        if on {
            match sphere.slot {
                Some(slot) => {
                    if let Ok((socket, g, place)) = slots.get(slot) {
                        let view = stow::SlotView::new(slot, g, socket, None, Some(place));
                        let full = socket.occupant.is_some();
                        let worn = matches!(
                            socket.kind,
                            SocketKind::Body(SlotKind::Vest | SlotKind::Head)
                        );
                        let mut takes = false;
                        let mut near: f32 = 0.0;
                        for (item, at, _, target) in &holding {
                            let def = stow.def(*item);
                            if view.takes(*item, def) {
                                takes = true;
                                let (d, r) = view.reach(*at);
                                near = near.max(nearness(d, r));
                            } else if full && !worn {
                                near = near.max(nearness(view.at.distance(*at), view.radius));
                            }
                            hover |= *target == Some(StowTarget::Socket(slot));
                        }
                        // A magazine here a held gun's well would take,
                        // one-handed: a place for the gun.
                        if one_handed
                            && let Some((mag, mg)) = socket.occupant.and_then(|o| mags.get(o).ok())
                        {
                            let top = mg.transform_point(Vec3::Y * mag.height * 0.5);
                            for (family, at) in &wells {
                                if *family == mag.family {
                                    let d = top.distance(*at);
                                    takes = true;
                                    near = near.max(nearness(d, super::BODY_SEAT));
                                    hover |= d < super::BODY_SEAT * 2.0;
                                }
                            }
                        }
                        // An empty hand at a full one: as near as a
                        // thing it would take.
                        if full {
                            for h in &empty_hands {
                                near = near.max(nearness_from(
                                    view.at.distance(*h),
                                    view.radius,
                                    REVEAL,
                                ));
                            }
                        }
                        if !worn || takes {
                            want = slot_look(socket.radius, takes && !full, full, near, hover, t);
                            holds = socket.occupant;
                        }
                        tf.translation = sphere_at(&socket.kind, place.translation);
                    }
                }
                None => {
                    // The pack: where a stow over the shoulder (or a let
                    // go into it hanging open) puts it.
                    for (item, at, side, target) in &holding {
                        let Some(def) = stow.def(*item) else {
                            continue;
                        };
                        let shoulder = stow::shoulder_stow(
                            def,
                            me,
                            stow.slots().collect::<Vec<_>>(),
                            stow.packs(Some(def)),
                        );
                        let bag = match shoulder {
                            StowTarget::Pack(b) => Some(b),
                            _ => None,
                        };
                        let open = packs
                            .iter()
                            .find(|(_, b, _)| b.owner == me && b.open)
                            .map(|(e, b, g)| (e, b, *g));
                        let into = match target {
                            Some(StowTarget::Pack(b)) => Some(*b),
                            _ => None,
                        };
                        if bag.is_none() && open.is_none() && into.is_none() {
                            continue;
                        }
                        let here = into.is_some();
                        let near = match (open, torsos.get(parent.parent())) {
                            (Some((_, b, g)), Ok(torso)) => {
                                let mouth = g.transform_point(Vec3::Y * b.half.y);
                                tf.translation = torso.affine().inverse().transform_point3(mouth);
                                nearness(at.distance(mouth), b.half.x)
                            }
                            (None, Ok(torso)) => {
                                let x = if *side == Side::Left { -1.0 } else { 1.0 };
                                tf.translation = OVER_SHOULDER * Vec3::new(x, 1.0, 1.0);
                                let spot = torso.transform_point(tf.translation);
                                nearness(at.distance(spot), 0.15)
                            }
                            _ => 0.0,
                        };
                        hover = here;
                        want = slot_look(0.12, true, false, near, hover, t);
                    }
                }
            }
        }
        sphere.want = want;
        sphere.hover = hover;
        sphere.holds = holds;
        // Eased (a hover comes and goes at once: it's the truth now).
        let k = (EASE * dt).min(1.0);
        let mut look = sphere.look;
        look.alpha += (want.alpha - look.alpha) * k;
        look.grey += (want.grey - look.grey) * k;
        look.radius = if look.alpha < 0.01 {
            want.radius
        } else {
            look.radius + (want.radius - look.radius) * k
        };
        if hover {
            look = want;
        }
        sphere.look = look;
        tf.scale = Vec3::splat(look.radius.max(1e-4));
        let shown = look.alpha > 0.01;
        let v = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != v {
            *vis = v;
        }
    }
}
