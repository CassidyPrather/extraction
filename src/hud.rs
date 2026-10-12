//! The flat-screen HUD: a crosshair, what you're looking at, what your
//! hands hold, and your load. VR gets diegetic readouts instead.

use bevy::prelude::*;
use extraction_core::inventory::Encumbrance;

use crate::interact::Hand;
use crate::interact::body::Backpack;
use crate::interact::flat::FlatTarget;
use crate::items::{Catalog, Item};
use crate::player::Body;
use crate::rig::{LocalPlayer, RigHand, Side};
use crate::weapons::{Gun, Mag, mode_name};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, update_hud);
    }
}

#[derive(Component)]
struct HudText;

#[derive(Component)]
struct TargetText;

fn spawn_hud(mut commands: Commands, opts: Res<crate::launch::LaunchOptions>) {
    if opts.screenshot.is_some() {
        return;
    }
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(50.0),
            top: percent(50.0),
            width: px(4.0),
            height: px(4.0),
            margin: UiRect::all(px(-2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
    ));
    commands.spawn((
        TargetText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgba(0.95, 0.95, 0.9, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            left: percent(52.0),
            top: percent(52.0),
            ..default()
        },
    ));
    commands.spawn((
        HudText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgba(0.85, 0.87, 0.85, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            left: px(14.0),
            bottom: px(12.0),
            ..default()
        },
    ));
}

fn describe(
    e: Entity,
    items: &Query<&Item>,
    guns: &Query<&Gun>,
    mags: &Query<&Mag>,
    catalog: &Catalog,
) -> String {
    let Ok(item) = items.get(e) else {
        return "backpack".into();
    };
    let def = catalog.get(item.kind());
    if let Ok(gun) = guns.get(e) {
        let f = &gun.firearm;
        let chamber = match f.chambers.first().copied().flatten() {
            Some(extraction_core::firearm::Round::Live) => "chambered",
            Some(extraction_core::firearm::Round::Spent) => "spent case",
            None => "empty chamber",
        };
        let jam = if f.malfunction.is_some() {
            " JAMMED"
        } else {
            ""
        };
        return format!(
            "{} [{} | {} in feed | {}{} | cond {:.0}% fouling {:.0}%]",
            def.name,
            mode_name(f.fire_mode()),
            f.rounds_in_feed(),
            chamber,
            jam,
            f.condition * 100.0,
            f.fouling * 100.0
        );
    }
    if mags.get(e).is_ok() || def.category == extraction_core::items::Category::Ammo {
        return format!("{} ({} rds)", def.name, item.rounds);
    }
    def.name.clone()
}

fn update_hud(
    mut hud: Query<&mut Text, (With<HudText>, Without<TargetText>)>,
    mut target_text: Query<&mut Text, (With<TargetText>, Without<HudText>)>,
    target: Res<FlatTarget>,
    players: Query<(Entity, &Body, &Children), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand)>,
    items: Query<&Item>,
    guns: Query<&Gun>,
    mags: Query<&Mag>,
    bags: Query<&Backpack>,
    catalog: Res<Catalog>,
) {
    if let Ok(mut t) = target_text.single_mut() {
        t.0 = target
            .entity
            .map(|e| format!("{}  [E]", describe(e, &items, &guns, &mags, &catalog)))
            .unwrap_or_default();
    }
    let Ok(mut text) = hud.single_mut() else {
        return;
    };
    let Some((player, body, children)) = players.iter().next() else {
        return;
    };
    let mut lines = Vec::new();
    for c in children.iter() {
        if let Ok((rh, h)) = hands.get(c) {
            let what = match (h.holding, h.supporting, h.handle) {
                (Some(e), _, _) => describe(e, &items, &guns, &mags, &catalog),
                (_, Some(_), _) => "steadying".into(),
                (_, _, Some(_)) => "working the action".into(),
                _ => "-".into(),
            };
            let side = if rh.side == Side::Left { "L" } else { "R" };
            lines.push(format!("{side}: {what}"));
        }
    }
    lines.sort();
    let enc = Encumbrance::from_mass(body.carried_kg);
    let bag = bags.iter().find(|b| b.owner == player && b.carried);
    let used: f32 = bag
        .map(|b| {
            b.contents
                .iter()
                .filter_map(|e| items.get(*e).ok())
                .map(|i| catalog.get(i.kind()).bulk)
                .sum()
        })
        .unwrap_or(0.0);
    lines.push(format!(
        "load {:.1} kg (speed x{:.2}{}) | pack {:.1}/{:.0} L{} | stamina {:.0}%",
        body.carried_kg,
        enc.speed,
        if enc.can_sprint { "" } else { ", no sprint" },
        used,
        bag.map_or(0.0, |b| b.model.capacity),
        if bag.is_some_and(|b| b.open) {
            " (open)"
        } else {
            ""
        },
        body.stamina * 100.0
    ));
    lines.push("WASD move | E grab | F 2nd hand | Q drop | G stow | Tab pack | 1-0 draw | LMB fire | RMB aim | R reload | V rack | X mode | B mag | C combine".into());
    text.0 = lines.join("\n");
}
