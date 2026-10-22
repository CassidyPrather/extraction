//! What the new world gives you away by, one rule each.
//!
//! - **A gun knocking** ([`gun_clatter`]): a held gun driven into a
//!   doorframe or a tree (`weapons::carry::GunBumped`), or let go and
//!   slapped back against you by its sling, is a [`Noise`] of yours:
//!   [`KNOCK_REACH`] at a hard knock, [`SLING_REACH`] for a sling's slap.
//!   (A slung gun's swivels jingling on every step is already in your
//!   kit's rattle, `foley`.)
//! - **A load you carry shows** ([`LOAD_SHOWS`]): a car door or a bus
//!   panel in your hands is a big flat thing beside you. It adds to how
//!   much of you a robot makes out, and while it swings
//!   ([`LOAD_SWINGING`]) you aren't keeping still, however still you
//!   stand (`senses::perceive`).
//! - **A trail that ends at a plough's lane** ([`along_lane`]): a fresh
//!   lane takes no prints, so prints that stop at its edge mean you
//!   walked it. A robot reading the trail (`tracks`) guesses you went on
//!   along the lane the way you were heading, [`LANE_LEAD`] further, and
//!   goes to look there ([`LANE_UNCERTAINTY`]), not at the last print.
//! - **A dragged load** is already heard: `interact::carry` makes its
//!   scrape a [`Noise`] of yours (`carry::DRAG_HEARD`), twice a second,
//!   so it never wears out as a lure would (`mind_tests::
//!   a_dragged_door_draws_a_walker`).

use bevy::prelude::*;

use super::Noise;
use super::plough::Lanes;
use extraction_core::plough as rules;

/// A hard knock of a held gun is heard this far (m), at
/// [`KNOCK_FULL`] m/s or faster; a slower one less, down to half.
pub const KNOCK_REACH: f32 = 15.0;
pub const KNOCK_FULL: f32 = 2.0;
/// A let-go gun slapped back against you by its sling, heard this far.
pub const SLING_REACH: f32 = 6.0;
/// A carried bulky load: how much more of you a robot makes out.
pub const LOAD_SHOWS: f32 = 1.4;
/// A load moving this fast (m/s) is swinging: you aren't still.
pub const LOAD_SWINGING: f32 = 0.3;
/// Prints ending this close to a fresh lane's edge (m) went onto it...
pub const LANE_EDGE: f32 = 1.5;
/// ...and on along it this far (m)...
pub const LANE_LEAD: f32 = 15.0;
/// ...as a guess this uncertain (m).
pub const LANE_UNCERTAINTY: f32 = 4.0;

pub struct SignsPlugin;

impl Plugin for SignsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, gun_clatter.before(super::perceive));
    }
}

/// How far a gun knocked at `speed` m/s is heard.
pub fn knock_reach(speed: f32) -> f32 {
    KNOCK_REACH * (speed / KNOCK_FULL).clamp(0.5, 1.0)
}

/// A gun knocking, heard: see the module docs.
fn gun_clatter(
    mut bumped: MessageReader<crate::weapons::carry::GunBumped>,
    mut events: MessageReader<crate::interact::InteractEvent>,
    held: Query<&crate::interact::Held>,
    slung: Query<(&crate::weapons::carry::OnSling, &GlobalTransform)>,
    mut noise: MessageWriter<Noise>,
) {
    for b in bumped.read() {
        noise.write(Noise {
            at: b.at,
            radius: knock_reach(b.speed),
            source: held.get(b.gun).ok().map(|h| h.player),
        });
    }
    for ev in events.read() {
        let crate::interact::InteractEvent::Released { item, speed, .. } = *ev else {
            continue;
        };
        // Let go gently, it's caught by the sling (`weapons::carry`).
        if speed > crate::weapons::carry::THROWN {
            continue;
        }
        if let Ok((s, gt)) = slung.get(item) {
            noise.write(Noise {
                at: gt.translation(),
                radius: SLING_REACH,
                source: Some(s.player),
            });
        }
    }
}

