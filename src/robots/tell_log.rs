//! Every attack's tell, in the raid record: a machine that has you began
//! the warning before its blow (a walker's aim, a drone's lock, a hound
//! coiling, a crawler's spark gun charging, a heavy's arm rising, a
//! grappler winding up, a sentry's chirp). `"tell"` lines, so a gauge
//! measures warning to harm from the warning each attack gives
//! (`robots::MIN_LEAD`), not only from an approach seen coming: a crawler
//! lying in wait in the dark never "comes", and its crackle was missing
//! from the record (Marrow seed 3, job 2: read as 0–1 s, 2.5 s in fact).

use bevy::prelude::*;
use serde_json::json;

use crate::rig::LocalPlayer;
use crate::sim::telemetry::{SimLog, round};

use super::Awareness;
use super::drone::DroneLock;
use super::grappler::HarpoonWindup;
use super::heavy::{HeavyAct, HeavyEvent};
use super::hound::HoundCrouch;
use super::indoor::CrawlerCharge;
use super::sentry::{SentryAct, SentryEvent};
use super::walker::WalkerAim;

pub struct TellLogPlugin;

impl Plugin for TellLogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, log_tells);
    }
}

/// A sentry has no mind of whom it has: its chirp is on you within this (m).
const SENTRY_NEAR: f32 = 60.0;

