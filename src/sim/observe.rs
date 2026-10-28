//! What a playtest's raid recordings don't see: every press of a card,
//! cord or button, every card or board read (looked at from close for a
//! moment), every onboarding goal ticked and every setting changed. The
//! first headset night changed BODY and read the job card and camp board
//! for five minutes, and nothing showed it.
//!
//! Each goes to the session log as a `PLAYTEST` line and into the raid's
//! recording if one is open, whoever does it: a person, or a bot through
//! the same hands. It runs in every game, sims and scenarios included.

use bevy::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;

use super::telemetry::{SimLog, round};
use crate::interact::InteractEvent;
use crate::raid::truck::Pressable;
use crate::rig::{LocalPlayer, RigHead};

/// A card is read when the eyes rest on it this long, s...
pub const READ_SECS: f32 = 1.0;
/// ...from no further than this, m...
pub const READ_REACH: f32 = 1.6;
/// ...within this of the middle of the view, degrees.
pub const READ_CONE: f32 = 20.0;
/// Looked away this long, a card can be read again, s.
const READ_AGAIN: f32 = 5.0;

pub struct ObservePlugin;

impl Plugin for ObservePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (presses, reads, goals, settings_changed).after(crate::raid::tick_raid),
        );
    }
}

/// One observation: the session log, and the recording if one is open.
fn note(log: Option<&mut SimLog>, ev: &str, fields: Value) {
    info!("PLAYTEST {ev} {fields}");
    if let Some(log) = log {
        log.event(ev, fields);
    }
}

/// What a card says, its first line or so.
fn says(text: Option<&crate::raid::bench::ScreenText>) -> String {
    text.map(|t| {
        let line = t.text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        line.trim().chars().take(60).collect()
    })
    .unwrap_or_default()
}

#[allow(clippy::type_complexity)]
fn presses(
    mut events: MessageReader<InteractEvent>,
    things: Query<(
        &Pressable,
        Option<&Name>,
        Option<&crate::camp::CampCard>,
        Option<&crate::raid::bench::ScreenText>,
        &GlobalTransform,
    )>,
    locals: Query<(), With<LocalPlayer>>,
    mut log: Option<ResMut<SimLog>>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, player } = ev else {
            continue;
        };
        if !locals.contains(*player) {
            continue;
        }
        let Ok((p, name, card, text, gt)) = things.get(*item) else {
            continue;
        };
        let at = gt.translation();
        note(
            log.as_deref_mut(),
            "press",
            json!({"what": format!("{p:?}"), "name": name.map_or("", |n| n.as_str()),
                   "card": card.map(|c| format!("{c:?}")), "says": says(text),
                   "x": round(at.x, 1), "z": round(at.z, 1)}),
        );
    }
}

/// Whether a pressable is something to read (a card, a board, a page),
/// not a cord, a door or a lever.
fn readable(p: &Pressable) -> bool {
    matches!(
        p,
        Pressable::Camp
            | Pressable::Request
            | Pressable::Destination
            | Pressable::Setting
            | Pressable::Logbook
            | Pressable::Note
            | Pressable::Terminal
    )
}

#[allow(clippy::type_complexity)]
fn reads(
    cards: Query<(
        Entity,
        &Pressable,
        &GlobalTransform,
        Option<&Name>,
        Option<&crate::raid::bench::ScreenText>,
    )>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    time: Res<Time>,
    mut log: Option<ResMut<SimLog>>,
    // Per card: how long the eyes have rested on it, how long since they
    // left it, and whether this look has been noted.
    mut looks: Local<HashMap<Entity, (f32, f32, bool)>>,
) {
    let dt = time.delta_secs();
    let Some(eye) = heads
        .iter()
        .find(|(_, p)| locals.contains(p.parent()))
        .map(|(g, _)| *g)
    else {
        return;
    };
    let (from, forward) = (eye.translation(), eye.forward().as_vec3());
    let cos = READ_CONE.to_radians().cos();
    for (e, p, gt, name, text) in &cards {
        if !readable(p) {
            continue;
        }
        let to = gt.translation() - from;
        let on = to.length() < READ_REACH && to.normalize_or_zero().dot(forward) > cos;
        let look = looks.entry(e).or_insert((0.0, READ_AGAIN, false));
        if on {
            look.0 += dt;
            look.1 = 0.0;
            if look.0 >= READ_SECS && !look.2 {
                look.2 = true;
                let at = gt.translation();
                note(
                    log.as_deref_mut(),
                    "read",
                    json!({"what": format!("{p:?}"), "name": name.map_or("", |n| n.as_str()),
                           "says": says(text), "x": round(at.x, 1), "z": round(at.z, 1)}),
                );
            }
        } else {
            look.0 = 0.0;
            look.1 += dt;
            if look.1 >= READ_AGAIN {
                look.2 = false;
            }
        }
    }
    // Only cards still there, and looks still to be remembered.
    looks.retain(|e, l| cards.contains(*e) && (l.0 > 0.0 || l.1 < READ_AGAIN || l.2));
}

