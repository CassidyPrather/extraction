//! Using things: the radio, injectors, food, lights, flares.
//!
//! VR: press the primary button (A/X) while holding a non-gun item; food
//! and medicine are used with the hands instead (`interact::first_aid`),
//! medicine by the button too with the one-handed comfort setting. Flat: U
//! uses what's in the right hand.

use bevy::prelude::*;

use super::{Catalog, Item};
use crate::health::Health;
use crate::interact::{Hand, Held};
use crate::raid::{RadioCall, Raid};
use crate::rig::{RigHand, RigInput};
use crate::weapons::Gun;

/// Use the item in a hand.
/// A signal flare's star leaves the hand this fast, straight up: it
/// climbs about 60 m (`flares::star_step`), as a machine's distress
/// canister bursts (`robots::distress::HEIGHT`).
pub const SIGNAL_FLARE_SPEED: f32 = 52.0;

#[derive(Message, Clone, Copy, Debug)]
pub struct UseItem {
    pub player: Entity,
    pub item: Entity,
}

/// An injector used on `player` (recordings count heals
/// from this: the item is gone by the time anything else looks).
#[derive(Message, Clone, Copy, Debug)]
pub struct Treated {
    pub player: Entity,
    pub item: &'static str,
}

/// A light that's on.
#[derive(Component)]
pub struct LightOn;

/// Items used this frame (consumed ones are gone after it).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct UsageSystems;

pub struct UsagePlugin;

impl Plugin for UsagePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UseItem>()
            .add_message::<Treated>()
            .add_systems(
                Update,
                (vr_use_button, use_items.in_set(UsageSystems)).chain(),
            );
    }
}

/// Primary button on a hand holding something that isn't a gun.
fn vr_use_button(
    players: Query<(Entity, &RigInput, &Children)>,
    hands: Query<(&RigHand, &Hand)>,
    guns: Query<(), With<Gun>>,
    mut last: Local<std::collections::HashMap<(Entity, usize), bool>>,
    mut uses: MessageWriter<UseItem>,
    driver: Res<crate::rig::RigDriver>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
) {
    use extraction_core::items::Category;
    let one_handed = comfort.is_some_and(|c| c.one_handed);
    let by_hand = |e: Entity| {
        items.get(e).is_ok_and(|i| {
            let def = catalog.get(i.kind());
            def.is_food() || (def.category == Category::Medical && !one_handed)
        })
    };
    if *driver != crate::rig::RigDriver::Xr {
        return;
    }
    for (player, input, children) in &players {
        for c in children.iter() {
            let Ok((rh, h)) = hands.get(c) else {
                continue;
            };
            let pressed = input.hand(rh.side).primary;
            let was = last
                .insert((player, rh.side.index()), pressed)
                .unwrap_or(false);
            if pressed && !was {
                if let Some(item) = h.holding.filter(|i| !guns.contains(*i) && !by_hand(*i)) {
                    uses.write(UseItem { player, item });
                }
            }
        }
    }
}

fn use_items(
    mut commands: Commands,
    mut uses: MessageReader<UseItem>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut healths: Query<&mut Health>,
    mut radio: MessageWriter<RadioCall>,
    mut raid: ResMut<Raid>,
    lights: Query<(), With<LightOn>>,
    mut hands: Query<&mut Hand>,
    held: Query<&Held>,
    mut treated: MessageWriter<Treated>,
    at: Query<&GlobalTransform>,
) {
    for u in uses.read() {
        let Ok(item) = items.get(u.item) else {
            continue;
        };
        let def = catalog.get(item.kind());
        let mut consume = false;
        match def.id.as_str() {
            // Pull the cord: a red star straight up out of the hand, and
            // the tube's spent (#94).
            "signal_flare" => {
                if let Ok(gt) = at.get(u.item) {
                    use crate::weapons::flares::{Flare, Star};
                    commands.spawn((
                        Name::new("Flare"),
                        Flare::lit(Star::Red, Vec3::Y * SIGNAL_FLARE_SPEED, Some(u.player)),
                        Transform::from_translation(gt.translation() + Vec3::Y * 0.15),
                        Visibility::default(),
                    ));
                    consume = true;
                }
            }
            "radio" => {
                radio.write(RadioCall {
                    player: u.player,
                    pad: None,
                });
            }
            "regen_injector" => {
                if let Ok(mut h) = healths.get_mut(u.player) {
                    h.hp = (h.hp + 45.0).min(h.max);
                    raid.say("Cold, then warm. Something knits.");
                }
                treated.write(Treated {
                    player: u.player,
                    item: "regen_injector",
                });
                consume = true;
            }
            "flashlight" => {
                if lights.contains(u.item) {
                    commands.entity(u.item).remove::<LightOn>();
                } else {
                    commands.entity(u.item).insert(LightOn);
                }
            }
            _ if def.is_food() => {
                // No hunger meter. Eating is for the ritual of it.
                if def.tags.iter().any(|t| t == "food:Water") {
                    raid.say("You drink. It tastes of the bottle.");
                } else {
                    raid.say(format!(
                        "You eat the {}. It tastes of tin and smoke.",
                        def.name.to_lowercase()
                    ));
                }
                consume = true;
            }
            _ => {}
        }
        if consume {
            if let Ok(h) = held.get(u.item) {
                if let Ok(mut hand) = hands.get_mut(h.hand) {
                    hand.holding = None;
                }
            }
            commands.entity(u.item).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::flares::{Flare, Star, star_step};

    /// A signal flare's star climbs about as high as a machine's distress
    /// canister bursts: seen across the valley, not lost in the trees.
    #[test]
    fn a_signal_flares_star_climbs_about_sixty_metres() {
        let dt = 1.0 / 64.0;
        let (mut y, mut v) = (0.0, Vec3::Y * SIGNAL_FLARE_SPEED);
        while v.y > 0.0 {
            v = star_step(v, dt);
            y += v.y * dt;
        }
        assert!((55.0..65.0).contains(&y), "{y} m");
    }

    /// Used, a signal flare sends a red star up and the tube's gone.
    #[test]
    fn a_signal_flare_goes_up_red_and_is_spent() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let catalog = app.world().resource::<Catalog>().clone();
        let at = app.world().get::<Transform>(player).unwrap().translation + Vec3::Y;
        let item = {
            let world = app.world_mut();
            let e = crate::items::spawn_item(
                &mut world.commands(),
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind("signal_flare").expect("in the catalog"),
                    transform: Transform::from_translation(at),
                    condition: 1.0,
                },
            );
            world.flush();
            e
        };
        app.update();
        app.world_mut().write_message(UseItem { player, item });
        app.update();
        app.update();
        let stars: Vec<(Star, Vec3)> = app
            .world_mut()
            .query::<&Flare>()
            .iter(app.world())
            .map(|f| (f.star, f.velocity))
            .collect();
        assert_eq!(stars.len(), 1, "{stars:?}");
        assert_eq!(stars[0].0, Star::Red);
        assert!(stars[0].1.y > 30.0, "going up: {stars:?}");
        assert!(app.world().get_entity(item).is_err(), "the tube's spent");
    }
}
