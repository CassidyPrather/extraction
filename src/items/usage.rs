//! Using things: the radio, bandages and injectors, food, lights.
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
#[derive(Message, Clone, Copy, Debug)]
pub struct UseItem {
    pub player: Entity,
    pub item: Entity,
}

/// A dressing or an injector used on `player` (recordings count heals
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
        items
            .get(e)
            .is_ok_and(|i| match catalog.get(i.kind()).category {
                Category::Food => true,
                Category::Medical => !one_handed,
                _ => false,
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
    on_limb: Query<&crate::interact::first_aid::OnLimb>,
    wounds: Query<&crate::interact::wounds::Wounds>,
    mut treated: MessageWriter<Treated>,
) {
    for u in uses.read() {
        let Ok(item) = items.get(u.item) else {
            continue;
        };
        let def = catalog.get(item.kind());
        let mut consume = false;
        match def.id.as_str() {
            "radio" => {
                radio.write(RadioCall { player: u.player });
            }
            "field_dressing" => {
                // Wound round a limb, it bound that limb's wounds
                // (`interact::first_aid`); by the button, every bleed.
                let limb = on_limb.get(u.item).ok().map(|l| l.0);
                if let Ok(mut h) = healths.get_mut(u.player) {
                    if limb.is_none() {
                        h.bleeding = 0.0;
                    }
                    h.hp = (h.hp + 8.0).min(h.max);
                    // What's true now, not what a dressing usually does.
                    raid.say(crate::interact::wounds::dressed_line(
                        limb,
                        wounds.get(u.player).ok(),
                        h.bleeding,
                    ));
                }
                treated.write(Treated {
                    player: u.player,
                    item: "field_dressing",
                });
                consume = true;
            }
            "regen_injector" => {
                if let Ok(mut h) = healths.get_mut(u.player) {
                    let was = h.bleeding;
                    h.hp = (h.hp + 45.0).min(h.max);
                    h.bleeding *= 0.3;
                    raid.say(if was < crate::interact::wounds::STILL {
                        "Cold, then warm. Something knits.".to_string()
                    } else {
                        // It slows a bleed; it doesn't stop one.
                        format!(
                            "Cold, then warm. Something knits. {}",
                            crate::interact::wounds::bleeding_now(
                                wounds.get(u.player).ok(),
                                h.bleeding
                            )
                        )
                    });
                }
                treated.write(Treated {
                    player: u.player,
                    item: "regen_injector",
                });
                consume = true;
            }
            "flashlight" | "weapon_light" => {
                if lights.contains(u.item) {
                    commands.entity(u.item).remove::<LightOn>();
                } else {
                    commands.entity(u.item).insert(LightOn);
                }
            }
            _ if def.category == extraction_core::items::Category::Food => {
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