fn seg_closest(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    a + ab * t
}

/// Where a trail ending at `end`, heading `heading` (xz), went on along a
/// fresh lane, if it ends at one: [`LANE_LEAD`] along the lane the way
/// it was heading, as far as the lane is still clean. `now` is the clock
/// the plough stamps its lanes by; `falling`, how hard it's snowing.
pub fn along_lane(lanes: &Lanes, now: f32, falling: f32, end: Vec3, heading: Vec2) -> Option<Vec3> {
    let q = end.xz();
    let reach = rules::BLADE * 0.5 + LANE_EDGE;
    let (on, dir) = lanes
        .0
        .iter()
        .filter(|(a, b, t)| rules::lane_fresh(now - t, falling) && a.distance(*b) > 1e-3)
        .map(|(a, b, _)| (seg_closest(q, *a, *b), (*b - *a).normalize()))
        .filter(|(c, _)| c.distance(q) < reach)
        .min_by(|x, y| x.0.distance(q).total_cmp(&y.0.distance(q)))?;
    // Which way along it: the way the trail was going (straight across,
    // either; it takes the plough's way).
    let way = if heading.dot(dir) < 0.0 { -dir } else { dir };
    // As far along as the lane still runs clean, a couple of metres at a
    // time (a road bends).
    let mut gone = 0.0;
    let mut at = on;
    while gone < LANE_LEAD {
        let next = at + way * 2.0;
        if !lanes.clean(Vec3::new(next.x, end.y, next.y), now, falling) {
            break;
        }
        at = next;
        gone += 2.0;
    }
    Some(Vec3::new(at.x, end.y, at.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trail ending at a lane's edge goes on along it the way it was
    /// heading, as far as the lane runs; one ending away from it, or at a
    /// lane gone stale, doesn't.
    #[test]
    fn a_trail_that_stops_at_a_lane_goes_on_along_it() {
        // A lane along +X from x=0 to x=40, cleared at t=100.
        let lanes = Lanes(
            (0..20)
                .map(|i| {
                    let x = i as f32 * 2.0;
                    (Vec2::new(x, 0.0), Vec2::new(x + 2.0, 0.0), 100.0)
                })
                .collect(),
        );
        // Prints coming up from the south (+Z) to its edge, heading -Z
        // and a little +X.
        let end = Vec3::new(20.0, 5.0, 2.5);
        let at = along_lane(&lanes, 160.0, 0.0, end, Vec2::new(0.3, -1.0)).expect("on the lane");
        assert!((at.x - (20.0 + LANE_LEAD)).abs() < 2.1, "on along +X: {at}");
        assert!(at.z.abs() < 0.1, "on the lane: {at}");
        // Heading a little -X instead: the other way.
        let back = along_lane(&lanes, 160.0, 0.0, end, Vec2::new(-0.3, -1.0)).unwrap();
        assert!(back.x < 20.0 - LANE_LEAD + 2.1, "on along -X: {back}");
        // Near its end: no further than the lane runs.
        let near_end = Vec3::new(36.0, 5.0, 2.0);
        let short = along_lane(&lanes, 160.0, 0.0, near_end, Vec2::X).unwrap();
        assert!(short.x <= 40.0 + 0.1, "stops where the lane does: {short}");
        // 6 m off it: nothing to do with it.
        assert!(along_lane(&lanes, 160.0, 0.0, Vec3::new(20.0, 5.0, 6.0), Vec2::X).is_none());
        // Long after, in a blizzard, it's filled in: prints would show.
        assert!(along_lane(&lanes, 100.0 + rules::LANE_LIFE, 1.0, end, Vec2::X).is_none());
    }

    #[test]
    fn a_harder_knock_carries_further() {
        assert!(knock_reach(0.8) < knock_reach(1.5));
        assert_eq!(knock_reach(5.0), KNOCK_REACH);
        assert!(knock_reach(0.8) >= KNOCK_REACH * 0.5);
    }
}
