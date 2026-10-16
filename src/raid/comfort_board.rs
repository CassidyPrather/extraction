//! The comfort board: three cards pinned on the bed's left wall, above the
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
        for (card, y) in [
            (ComfortCard::Turning, 1.62),
            (ComfortCard::Vignette, 1.44),
            (ComfortCard::Sound, 1.80),
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
                Transform::from_xyz(-1.12, FLOOR + y, -1.25)
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
