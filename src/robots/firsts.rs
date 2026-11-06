//! The heavy workstream's machines in the scripted first uses
//! (`render::warmup`, `EXTRACTION_FIRSTS`): each machine brought out in
//! front of the camera, then what it does that's first drawn mid-raid,
//! one a step, so `EXTRACTION_PIPELINES=1` shows which of them builds a
//! pipeline on the frame it first shows. The run's own steps come first
//! (a sentry placed and its chirp among them); these follow them
//! ([`step`]).
//!
//! A heavy (it arrives, slams, is wrecked and scorched), a spotter (it
//! goes up, calls), a minelayer (it arrives, a mine laid in its ring of
//! turned earth), a mender (it arrives, welds), and the valley's plough
//! where there is one (set ploughing: its plume, its lane and its bank).

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use super::{Job, JobKind, Robot, Senses};
use crate::world::Zone;

/// Where the run's camera is: its place, and ahead on the level.
#[derive(Clone, Copy)]
pub struct View {
    pub at: Vec3,
    pub flat: Vec3,
}

impl View {
    /// On the ground `d` m ahead, `side` m to the right.
    fn ground(&self, zone: &Zone, d: f32, side: f32) -> Vec3 {
        let p = self.at + self.flat * d + self.flat.cross(Vec3::Y) * side;
        zone.on_ground(p.x, p.z)
    }
}

/// The machines brought out so far.
#[derive(Resource, Default)]
struct Out {
    heavy: Option<Entity>,
    spotter: Option<Entity>,
    mender: Option<Entity>,
}

/// The `i`th of these first uses: sets it off and names it for the log,
/// or `None` past the last.
pub fn step(world: &mut World, i: u32, view: View) -> Option<String> {
    let zone = world.get_resource::<Zone>()?.clone();
    world.init_resource::<Out>();
    let out = |world: &World| {
        let o = world.resource::<Out>();
        (o.heavy, o.spotter, o.mender)
    };
    let (heavy, spotter, mender) = out(world);
    let name = match i {
        0 => {
            let at = view.ground(&zone, 22.0, -6.0);
            let catalog = world.resource::<crate::items::Catalog>().clone();
            let job = Job {
                kind: JobKind::Watch,
                site: at,
                duration: 999.0,
                exit: at,
            };
            let e = with_commands(world, |c| {
                let e = super::heavy::spawn_heavy(c, &zone, &catalog, at, job);
                c.entity(e).remove::<Senses>();
                e
            });
            world.resource_mut::<Out>().heavy = Some(e);
            "a heavy arrives"
        }
        1 => {
            let at = view.ground(&zone, 18.0, -4.0);
            world.write_message(super::wreck::WreckImpact { at, size: 1.2 });
            world.write_message(crate::destruction::hooks::Smash {
                at,
                radius: 1.5,
                damage: 400.0,
                push: Vec3::NEG_Y * 3.0,
            });
            if let Some(e) = heavy {
                world.write_message(super::heavy::HeavyEvent {
                    heavy: e,
                    at,
                    what: super::heavy::HeavyAct::Slam { hit: false },
                });
            }
            "a heavy slams"
        }
        2 => {
            if let Some(mut r) = heavy.and_then(|e| world.get_mut::<Robot>(e)) {
                r.wrecked = true;
            }
            "a heavy wrecked"
        }
        3 => {
            let at = view.ground(&zone, 40.0, 14.0);
            let e = with_commands(world, |c| {
                let e = super::spotter::spawn_spotter(c, &zone, Vec2::new(at.x, at.z));
                c.entity(e).remove::<Senses>();
                e
            });
            world.resource_mut::<Out>().spotter = Some(e);
            "a spotter goes up"
        }
        4 => {
            if let Some(e) = spotter {
                world.write_message(super::spotter::SpotterEvent {
                    spotter: e,
                    at: view.ground(&zone, 40.0, 14.0),
                    what: super::spotter::SpotterAct::Call,
                });
            }
            "a spotter calls"
        }
        5 => {
            let Some(nav) = world
                .get_resource::<super::walker::Nav>()
                .map(|n| n.0.clone())
            else {
                return Some("a minelayer arrives (no nav)".into());
            };
            let at = view.ground(&zone, 12.0, -3.0);
            let site = view.ground(&zone, 30.0, -3.0);
            with_commands(world, |c| {
                let e = super::minelayer::spawn_minelayer(c, &zone, &nav, at, vec![site], at);
                c.entity(e).remove::<Senses>();
            });
            "a minelayer arrives"
        }
        6 => {
            let at = view.ground(&zone, 8.0, 1.5);
            with_commands(world, |c| {
                super::minelayer::lay_mine(c, at, 0.0);
            });
            "a mine laid"
        }
        7 => {
            let at = view.ground(&zone, 10.0, 3.0);
            let e = with_commands(world, |c| {
                let e = super::mender::spawn_mender(c, at + Vec3::Y * 2.5, at);
                c.entity(e).remove::<Senses>();
                e
            });
            world.resource_mut::<Out>().mender = Some(e);
            "a mender arrives"
        }
        8 => {
            let at = view.ground(&zone, 10.0, 3.0) + Vec3::Y * 1.2;
            if let Some(mut m) = mender.and_then(|e| world.get_mut::<super::mender::Mender>(e)) {
                m.welding = Some(at);
            }
            "a mender welds"
        }
        9 => {
            // The valley's own plough (it's out from the start where snow
            // lies, its look warmed with the valley): set ploughing, its
            // first bank thrown up now, mid-raid.
            let mut q = world.query::<&mut super::plough::Plough>();
            let mut any = false;
            for mut p in q.iter_mut(world) {
                p.needs_snow = false;
                p.jammed = 0.0;
                any = true;
            }
            if any {
                "a plough ploughs"
            } else {
                "a plough ploughs (none in this valley)"
            }
        }
        _ => return None,
    };
    Some(name.into())
}

/// Commands on the world, applied at once.
fn with_commands<R>(world: &mut World, f: impl FnOnce(&mut Commands) -> R) -> R {
    let mut queue = CommandQueue::default();
    let r = {
        let mut c = Commands::new(&mut queue, world);
        f(&mut c)
    };
    queue.apply(world);
    r
}
