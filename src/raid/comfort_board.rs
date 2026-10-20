//! The comfort board: twelve cards pinned on the bed's left wall, above the
//! stash, for the settings a headset player can't reach otherwise. Press
//! one to step through its presets; the choice is kept in `settings.ron`.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::bench::{ScreenStyle, ScreenText};
use super::truck::{FLOOR, Pressable, Truck};
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::launch::LaunchOptions;
use crate::rig::comfort::Comfort;
use crate::world::Layer;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComfortCard {
    Turning,
    Vignette,
    Sound,
    Sharpness,
    FingerGrab,
    /// Sounds placed around your head (on), or plain stereo.
    Binaural,
    /// The music that rises with danger.
    Score,
    /// Which hand holds the gun.
    Handed,
    /// Standing or seated play.
    Seated,
    /// Two hands or one.
    OneHanded,
    /// Captions on the wrist.
    Captions,
    Climb,
}

pub struct ComfortBoardPlugin;

impl Plugin for ComfortBoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_cards, press_cards, card_text, follow_sound).chain(),
        );
    }
}

fn spawn_cards(mut commands: Commands, trucks: Query<Entity, Added<Truck>>) {
    for truck in &trucks {
        // Two columns: above 1.9 m the canvas slopes into the canopy.
        for (card, y, z) in [
            (ComfortCard::Turning, 1.62, -1.25),
            (ComfortCard::Vignette, 1.44, -1.25),
            (ComfortCard::Sound, 1.80, -1.25),
            (ComfortCard::Sharpness, 1.62, -0.85),
            (ComfortCard::FingerGrab, 1.44, -0.85),
            // A row under them, over the stash.
            (ComfortCard::Binaural, 1.26, -1.25),
            (ComfortCard::Score, 1.26, -0.85),
            // A third column, nearer the tailgate: how you play.
            (ComfortCard::Handed, 1.80, -0.50),
            (ComfortCard::Seated, 1.62, -0.50),
            (ComfortCard::OneHanded, 1.44, -0.50),
            (ComfortCard::Captions, 1.26, -0.50),
            (ComfortCard::Climb, 1.80, -0.85),
        ] {
            commands.spawn((
                Name::new("Comfort card"),
                card,
                Pressable::Setting,
                Grabbable {
                    points: vec![GrabPoint::free()],
                },
                Collider::cuboid(0.26, 0.14, 0.03),
                CollisionLayers::new(Layer::Item, LayerMask::NONE),
                ScreenText {
                    text: String::new(),
                    size: Vec2::new(0.26, 0.14),
                    style: ScreenStyle::Card,
                },
                // Left canvas wall, facing in, toward the front.
                Transform::from_xyz(-1.12, FLOOR + y, z)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
                Visibility::default(),
                ChildOf(truck),
            ));
        }
    }
}

fn press_cards(
    mut events: MessageReader<InteractEvent>,
    cards: Query<&ComfortCard>,
    comfort: Option<ResMut<Comfort>>,
    opts: Res<LaunchOptions>,
) {
    let Some(mut comfort) = comfort else {
        return;
    };
    for ev in events.read() {
        if let InteractEvent::Pressed { item, .. } = ev {
            let Ok(card) = cards.get(*item) else {
                continue;
            };
            match card {
                ComfortCard::Turning => comfort.next_turning(),
                ComfortCard::Vignette => comfort.next_vignette(),
                ComfortCard::Sound => comfort.narrow_dynamics = !comfort.narrow_dynamics,
                ComfortCard::Sharpness => comfort.next_render_scale(),
                ComfortCard::FingerGrab => comfort.finger_grab = !comfort.finger_grab,
                ComfortCard::Binaural => comfort.binaural = !comfort.binaural,
                ComfortCard::Score => comfort.score = !comfort.score,
                ComfortCard::Handed => comfort.left_handed = !comfort.left_handed,
                ComfortCard::Seated => comfort.seated = !comfort.seated,
                ComfortCard::OneHanded => comfort.one_handed = !comfort.one_handed,
                ComfortCard::Captions => comfort.captions = !comfort.captions,
                ComfortCard::Climb => comfort.climb_teleport = !comfort.climb_teleport,
            }
            // Tests and sims don't touch the player's settings.
            if !opts.headless {
                comfort.save();
            }
        }
    }
}

