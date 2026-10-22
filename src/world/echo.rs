//! A room that carries sound, for any site whose layout has one
//! (`gallery_sallow`'s 44 m gallery first): a noise made in it, or a step
//! taken in it standing and at a walk, is heard its whole length. The
//! tell is your own steps coming back to you off its far end (told, and
//! `Echoed` for the ear); crouch, or go slowly, and they don't carry.
//!
//! Headless-safe: it only listens to the noises and steps and makes the
//! carried ones (`robots::Noise`).

use bevy::prelude::*;
use extraction_core::underground as ug;

use super::buildings::Building;

/// Slower than this (m/s), a step doesn't carry.
pub const QUIET_PACE: f32 = 1.0;
/// Lower than this (the body, m), a step doesn't carry: crouched.
pub const QUIET_HEIGHT: f32 = 1.3;

pub struct EchoPlugin;

impl Plugin for EchoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Carrying>()
            .add_message::<Echoed>()
            .init_resource::<crate::bot::Counters>()
            .add_systems(
                Update,
                (hear_the_gallery, carry, stage_scenario, count_echoes).chain(),
            );
    }
}

/// A sound carried the length of a room (for the ear: the echo).
#[derive(Message, Clone, Copy, Debug)]
pub struct Echoed {
    pub at: Vec3,
    pub radius: f32,
    /// The room's end farthest from it, on its floor: where the echo
    /// comes back from.
    pub from: Vec3,
    /// Whose step it was, if a step.
    pub player: Option<Entity>,
}

/// What's to be carried, heard this frame and made next.
#[derive(Resource, Default)]
struct Carrying(Vec<crate::robots::Noise>);

/// How far a sound made at `p` carries, if it's made in a carrying room;
/// and the room's end farthest from it (on its floor).
pub fn carries(sites: &Query<(Entity, &Building, &Transform)>, p: Vec3) -> Option<(f32, Vec3)> {
    for (_, b, xf) in sites {
        let Some(l) = ug::layout(&b.prefab) else {
            continue;
        };
        if !l.rooms.iter().any(|r| r.carries) {
            continue;
        }
        let q = xf.compute_affine().inverse().transform_point3(p);
        let found = l.rooms.iter().find(|r| {
            r.carries
                && (q.x - r.at[0]).abs() <= r.size[0] * 0.5
                && (q.z - r.at[1]).abs() <= r.size[1] * 0.5
                && q.y > l.floor_of(r) - 0.5
                && q.y < l.floor_of(r) + r.height
        });
        if let Some(r) = found {
            let along = if r.size[0] >= r.size[1] {
                Vec3::X * r.size[0]
            } else {
                Vec3::Z * r.size[1]
            } * 0.5;
            let mid = Vec3::new(r.at[0], l.floor_of(r), r.at[1]);
            let far = if (q - mid).dot(along) > 0.0 {
                mid - along
            } else {
                mid + along
            };
            return r.carry().map(|c| (c, xf.transform_point(far)));
        }
    }
    None
}

/// The noises and steps made in a carrying room, to be carried; the first
/// step of yours that carries, told.
#[allow(clippy::too_many_arguments)]
fn hear_the_gallery(
    mut noises: MessageReader<crate::robots::Noise>,
    mut steps: MessageReader<crate::player::Footstep>,
    sites: Query<(Entity, &Building, &Transform)>,
    bodies: Query<&crate::player::Body>,
    mut out: ResMut<Carrying>,
    mut echoed: MessageWriter<Echoed>,
    mut raid: ResMut<crate::raid::Raid>,
    mut told: Local<bool>,
) {
    for n in noises.read() {
        let Some((radius, from)) = carries(&sites, n.at) else {
            continue;
        };
        // (What's carried already, or louder, isn't carried again.)
        if n.radius >= radius * 0.9 {
            continue;
        }
        out.0.push(crate::robots::Noise {
            at: n.at,
            radius,
            source: n.source,
        });
        echoed.write(Echoed {
            at: n.at,
            radius,
            from,
            player: None,
        });
    }
    for s in steps.read() {
        let Some((radius, from)) = carries(&sites, s.position) else {
            continue;
        };
        let tall = bodies
            .get(s.entity)
            .is_ok_and(|b| b.height <= 0.0 || b.height >= QUIET_HEIGHT);
        if s.speed < QUIET_PACE || !tall {
            continue;
        }
        out.0.push(crate::robots::Noise {
            at: s.position + Vec3::Y * 0.1,
            radius,
            source: Some(s.entity),
        });
        echoed.write(Echoed {
            at: s.position,
            radius,
            from,
            player: Some(s.entity),
        });
        if !*told {
            *told = true;
            raid.say(
                "Your steps come back to you off the far end: the gallery carries every sound.",
            );
        }
    }
}