#[allow(clippy::too_many_arguments)]
fn log_tells(
    log: Option<ResMut<SimLog>>,
    me: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    minds: Query<&Awareness>,
    mut aims: MessageReader<WalkerAim>,
    mut locks: MessageReader<DroneLock>,
    mut hounds: MessageReader<HoundCrouch>,
    mut crawlers: MessageReader<CrawlerCharge>,
    mut heavies: MessageReader<HeavyEvent>,
    mut harpoons: MessageReader<HarpoonWindup>,
    mut sentries: MessageReader<SentryEvent>,
) {
    let (Some(mut log), Ok((me, body))) = (log, me.single()) else {
        aims.clear();
        locks.clear();
        hounds.clear();
        crawlers.clear();
        heavies.clear();
        harpoons.clear();
        sentries.clear();
        return;
    };
    let here = body.translation();
    let on_me = |r: Entity| minds.get(r).is_ok_and(|a| a.target == Some(me));
    let mut tells: Vec<(&str, &str, Vec3)> = Vec::new();
    tells.extend(
        aims.read()
            .filter(|e| on_me(e.walker))
            .map(|e| ("Walker", "aim", e.at)),
    );
    tells.extend(
        locks
            .read()
            .filter(|e| on_me(e.drone))
            .map(|e| ("Drone", "lock", e.at)),
    );
    tells.extend(
        hounds
            .read()
            .filter(|e| on_me(e.hound))
            .map(|e| ("Hound", "coil", e.at)),
    );
    tells.extend(
        crawlers
            .read()
            .filter(|e| on_me(e.crawler))
            .map(|e| ("Crawler", "charge", e.at)),
    );
    tells.extend(
        heavies
            .read()
            .filter(|e| e.what == HeavyAct::Raise && on_me(e.heavy))
            .map(|e| ("Heavy", "raise", e.at)),
    );
    tells.extend(
        harpoons
            .read()
            .filter(|e| e.to == me)
            .map(|_| ("Grappler", "windup", here)),
    );
    tells.extend(
        sentries
            .read()
            .filter(|e| {
                e.what == SentryAct::Chirp
                    && (on_me(e.sentry)
                        || minds.get(e.sentry).is_err() && e.at.distance(here) < SENTRY_NEAR)
            })
            .map(|e| ("Sentry", "chirp", e.at)),
    );
    for (robot, what, at) in tells {
        log.event(
            "tell",
            json!({"robot": robot, "what": what,
                   "dist": round(at.distance(here), 1),
                   "x": round(at.x, 1), "z": round(at.z, 1)}),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (App, Entity, std::path::PathBuf) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<WalkerAim>()
            .add_message::<DroneLock>()
            .add_message::<HoundCrouch>()
            .add_message::<CrawlerCharge>()
            .add_message::<HeavyEvent>()
            .add_message::<HarpoonWindup>()
            .add_message::<SentryEvent>()
            .add_plugins(TellLogPlugin);
        let path = std::env::temp_dir().join(format!(
            "tell_log_{}_{:?}.jsonl",
            std::process::id(),
            std::thread::current().id()
        ));
        app.insert_resource(SimLog::open(path.clone()));
        let me = app
            .world_mut()
            .spawn((LocalPlayer, GlobalTransform::default()))
            .id();
        (app, me, path)
    }

    fn lines(app: &mut App, path: &std::path::Path) -> Vec<serde_json::Value> {
        app.world_mut().resource_mut::<SimLog>().flush();
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    /// A crawler that has you, charging its spark gun, is a tell in the
    /// record; one charging at someone else isn't.
    #[test]
    fn a_crawlers_charge_on_you_is_a_tell_in_the_record() {
        let (mut app, me, path) = app();
        let mine = app
            .world_mut()
            .spawn(Awareness {
                target: Some(me),
                ..Default::default()
            })
            .id();
        let other = app.world_mut().spawn(Awareness::default()).id();
        app.world_mut().write_message(CrawlerCharge {
            crawler: mine,
            at: Vec3::new(3.0, 0.0, 4.0),
        });
        app.world_mut().write_message(CrawlerCharge {
            crawler: other,
            at: Vec3::ZERO,
        });
        app.update();
        let tells: Vec<_> = lines(&mut app, &path)
            .into_iter()
            .filter(|l| l["ev"] == "tell")
            .collect();
        assert_eq!(tells.len(), 1, "{tells:?}");
        assert_eq!(tells[0]["robot"], "Crawler");
        assert_eq!(tells[0]["what"], "charge");
        assert_eq!(tells[0]["dist"], 5.0);
        let _ = std::fs::remove_file(path);
    }

    /// Each machine's tell on you is recorded (and a heavy's step isn't one).
    #[test]
    fn every_attacks_tell_on_you_is_recorded() {
        let (mut app, me, path) = app();
        let on = |app: &mut App| {
            app.world_mut()
                .spawn(Awareness {
                    target: Some(me),
                    ..Default::default()
                })
                .id()
        };
        let (w, d, h, hv) = (on(&mut app), on(&mut app), on(&mut app), on(&mut app));
        let s = app.world_mut().spawn_empty().id();
        let world = app.world_mut();
        world.write_message(WalkerAim {
            walker: w,
            at: Vec3::X,
        });
        world.write_message(DroneLock {
            drone: d,
            at: Vec3::X,
        });
        world.write_message(HoundCrouch {
            hound: h,
            at: Vec3::X,
        });
        world.write_message(HeavyEvent {
            heavy: hv,
            at: Vec3::X,
            what: HeavyAct::Step,
        });
        world.write_message(HeavyEvent {
            heavy: hv,
            at: Vec3::X,
            what: HeavyAct::Raise,
        });
        world.write_message(HarpoonWindup { from: s, to: me });
        world.write_message(SentryEvent {
            sentry: s,
            at: Vec3::new(10.0, 0.0, 0.0),
            what: SentryAct::Chirp,
        });
        app.update();
        let mut got: Vec<String> = lines(&mut app, &path)
            .into_iter()
            .filter(|l| l["ev"] == "tell")
            .map(|l| {
                format!(
                    "{} {}",
                    l["robot"].as_str().unwrap(),
                    l["what"].as_str().unwrap()
                )
            })
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                "Drone lock",
                "Grappler windup",
                "Heavy raise",
                "Hound coil",
                "Sentry chirp",
                "Walker aim"
            ]
        );
        let _ = std::fs::remove_file(path);
    }
}