fn card_text(
    comfort: Option<Res<Comfort>>,
    mut cards: Query<(&ComfortCard, &mut ScreenText)>,
    added: Query<(), Added<ComfortCard>>,
) {
    let Some(comfort) = comfort else {
        return;
    };
    if !comfort.is_changed() && added.is_empty() {
        return;
    }
    for (card, mut text) in &mut cards {
        let t = match card {
            ComfortCard::Turning => comfort.turning_label(),
            ComfortCard::Vignette => comfort.vignette_label(),
            ComfortCard::Sound => comfort.sound_label(),
            ComfortCard::Sharpness => comfort.render_scale_label(),
            ComfortCard::FingerGrab => comfort.finger_grab_label(),
            ComfortCard::Binaural => comfort.binaural_label(),
            ComfortCard::Score => comfort.score_label(),
            ComfortCard::Handed => comfort.handed_label(),
            ComfortCard::Seated => comfort.seated_label(),
            ComfortCard::OneHanded => comfort.one_handed_label(),
            ComfortCard::Captions => comfort.captions_label(),
            ComfortCard::Climb => comfort.climb_label(),
        };
        if text.text != t {
            text.text = t;
        }
    }
}

/// The mix follows the sound card.
fn follow_sound(comfort: Option<Res<Comfort>>) {
    if let Some(c) = comfort
        && c.is_changed()
    {
        crate::audio_mix::set_narrow(c.narrow_dynamics);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cards for how you play each switch their setting, and say so.
    #[test]
    fn the_access_cards_switch_hand_seat_hands_and_captions() {
        let mut app = crate::headless_app(Default::default(), false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        *app.world_mut().resource_mut::<Comfort>() = Comfort::default();
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let read = |c: &Comfort, card: ComfortCard| match card {
            ComfortCard::Handed => c.left_handed,
            ComfortCard::Seated => c.seated,
            ComfortCard::OneHanded => c.one_handed,
            _ => !c.captions,
        };
        for (which, on_word) in [
            (ComfortCard::Handed, "left"),
            (ComfortCard::Seated, "seated"),
            (ComfortCard::OneHanded, "one"),
            (ComfortCard::Captions, "off"),
        ] {
            let card = app
                .world_mut()
                .query::<(Entity, &ComfortCard)>()
                .iter(app.world())
                .find(|(_, c)| **c == which)
                .map(|(e, _)| e)
                .unwrap_or_else(|| panic!("a {which:?} card in the truck"));
            assert!(!read(app.world().resource::<Comfort>(), which));
            app.world_mut()
                .write_message(InteractEvent::Pressed { item: card, player });
            app.update();
            app.update();
            assert!(read(app.world().resource::<Comfort>(), which), "{which:?}");
            let text = &app.world().get::<ScreenText>(card).unwrap().text;
            assert!(text.contains(on_word), "{which:?} card says: {text}");
        }
    }

    /// The binaural and score cards each switch their setting, and say so.
    #[test]
    fn the_audio_cards_switch_binaural_and_the_score() {
        let mut app = crate::headless_app(Default::default(), false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        *app.world_mut().resource_mut::<Comfort>() = Comfort::default();
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        for (which, off) in [
            (ComfortCard::Binaural, "STEREO"),
            (ComfortCard::Score, "OFF"),
        ] {
            let card = app
                .world_mut()
                .query::<(Entity, &ComfortCard)>()
                .iter(app.world())
                .find(|(_, c)| **c == which)
                .map(|(e, _)| e)
                .unwrap_or_else(|| panic!("a {which:?} card in the truck"));
            let on = |app: &App| {
                let c = app.world().resource::<Comfort>();
                if which == ComfortCard::Binaural {
                    c.binaural
                } else {
                    c.score
                }
            };
            assert!(on(&app), "{which:?} starts on");
            for want in [false, true] {
                app.world_mut()
                    .write_message(InteractEvent::Pressed { item: card, player });
                app.update();
                app.update();
                assert_eq!(on(&app), want, "{which:?} after a press");
                let text = &app.world().get::<ScreenText>(card).unwrap().text;
                assert_eq!(text.contains(off), !want, "{which:?} card says: {text}");
            }
        }
    }

    #[test]
    fn pressing_the_turning_card_changes_how_you_turn() {
        let mut app = crate::headless_app(Default::default(), false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        *app.world_mut().resource_mut::<Comfort>() = Comfort::default();
        app.update();
        let (card, player) = {
            let w = app.world_mut();
            let card = w
                .query::<(Entity, &ComfortCard)>()
                .iter(w)
                .find(|(_, c)| **c == ComfortCard::Turning)
                .map(|(e, _)| e)
                .expect("a turning card in the truck");
            let player = w
                .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
                .iter(w)
                .next()
                .unwrap();
            (card, player)
        };
        app.world_mut()
            .write_message(InteractEvent::Pressed { item: card, player });
        app.update();
        app.update();
        assert!((app.world().resource::<Comfort>().snap_angle - 45.0).abs() < 0.1);
        let text = &app.world().get::<ScreenText>(card).unwrap().text;
        assert!(text.contains("45"), "the card says so: {text}");
    }
}