/// The scenario: in the gallery's east end, the bot walks its length,
/// upright, and its steps carry (`gallery_echoes`).
pub const SCENARIO: &str = "gallery_echo";
/// Filled in by the setup: the gallery's far (west) end.
pub const FAR_END: Vec2 = Vec2::new(-9002.0, 1.0);

/// The scenario staged: the bot into the gallery's east end, its walk to
/// the far end filled in.
fn stage_scenario(
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    sites: Query<(&Building, &Transform), Without<crate::rig::LocalPlayer>>,
    mut players: Query<(&mut Transform, &mut crate::bot::Bot), With<crate::rig::LocalPlayer>>,
) {
    for s in setups.read() {
        if s.name != SCENARIO {
            continue;
        }
        let Some((l, xf)) = sites.iter().find_map(|(b, xf)| {
            ug::layout(&b.prefab)
                .filter(|l| l.rooms.iter().any(|r| r.carries))
                .map(|l| (l, *xf))
        }) else {
            error!("gallery scenario: no carrying room on this map");
            continue;
        };
        let Some(g) = l.rooms.iter().find(|r| r.carries) else {
            continue;
        };
        let along = if g.size[0] >= g.size[1] {
            Vec2::X
        } else {
            Vec2::Y
        } * (g.size[0].max(g.size[1]) * 0.5 - 1.5);
        let end = |k: f32| {
            let p = Vec2::from(g.at) + along * k;
            xf.transform_point(Vec3::new(p.x, l.floor_of(g) + 0.05, p.y))
        };
        let (from, to) = (end(-1.0), end(1.0));
        for (mut t, mut bot) in &mut players {
            t.translation = from;
            for task in bot.tasks.iter_mut() {
                if let crate::bot::Task::WalkTo { to: w, .. }
                | crate::bot::Task::Expect(crate::bot::Expect::Near(w, _)) = task
                    && *w == FAR_END
                {
                    *w = Vec2::new(to.x, to.z);
                }
            }
        }
        info!("scenario: the gallery from {from:?} to {to:?}");
    }
}

/// Counts the steps of yours the gallery carried.
fn count_echoes(mut echoed: MessageReader<Echoed>, mut counters: ResMut<crate::bot::Counters>) {
    for e in echoed.read() {
        if e.player.is_some() {
            counters.bump("gallery_echoes");
        }
    }
}

