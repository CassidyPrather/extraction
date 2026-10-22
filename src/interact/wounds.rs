//! Wounds with places: where a hit landed on the body (`rig::stance`), and
//! how much that wound bleeds. `health::Health::bleeding` stays the total
//! everything else reads; the wounds follow it ([`sync_wounds`]): a hit's
//! new bleeding goes to the wound it made, and bleeding stopped anywhere
//! else (the one-handed button's dressing, an injector, time) comes off
//! the worst wounds first.
//!
//! A dressing wound round a limb (`first_aid`) stops that limb's wounds,
//! not the rest; they show on the drawn body and the glove that touched
//! them (`render::wounds`).
//!
//! A hard landing ([`BREAK_LANDING`]) breaks a leg ([`Fracture`]): you
//! limp ([`LIMP`]) until a splint is strapped along the shin
//! (`first_aid`), and a little after ([`SPLINTED_LIMP`]).

use bevy::prelude::*;

use crate::health::Health;
use crate::interact::Hand;
use crate::player::BodyCollider;
use crate::rig::stance::Stance;
use crate::rig::{LocalPlayer, RigHand, RigInput, Side};
use crate::weapons::ballistics::Hit;

/// A part of the body a wound can be on, and a dressing wound round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Limb {
    Head,
    Chest,
    Belly,
    UpperArm(Side),
    Forearm(Side),
    Thigh(Side),
    Shin(Side),
}

/// A wound: on `limb`, `along` it (0 at its upper end, 1 at its lower),
/// `around` it (radians), bleeding this much (hp/s, its share of
/// `Health::bleeding`); dressed once a dressing's round it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wound {
    pub limb: Limb,
    pub along: f32,
    pub around: f32,
    pub bleed: f32,
    pub dressed: bool,
}

/// Landing this fast (m/s) breaks a leg (a safe one is under
/// `health::SAFE_LANDING`).
pub const BREAK_LANDING: f32 = 11.0;
/// How much slower a broken leg walks you, and splinted.
pub const LIMP: f32 = 0.55;
pub const SPLINTED_LIMP: f32 = 0.2;

/// A broken leg, splinted or not.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Fracture {
    pub leg: Side,
    pub splinted: bool,
}

/// The local player's wounds.
#[derive(Component, Debug, Default)]
pub struct Wounds(pub Vec<Wound>);

impl Wounds {
    /// Bleeding from the wounds, hp/s.
    pub fn bleeding(&self) -> f32 {
        self.0.iter().map(|w| w.bleed).sum()
    }

    /// Stops the wounds on `limb` (dressed): the bleeding stopped.
    pub fn dress(&mut self, limb: Limb) -> f32 {
        let mut stopped = 0.0;
        for w in self.0.iter_mut().filter(|w| w.limb == limb) {
            stopped += w.bleed;
            w.bleed = 0.0;
            w.dressed = true;
        }
        stopped
    }

    /// Brings the wounds' bleeding to `total`: more goes to the newest
    /// undressed wound (or a new one on the chest), less comes off the
    /// worst first.
    pub fn follow(&mut self, total: f32) {
        let mut delta = total - self.bleeding();
        if delta > 1e-4 {
            match self.0.iter_mut().rev().find(|w| !w.dressed) {
                Some(w) => w.bleed += delta,
                None => self.0.push(Wound {
                    limb: Limb::Chest,
                    along: 0.5,
                    around: 0.0,
                    bleed: delta,
                    dressed: false,
                }),
            }
        } else if delta < -1e-4 {
            let mut order: Vec<usize> = (0..self.0.len()).collect();
            order.sort_by(|a, b| self.0[*b].bleed.total_cmp(&self.0[*a].bleed));
            for i in order {
                let take = (-delta).min(self.0[i].bleed);
                self.0[i].bleed -= take;
                delta += take;
                if delta >= -1e-4 {
                    break;
                }
            }
        }
    }
}

/// A limb as a segment of the body now: its ends (upper, lower) and how
/// thick it is round.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub limb: Limb,
    pub a: Vec3,
    pub b: Vec3,
    pub r: f32,
}

impl Segment {
    /// Where `p` is on it: (along 0..1, angle round, distance from its
    /// axis), measured round its axis from a reference out to its right.
    pub fn locate(&self, p: Vec3) -> (f32, f32, f32) {
        let axis = (self.b - self.a).normalize_or(Vec3::NEG_Y);
        let len = self.a.distance(self.b).max(1e-3);
        let t = (p - self.a).dot(axis) / len;
        let radial = (p - self.a) - axis * (p - self.a).dot(axis);
        let x = axis.any_orthonormal_vector();
        let y = axis.cross(x);
        (t, radial.dot(y).atan2(radial.dot(x)), radial.length())
    }

