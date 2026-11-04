//! A tape left playing is a lure: a walker at its post, out of sight of
//! you, hears it and walks over to look (Robot minds' hearing, through
//! `robots::Noise`, the same channel a thrown bolt uses).

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use extraction_core::music::deck::{Mode, Side};

use super::deck::{TapeDeck, WithCell};
use crate::rig::{LocalPlayer, RigInput};
use crate::robots::{Brain, Job, JobKind};

fn flat(v: Vec3) -> f32 {
    Vec2::new(v.x, v.z).length()
}

#[test]
fn a_tape_playing_draws_a_walker() {
    let mut app = crate::headless_app(
        crate::LaunchOptions {
            map: extraction_core::map::TEST_FIELD_ID.into(),
            robots: false,
            ..Default::default()
        },
        false,
    );
    app.finish();
    app.cleanup();
    for _ in 0..3 {
        app.update();
    }
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(app.world())
        .next()
        .unwrap();
    app.world_mut()
        .get_mut::<RigInput>(player)
        .unwrap()
        .head
        .translation
        .y = 1.65;
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let post = zone.on_ground(100.0, 200.0);
    // You, well out of its sight.
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation = zone.on_ground(100.0, 330.0) + Vec3::Y * 0.05;
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let mut queue = CommandQueue::default();
    let (walker, deck, spot) = {
        let mut c = Commands::new(&mut queue, app.world());
        let walker = crate::robots::walker::spawn_walker_with(
            &mut c,
            &zone,
            post,
            Job {
                kind: JobKind::Watch,
                site: post,
                duration: 999.0,
                exit: post,
            },
            false,
            130.0,
        );
        // The player on the ground 20 m off, a tape loaded, playing.
        let spot = zone.on_ground(post.x + 20.0, post.z + 2.0);
        let spawn = |c: &mut Commands, id: &str, at: Vec3| {
            crate::items::spawn_item(
                c,
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind(id).unwrap(),
                    transform: Transform::from_translation(at),
                    condition: 1.0,
                },
            )
        };
        let deck = spawn(&mut c, "cassette_player", spot + Vec3::Y * 0.1);
        c.entity(deck).insert(WithCell(1.0));
        (walker, deck, spot)
    };
    queue.apply(app.world_mut());
    for _ in 0..90 {
        app.update();
    }
    *app.world_mut().get_mut::<Brain>(walker).unwrap() = Brain::Working {
        at: post,
        left: 999.0,
    };
    // Loaded and playing (the mechanism's own rules; loading by hand is
    // `radio::tests`).
    {
        let mut d = app.world_mut().get_mut::<TapeDeck>(deck).unwrap();
        d.deck.open_door();
        assert!(d.deck.insert(120.0, Side::A, 0.0));
        d.deck.close_door();
        d.deck.mode = Mode::Playing;
        d.tape = None;
    }
    let near = 3.5 + (20.0 * crate::robots::senses::HEARD_SPREAD);
    let mut closest = f32::MAX;
    for _ in 0..90 * 30 {
        app.update();
        let at = app.world().get::<Transform>(walker).unwrap().translation;
        closest = closest.min(flat(at - spot));
        if closest < near {
            break;
        }
    }
    assert!(
        closest < near,
        "walked to the music (closest {closest:.1} m of {near:.1})"
    );
}

/// Stopped, a player says nothing to the robots; playing, it does.
#[test]
fn a_tape_stopped_draws_nothing() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<crate::robots::Noise>()
        .add_systems(Update, super::deck::deck_noise);
    let deck = app
        .world_mut()
        .spawn((TapeDeck::default(), GlobalTransform::default()))
        .id();
    for _ in 0..10 {
        app.update();
    }
    let n = app
        .world()
        .resource::<Messages<crate::robots::Noise>>()
        .iter_current_update_messages()
        .count();
    assert_eq!(n, 0);
    app.world_mut().get_mut::<TapeDeck>(deck).unwrap().deck.mode = Mode::Playing;
    app.update();
    let n = app
        .world()
        .resource::<Messages<crate::robots::Noise>>()
        .iter_current_update_messages()
        .count();
    assert_eq!(n, 1, "playing, it's heard");
}