fn carry(mut out: ResMut<Carrying>, mut noise: MessageWriter<crate::robots::Noise>) {
    for n in out.0.drain(..) {
        noise.write(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robots::{Awareness, Noise};

    const SITE: &str = "gallery_sallow";

    #[derive(Resource, Default)]
    struct Heard(Vec<Noise>);

    /// A tin knocked over at the gallery's east end is carried its length:
    /// the crawler 36 m off at its west end hears it, and has an idea
    /// where.
    #[test]
    fn the_gallery_carries_sound_its_length() {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: true,
                map: "sallow_reservoir".into(),
                ..Default::default()
            },
            false,
        );
        app.init_resource::<Heard>().add_systems(
            Update,
            |mut n: MessageReader<Noise>, mut h: ResMut<Heard>| h.0.extend(n.read().cloned()),
        );
        app.finish();
        app.cleanup();
        for _ in 0..10 {
            app.update();
        }
        let xf = *app
            .world_mut()
            .query::<(&Building, &Transform)>()
            .iter(app.world())
            .find(|(b, _)| b.prefab == SITE)
            .map(|(_, t)| t)
            .expect("the gallery placed");
        let l = ug::layout(SITE).unwrap();
        let room = |id: &str| l.rooms.iter().find(|r| r.id == id).unwrap();
        let g = room("gallery");
        let at = |x: f32, id: &str| {
            let r = room(id);
            xf.transform_point(Vec3::new(r.at[0] + x, l.floor_of(r) + 0.1, r.at[1]))
        };
        let east = at(-20.0, "gallery");
        let west = at(16.0, "gallery");
        assert_eq!(g.carry(), Some(44.0 + ug::CARRY_OVER));

        // The gallery's crawler, awake, at the west end.
        let crawler = app
            .world_mut()
            .query_filtered::<(Entity, &crate::robots::indoor::Resident), With<Awareness>>()
            .iter(app.world())
            .min_by(|a, b| a.1.bay.distance(west).total_cmp(&b.1.bay.distance(west)))
            .map(|(e, _)| e)
            .expect("the gallery's crawler");
        app.world_mut()
            .entity_mut(crawler)
            .remove::<crate::robots::indoor::Dormant>();
        let place = |app: &mut App| {
            app.world_mut()
                .get_mut::<Transform>(crawler)
                .unwrap()
                .translation = west;
            let mut a = app.world_mut().get_mut::<Awareness>(crawler).unwrap();
            a.suspicion = 0.0;
            a.last_known = None;
        };
        place(&mut app);
        app.update();
        app.world_mut().resource_mut::<Heard>().0.clear();

        // A tin knocked over at the east end.
        app.world_mut().write_message(Noise {
            at: east,
            radius: 8.0,
            source: None,
        });
        for _ in 0..4 {
            app.update();
        }
        let heard = &app.world().resource::<Heard>().0;
        assert!(
            heard
                .iter()
                .any(|n| n.at.distance(east) < 0.5 && n.radius >= 44.0),
            "carried: {heard:?}"
        );
        let a = app.world().get::<Awareness>(crawler).unwrap();
        assert!(a.suspicion > 0.0, "the crawler heard it: {a:?}");
        let guess = a.last_known.expect("and has an idea where");
        assert!(
            guess.distance(east) < guess.distance(west),
            "toward the east end: {guess:?}"
        );
    }

    /// A step in the gallery at a walk carries; slow, or crouched, it
    /// doesn't; nor out on the landing.
    #[test]
    fn a_walk_carries_and_a_creep_does_not() {
        let mut vr = crate::vr_harness::Vr::new("sallow_reservoir");
        vr.app.init_resource::<Heard>().add_systems(
            Update,
            |mut n: MessageReader<Noise>, mut h: ResMut<Heard>| h.0.extend(n.read().cloned()),
        );
        let xf = *vr
            .app
            .world_mut()
            .query::<(&Building, &Transform)>()
            .iter(vr.app.world())
            .find(|(b, _)| b.prefab == SITE)
            .map(|(_, t)| t)
            .expect("the gallery placed");
        let l = ug::layout(SITE).unwrap();
        let at = |x: f32, id: &str| {
            let r = l.rooms.iter().find(|r| r.id == id).unwrap();
            xf.transform_point(Vec3::new(r.at[0] + x, l.floor_of(r) + 0.05, r.at[1]))
        };
        let player = vr.player;
        let step = |vr: &mut crate::vr_harness::Vr, at: Vec3, speed: f32, head: f32| -> bool {
            vr.stand(at, 0.0);
            vr.head(Vec3::new(0.0, head, 0.0));
            vr.step(10);
            vr.app.world_mut().resource_mut::<Heard>().0.clear();
            vr.app.world_mut().write_message(crate::player::Footstep {
                entity: player,
                position: at,
                speed,
            });
            vr.step(2);
            vr.app
                .world()
                .resource::<Heard>()
                .0
                .iter()
                .any(|n| n.source == Some(player) && n.radius >= 44.0)
        };
        let east = at(-20.0, "gallery");
        assert!(step(&mut vr, east, 1.5, 1.65), "a walk carries");
        assert!(!step(&mut vr, east, 0.6, 1.65), "a creep doesn't");
        assert!(!step(&mut vr, east, 1.5, 0.9), "nor a crouch");
        assert!(
            !step(&mut vr, at(0.0, "toe_landing"), 1.5, 1.65),
            "nor the landing"
        );
    }
}
