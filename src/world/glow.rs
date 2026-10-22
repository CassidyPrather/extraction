//! Throwable lights for the dark (`docs/UNDERGROUND.md`, "The dark"): a
//! chemlight, snapped, glows green for ten minutes without a sound; a hand
//! flare, struck, burns red for a minute. Held or thrown, a light that's
//! going lights whoever stands by it for the machines' eyes
//! (`robots::senses::Lighting`), and a new one is a sign they come to look
//! at; a burning flare in firedamp sets it off (`world::workings`).
//!
//! Logic in [`GlowPlugin`] (headless-safe); the lights themselves in
//! [`GlowVisualsPlugin`].

use bevy::prelude::*;

/// How long each burns (s).
pub const CHEM_BURN: f32 = 600.0;
pub const FLARE_BURN: f32 = 60.0;
/// How far each lights a body, for the machines (m).
pub const CHEM_REACH: f32 = 4.0;
pub const FLARE_REACH: f32 = 9.0;

pub struct GlowPlugin;

impl Plugin for GlowPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Lit>().add_systems(
            Update,
            (light_up, burn)
                .chain()
                .after(crate::items::usage::UsageSystems),
        );
    }
}

pub struct GlowVisualsPlugin;

impl Plugin for GlowVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, glow);
    }
}

/// Which light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlowKind {
    Chem,
    Flare,
}

impl GlowKind {
    pub fn of(id: &str) -> Option<Self> {
        match id {
            "chemlight" => Some(Self::Chem),
            "hand_flare" => Some(Self::Flare),
            _ => None,
        }
    }

    pub fn reach(self) -> f32 {
        match self {
            Self::Chem => CHEM_REACH,
            Self::Flare => FLARE_REACH,
        }
    }

    fn burn(self) -> f32 {
        match self {
            Self::Chem => CHEM_BURN,
            Self::Flare => FLARE_BURN,
        }
    }
}

/// A light that's going: what it is, how long it has left, and where it
/// was last put among the machines' lamps.
#[derive(Component, Debug)]
pub struct Glowing {
    pub kind: GlowKind,
    pub left: f32,
    lamp: Option<Vec3>,
}

/// A burning flare: an open flame (firedamp goes up at it).
#[derive(Component, Debug)]
pub struct Burning;

/// Burnt out: it won't go again.
#[derive(Component, Debug)]
pub struct Spent;

/// A light snapped or struck.
#[derive(Message, Clone, Copy, Debug)]
pub struct Lit {
    pub item: Entity,
    pub kind: GlowKind,
    pub at: Vec3,
}

/// Used in a hand: a chemlight snaps, a flare strikes. Once.
fn light_up(
    mut commands: Commands,
    mut uses: MessageReader<crate::items::usage::UseItem>,
    items: Query<(&crate::items::Item, &GlobalTransform), (Without<Glowing>, Without<Spent>)>,
    catalog: Res<crate::items::Catalog>,
    mut lit: MessageWriter<Lit>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for u in uses.read() {
        let Ok((item, gt)) = items.get(u.item) else {
            continue;
        };
        let Some(kind) = GlowKind::of(&catalog.get(item.kind()).id) else {
            continue;
        };
        let mut e = commands.entity(u.item);
        e.insert(Glowing {
            kind,
            left: kind.burn(),
            lamp: None,
        });
        if kind == GlowKind::Flare {
            e.insert(Burning);
        }
        lit.write(Lit {
            item: u.item,
            kind,
            at: gt.translation(),
        });
        raid.say(match kind {
            GlowKind::Chem => "It snaps inside, and green light floods the tube.",
            GlowKind::Flare => "The cap rasps across the end and the flare takes, spitting red.",
        });
    }
}

/// Lights burn down; while going, each lights the bodies by it for the
/// machines (moved with it, wherever it's thrown); burnt out, spent.
fn burn(
    mut commands: Commands,
    mut lights: Query<(Entity, &mut Glowing, &GlobalTransform)>,
    lighting: Option<ResMut<crate::robots::senses::Lighting>>,
    time: Res<Time>,
) {
    let mut lighting = lighting;
    let dt = time.delta_secs();
    for (e, mut g, gt) in &mut lights {
        g.left -= dt;
        let at = gt.translation();
        if let Some(l) = lighting.as_mut() {
            if let Some(old) = g.lamp.take() {
                if let Some(k) = l.lamps.iter().position(|(p, _)| p.distance(old) < 1e-4) {
                    l.lamps.swap_remove(k);
                }
            }
            if g.left > 0.0 {
                l.lamps.push((at, g.kind.reach()));
                g.lamp = Some(at);
            }
        }
        if g.left <= 0.0 {
            commands
                .entity(e)
                .remove::<(Glowing, Burning)>()
                .insert(Spent);
        }
    }
}