fn goals(mut done: MessageReader<crate::onboarding::GoalDone>, mut log: Option<ResMut<SimLog>>) {
    for g in done.read() {
        note(
            log.as_deref_mut(),
            "goal",
            json!({"goal": format!("{:?}", g.goal), "station_done": g.station_done}),
        );
    }
}

/// Every comfort setting changed, from what to what.
fn settings_changed(
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut was: Local<Option<serde_json::Map<String, Value>>>,
    mut log: Option<ResMut<SimLog>>,
) {
    let Some(comfort) = comfort else {
        return;
    };
    if !comfort.is_changed() && was.is_some() {
        return;
    }
    let Ok(Value::Object(now)) = serde_json::to_value(&*comfort) else {
        return;
    };
    if let Some(before) = was.as_ref() {
        for (k, v) in &now {
            let old = before.get(k).cloned().unwrap_or(Value::Null);
            if old != *v {
                note(
                    log.as_deref_mut(),
                    "setting",
                    json!({"name": k, "from": old, "to": v}),
                );
            }
        }
    }
    *was = Some(now);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raid::comfort_board::ComfortCard;

    /// The yard, as someone new plays it, is recorded like a raid, and
    /// what they do at the cards shows in it: a press, the setting it
    /// changed, a card read by looking at it, and the end when they go.
    #[test]
    fn the_yard_is_recorded_with_its_presses_reads_and_settings() {
        let dir = std::env::temp_dir().join(format!("extraction-observe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let opts = crate::LaunchOptions {
            robots: false,
            map: crate::camp::TRAINING_YARD.into(),
            map_explicit: true,
            data_dir: Some(dir.clone()),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        assert!(
            app.world().contains_resource::<SimLog>(),
            "recording the yard"
        );
        let w = app.world_mut();
        let player = w
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(w)
            .next()
            .unwrap();
        let (body_card, card_at) = w
            .query::<(Entity, &ComfortCard, &GlobalTransform)>()
            .iter(w)
            .find(|(_, c, _)| **c == ComfortCard::Body)
            .map(|(e, _, g)| (e, g.translation()))
            .expect("the BODY card in the truck");
        // Press it: BODY changes.
        app.world_mut().write_message(InteractEvent::Pressed {
            item: body_card,
            player,
        });
        app.update();
        app.update();
        // Look at it from a metre off for a second and a half, as a
        // headset does (the head's pose from the driver).
        app.insert_resource(crate::rig::RigDriver::Xr);
        for _ in 0..135 {
            let eye = card_at + Vec3::new(0.0, 0.0, 1.0);
            let root = eye - Vec3::Y * 1.65;
            *app.world_mut().get_mut::<Transform>(player).unwrap() =
                Transform::from_translation(root);
            app.world_mut()
                .get_mut::<crate::rig::RigInput>(player)
                .unwrap()
                .head = Transform::from_xyz(0.0, 1.65, 0.0).looking_at(card_at - root, Vec3::Y);
            app.update();
        }
        let path = app.world().resource::<SimLog>().path.clone();
        app.world_mut().write_message(AppExit::Success);
        app.update();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let evs: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let find = |ev: &str| evs.iter().find(|v| v["ev"] == ev);
        assert!(path.starts_with(&dir), "in the profile's own folder");
        let press = find("press").unwrap_or_else(|| panic!("a press: {text}"));
        assert_eq!(press["what"], "Setting");
        let setting = find("setting").unwrap_or_else(|| panic!("the setting: {text}"));
        assert_eq!(setting["name"], "body");
        let read = find("read").unwrap_or_else(|| panic!("a read: {text}"));
        assert_eq!(read["name"], "Comfort card");
        assert!(find("pos").is_some(), "positions: {text}");
        let end = find("end").unwrap_or_else(|| panic!("the end: {text}"));
        assert_eq!(end["outcome"], "left_yard");
    }
}
