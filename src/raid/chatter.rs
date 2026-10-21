//! Robot chatter on the radio. When something reports you — a pylon, the
//! airship, a shot heard far off — and the director sends a response team,
//! the robots talk about it, and a radio you carry picks it up: a burst of
//! garbled static, and a sense of where they're headed. Close by, it's
//! your cue to move.

use bevy::prelude::*;

use super::{Phase, Raid};
use crate::anomaly::artifacts::CarriedBy;
use crate::items::{Catalog, Item};
use crate::rig::LocalPlayer;
use crate::robots::director::Director;

/// Within this of you, the chatter is about you.
pub const CLOSE: f32 = 120.0;

/// A radio on the local player picked up robot chatter.
#[derive(Message, Clone, Copy, Debug)]
pub struct RobotChatter {
    pub radio: Entity,
    /// Where the response is headed.
    pub at: Vec3,
    pub close: bool,
}

pub struct ChatterPlugin;

impl Plugin for ChatterPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RobotChatter>()
            .add_systems(Update, listen.after(super::tick_raid));
    }
}

fn listen(
    director: Option<ResMut<Director>>,
    mut raid: ResMut<Raid>,
    players: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    radios: Query<(Entity, &Item, &CarriedBy)>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<RobotChatter>,
    mut heard_count: Local<u32>,
    opts: Option<Res<crate::launch::LaunchOptions>>,
) {
    let Some(mut director) = director else {
        return;
    };
    if director.chatter.is_empty() {
        return;
    }
    let heard = std::mem::take(&mut director.chatter);
    // No machine talk in the training yard: nothing hunts you there, and
    // its range had the radio saying they were coming.
    if raid.phase == Phase::AtBase || opts.is_some_and(|o| o.map == crate::camp::TRAINING_YARD) {
        return;
    }
    for (player, gt) in &players {
        let Some(radio) = radios
            .iter()
            .find(|(_, item, by)| by.0 == player && catalog.get(item.kind()).id == "radio")
            .map(|(e, ..)| e)
        else {
            continue;
        };
        let me = gt.translation();
        // Several at once is one burst, about the nearest.
        let Some(at) = heard
            .iter()
            .copied()
            .min_by(|a, b| a.distance_squared(me).total_cmp(&b.distance_squared(me)))
        else {
            continue;
        };
        let d = Vec2::new(at.x - me.x, at.z - me.z);
        let close = d.length() < CLOSE;
        // A different way of hearing it each time, round and round.
        *heard_count += 1;
        let ways = if close {
            super::lines::CHATTER_CLOSE
        } else {
            super::lines::CHATTER_FAR
        };
        raid.say(super::lines::fill(
            super::lines::pick(ways, *heard_count),
            "",
            0.0,
            crate::anomaly::offering::bearing(d),
        ));
        out.write(RobotChatter { radio, at, close });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Heard(Vec<RobotChatter>);

    fn hear(mut m: MessageReader<RobotChatter>, mut h: ResMut<Heard>) {
        h.0.extend(m.read().copied());
    }

    #[test]
    fn a_carried_radio_hears_a_response_team_sent_your_way() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.init_resource::<Heard>().add_systems(Last, hear);
        app.finish();
        app.cleanup();
        app.update();
        let (player, me) = app
            .world_mut()
            .query_filtered::<(Entity, &GlobalTransform), With<LocalPlayer>>()
            .iter(app.world())
            .map(|(e, g)| (e, g.translation()))
            .next()
            .unwrap();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        // Without a radio: nothing.
        let radios: Vec<Entity> = {
            let catalog = app.world().resource::<Catalog>().clone();
            app.world_mut()
                .query::<(Entity, &Item, &CarriedBy)>()
                .iter(app.world())
                .filter(|(_, i, by)| by.0 == player && catalog.get(i.kind()).id == "radio")
                .map(|(e, ..)| e)
                .collect()
        };
        for r in &radios {
            app.world_mut().entity_mut(*r).remove::<CarriedBy>();
        }
        app.world_mut()
            .resource_mut::<Director>()
            .queue_response(30.0, me + Vec3::X * 20.0);
        // Run only the listener: the carried-by bookkeeping would put the
        // radio straight back on the player.
        app.world_mut().run_system_cached(listen).unwrap();
        assert!(app.world().resource::<Director>().chatter.is_empty());
        app.update();
        assert!(app.world().resource::<Heard>().0.is_empty());

        // With one.
        let catalog = app.world().resource::<Catalog>().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let radio = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind("radio").unwrap(),
                    transform: Transform::from_translation(me + Vec3::Y),
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.world_mut().entity_mut(radio).insert(CarriedBy(player));
        app.world_mut()
            .resource_mut::<Director>()
            .queue_response(30.0, me + Vec3::X * 20.0);
        app.world_mut().run_system_cached(listen).unwrap();
        app.update();
        let heard = &app.world().resource::<Heard>().0;
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0].radio, radio);
        assert!(heard[0].close);
        let raid = app.world().resource::<Raid>();
        assert!(raid.log.iter().any(|(l, _)| l.contains("coming here")));
    }
}