/// Client: a chemlight's soft green, a flare's spitting red; nothing once
/// it's out.
fn glow(
    mut commands: Commands,
    mut lights: Query<(Entity, &Glowing, Option<&mut PointLight>)>,
    mut gone: RemovedComponents<Glowing>,
    time: Res<Time>,
) {
    for e in gone.read() {
        if let Ok(mut c) = commands.get_entity(e) {
            c.remove::<PointLight>();
        }
    }
    let t = time.elapsed_secs();
    for (e, g, light) in &mut lights {
        let (colour, range, full) = match g.kind {
            GlowKind::Chem => (Color::srgb(0.35, 1.0, 0.3), CHEM_REACH + 1.0, 9_000.0),
            GlowKind::Flare => (Color::srgb(1.0, 0.18, 0.08), FLARE_REACH + 3.0, 90_000.0),
        };
        // A chemlight fades over its last minute; a flare spits all through.
        let level = match g.kind {
            GlowKind::Chem => (g.left / 60.0).clamp(0.15, 1.0),
            GlowKind::Flare => {
                0.75 + 0.25
                    * ((t * 31.0 + e.to_bits() as f32 * 0.37).sin() * (t * 17.0).sin()).abs()
            }
        };
        match light {
            Some(mut l) => l.intensity = full * level,
            None => {
                commands.entity(e).insert(PointLight {
                    color: colour,
                    intensity: full * level,
                    range,
                    radius: 0.02,
                    shadow_maps_enabled: false,
                    ..default()
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robots::senses::Lighting;

    /// A chemlight snapped in the hand lights its holder for the machines,
    /// thrown it lights where it lands, and spent it's dark for good; a
    /// flare struck burns as an open flame and is out in a minute.
    #[test]
    fn a_light_goes_lights_its_place_and_goes_out() {
        let mut vr = crate::vr_harness::Vr::new("marrow_creek");
        let player = vr.player;
        let feet = vr.feet();
        let chem = vr.spawn(
            "chemlight",
            Transform::from_translation(feet + Vec3::new(0.5, 1.0, 0.0)),
        );
        let lamps =
            |vr: &crate::vr_harness::Vr| vr.app.world().resource::<Lighting>().lamps.clone();
        let before = lamps(&vr).len();
        vr.app
            .world_mut()
            .write_message(crate::items::usage::UseItem { player, item: chem });
        vr.step(3);
        assert!(vr.app.world().get::<Glowing>(chem).is_some(), "snapped");
        let now = lamps(&vr);
        assert_eq!(now.len(), before + 1, "a lamp for the machines");
        // Moved: the lamp follows, still one.
        vr.app
            .world_mut()
            .get_mut::<Transform>(chem)
            .unwrap()
            .translation += Vec3::X * 3.0;
        vr.step(3);
        let moved = lamps(&vr);
        assert_eq!(moved.len(), before + 1, "still one lamp");
        let at = vr
            .app
            .world()
            .get::<GlobalTransform>(chem)
            .unwrap()
            .translation();
        assert!(
            moved
                .iter()
                .any(|(p, r)| p.distance(at) < 0.05 && (*r - CHEM_REACH).abs() < 1e-3),
            "where it lies now"
        );
        // Snapped again: nothing new.
        vr.app
            .world_mut()
            .write_message(crate::items::usage::UseItem { player, item: chem });
        vr.step(2);
        assert_eq!(lamps(&vr).len(), before + 1);
        // Burnt down: dark, and spent.
        vr.app.world_mut().get_mut::<Glowing>(chem).unwrap().left = 0.01;
        vr.step(3);
        assert!(vr.app.world().get::<Spent>(chem).is_some(), "spent");
        assert_eq!(lamps(&vr).len(), before, "its lamp gone");
        vr.app
            .world_mut()
            .write_message(crate::items::usage::UseItem { player, item: chem });
        vr.step(2);
        assert!(
            vr.app.world().get::<Glowing>(chem).is_none(),
            "a spent one won't go again"
        );

        let flare = vr.spawn(
            "hand_flare",
            Transform::from_translation(feet + Vec3::new(-0.5, 1.0, 0.0)),
        );
        vr.app
            .world_mut()
            .write_message(crate::items::usage::UseItem {
                player,
                item: flare,
            });
        vr.step(3);
        assert!(
            vr.app.world().get::<Burning>(flare).is_some(),
            "an open flame"
        );
        let left = vr.app.world().get::<Glowing>(flare).unwrap().left;
        assert!(
            left <= FLARE_BURN && left > FLARE_BURN - 1.0,
            "a minute: {left}"
        );
    }
}