    /// The point `along` it, `around` it, on its surface.
    pub fn point(&self, along: f32, around: f32) -> Vec3 {
        let axis = (self.b - self.a).normalize_or(Vec3::NEG_Y);
        let x = axis.any_orthonormal_vector();
        let y = axis.cross(x);
        self.a.lerp(self.b, along) + (x * around.cos() + y * around.sin()) * self.r
    }
}

/// The local player's limbs now, from the stance and the hands.
pub fn segments(st: &Stance, head: Vec3, hands: [Option<Vec3>; 2]) -> Vec<Segment> {
    let mut out = vec![
        Segment {
            limb: Limb::Head,
            a: head + Vec3::Y * 0.1,
            b: st.neck,
            r: 0.11,
        },
        Segment {
            limb: Limb::Chest,
            a: st.neck,
            b: st.chest.translation,
            r: 0.17,
        },
        Segment {
            limb: Limb::Belly,
            a: st.chest.translation,
            b: st.pelvis.translation,
            r: 0.16,
        },
    ];
    for side in [Side::Left, Side::Right] {
        let i = side.index();
        out.push(Segment {
            limb: Limb::Thigh(side),
            a: st.hips[i],
            b: st.knees[i],
            r: 0.085,
        });
        out.push(Segment {
            limb: Limb::Shin(side),
            a: st.knees[i],
            b: st.ankles[i],
            r: 0.06,
        });
        let shoulder = st.shoulder(side);
        let Some(wrist) = hands[i] else {
            continue;
        };
        let hint = st.chest.rotation * Vec3::new(0.6 * side.sign(), -1.0, 0.3).normalize();
        let axis = Vec2::new(st.chest.translation.x, st.chest.translation.z);
        let (s, elbow) = crate::render::arms::arm_joints(shoulder, wrist, hint, axis, st.arm);
        out.push(Segment {
            limb: Limb::UpperArm(side),
            a: s,
            b: elbow,
            r: 0.06,
        });
        out.push(Segment {
            limb: Limb::Forearm(side),
            a: elbow,
            b: wrist,
            r: 0.05,
        });
    }
    out
}

/// The limb nearest `p` (its axis), and where on it.
pub fn nearest(segments: &[Segment], p: Vec3) -> Option<(Segment, f32, f32)> {
    segments
        .iter()
        .map(|s| {
            let (t, a, d) = s.locate(p);
            let off = (t.clamp(0.0, 1.0) - t).abs() * s.a.distance(s.b);
            (*s, t.clamp(0.0, 1.0), a, d - s.r + off)
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(s, t, a, _)| (s, t, a))
}

/// The local player's limbs, now.
#[allow(clippy::type_complexity)]
pub fn player_segments(
    player: Entity,
    players: &Query<(&Transform, &RigInput, &Stance, &Children), With<LocalPlayer>>,
    hands: &Query<(&RigHand, &Hand)>,
) -> Option<Vec<Segment>> {
    let (root, input, st, children) = players.get(player).ok()?;
    if !st.ready {
        return None;
    }
    let mut wrists = [None, None];
    for c in children.iter() {
        if let Ok((rh, h)) = hands.get(c) {
            wrists[rh.side.index()] = Some(h.world.translation);
        }
    }
    let head = root.transform_point(input.head.translation);
    Some(segments(st, head, wrists))
}

pub struct WoundsPlugin;

impl Plugin for WoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, give_wounds)
            .add_systems(FixedUpdate, break_legs.before(crate::health::HealthSystems))
            .add_systems(
                FixedUpdate,
                (place_hits, sync_wounds, limp)
                    .chain()
                    .after(crate::health::HealthSystems),
            );
    }
}

fn give_wounds(
    mut commands: Commands,
    players: Query<Entity, (With<LocalPlayer>, With<Health>, Without<Wounds>)>,
) {
    for p in &players {
        commands.entity(p).insert(Wounds::default());
    }
}

/// A hit on the player's body: a wound where it landed (its bleeding
/// comes to it in [`sync_wounds`]).
#[allow(clippy::type_complexity)]
fn place_hits(
    mut hits: MessageReader<Hit>,
    colliders: Query<&ChildOf, With<BodyCollider>>,
    mut wounds: Query<&mut Wounds>,
    players: Query<(&Transform, &RigInput, &Stance, &Children), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand)>,
) {
    for hit in hits.read() {
        let player = colliders
            .get(hit.entity)
            .map(|c| c.parent())
            .unwrap_or(hit.entity);
        let Ok(mut w) = wounds.get_mut(player) else {
            continue;
        };
        let Some(segs) = player_segments(player, &players, &hands) else {
            continue;
        };
        let Some((seg, along, around)) = nearest(&segs, hit.point) else {
            continue;
        };
        w.0.push(Wound {
            limb: seg.limb,
            along,
            around,
            bleed: 0.0,
            dressed: false,
        });
    }
}

/// A landing hard enough breaks a leg (the one that took it: the left on
/// odd landings, for want of better), and the fall's bleeding is in that
/// shin. Read before `health::falls` takes the landing.
fn break_legs(
    mut commands: Commands,
    mut players: Query<(Entity, &crate::player::Body, &mut Wounds, Option<&Fracture>)>,
) {
    for (e, body, mut wounds, broken) in &mut players {
        if body.impact < BREAK_LANDING || broken.is_some() {
            continue;
        }
        let leg = if (body.impact * 10.0) as i32 % 2 == 0 {
            Side::Right
        } else {
            Side::Left
        };
        commands.entity(e).insert(Fracture {
            leg,
            splinted: false,
        });
        wounds.0.push(Wound {
            limb: Limb::Shin(leg),
            along: 0.5,
            around: 0.0,
            bleed: 0.0,
            dressed: false,
        });
    }
}

/// A broken leg slows you; splinted, less.
fn limp(mut players: Query<(&mut crate::player::Body, Option<&Fracture>), With<Wounds>>) {
    for (mut body, f) in &mut players {
        let want = match f {
            Some(f) if f.splinted => SPLINTED_LIMP,
            Some(_) => LIMP,
            None => 0.0,
        };
        if body.limping != want {
            body.limping = want;
        }
    }
}

/// The wounds follow `Health::bleeding`.
fn sync_wounds(mut players: Query<(&Health, &mut Wounds)>) {
    for (h, mut w) in &mut players {
        if (h.bleeding - w.bleeding()).abs() > 1e-4 {
            w.follow(h.bleeding);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(limb: Limb, bleed: f32) -> Wound {
        Wound {
            limb,
            along: 0.5,
            around: 0.0,
            bleed,
            dressed: false,
        }
    }

    /// New bleeding goes to the newest open wound; stopped bleeding comes
    /// off the worst first; dressing a limb stops only its wounds.
    #[test]
    fn wounds_follow_the_bleeding() {
        let mut w = Wounds(vec![
            cut(Limb::Thigh(Side::Left), 1.0),
            cut(Limb::Chest, 0.0),
        ]);
        w.follow(1.5);
        assert!((w.0[1].bleed - 0.5).abs() < 1e-4, "{w:?}");
        w.follow(1.2);
        assert!((w.0[0].bleed - 0.7).abs() < 1e-4, "the worst first: {w:?}");
        let stopped = w.dress(Limb::Thigh(Side::Left));
        assert!((stopped - 0.7).abs() < 1e-4);
        assert!(w.0[0].dressed && w.0[0].bleed == 0.0);
        assert!((w.bleeding() - 0.5).abs() < 1e-4, "the chest bleeds on");
        // Bleeding with nothing open: a wound on the chest.
        let mut none = Wounds::default();
        none.follow(0.3);
        assert_eq!(none.0[0].limb, Limb::Chest);
    }

    /// A point on a limb is found on it, where it is.
    #[test]
    fn a_point_is_found_on_its_limb() {
        let s = Segment {
            limb: Limb::Thigh(Side::Right),
            a: Vec3::new(0.1, 0.9, 0.0),
            b: Vec3::new(0.1, 0.5, 0.0),
            r: 0.085,
        };
        let p = s.point(0.25, 1.0);
        let (t, a, d) = s.locate(p);
        assert!((t - 0.25).abs() < 1e-3 && (a - 1.0).abs() < 1e-3 && (d - 0.085).abs() < 1e-3);
        let other = Segment {
            limb: Limb::Chest,
            a: Vec3::new(0.0, 1.45, 0.0),
            b: Vec3::new(0.0, 1.27, 0.0),
            r: 0.17,
        };
        let (found, ..) = nearest(&[other, s], p).unwrap();
        assert_eq!(found.limb, Limb::Thigh(Side::Right));
    }
}
