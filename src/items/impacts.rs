//! Things landing. A loose item that was falling and suddenly isn't has hit
//! something: that's an [`ItemImpact`] — heard by the player (audio), and
//! by robots, as a noise where it landed. So a dropped pan gives you away,
//! and a tin can thrown down the street sends a walker to look at it.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Catalog, Item};
use crate::interact::body::InBackpack;
use crate::interact::{Held, InSocket};

/// What a landing item is mostly made of, for how it sounds and carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Metal,
    Wood,
    Plastic,
    Soft,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct ItemImpact {
    pub item: Entity,
    pub at: Vec3,
    /// How fast it was falling, m/s.
    pub speed: f32,
    pub mass: f32,
    pub material: Material,
}

/// Slower landings than this make no sound worth hearing.
pub const MIN_SPEED: f32 = 1.4;

pub struct ImpactPlugin;

impl Plugin for ImpactPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ItemImpact>().add_systems(
            FixedUpdate,
            (detect_impacts, impact_noise)
                .chain()
                .after(PhysicsSystems::Writeback)
                .before(crate::robots::perceive),
        );
    }
}

pub fn material_of(def: &extraction_core::items::ItemDef) -> Material {
    if def.has_tag("metal") {
        Material::Metal
    } else if def.has_tag("wood") {
        Material::Wood
    } else if def.has_tag("polymer") || def.has_tag("electronics") || def.has_tag("glass") {
        Material::Plastic
    } else {
        Material::Soft
    }
}

#[allow(clippy::type_complexity)]
fn detect_impacts(
    items: Query<
        (Entity, &Item, &LinearVelocity, &GlobalTransform),
        (
            Without<Held>,
            Without<InSocket>,
            Without<InBackpack>,
            Without<Sleeping>,
        ),
    >,
    catalog: Res<Catalog>,
    mut last: Local<std::collections::HashMap<Entity, Vec3>>,
    mut out: MessageWriter<ItemImpact>,
) {
    let mut now = std::collections::HashMap::with_capacity(last.len());
    for (e, item, vel, gt) in &items {
        let v = vel.0;
        if let Some(prev) = last.get(&e) {
            // Falling, then suddenly not: it hit something.
            let speed = -prev.y;
            if speed > MIN_SPEED && v.y > prev.y + 0.8 * speed {
                let def = catalog.get(item.kind());
                out.write(ItemImpact {
                    item: e,
                    at: gt.translation(),
                    speed,
                    mass: def.mass,
                    material: material_of(def),
                });
            }
        }
        now.insert(e, v);
    }
    *last = now;
}

/// How far a landing carries to robot ears: harder and heavier is louder,
/// metal rings, cloth thuds.
pub fn noise_radius(i: &ItemImpact) -> f32 {
    let base = (i.speed - MIN_SPEED).max(0.0) * 3.0 + i.mass.clamp(0.05, 5.0) * 4.0;
    let ring = match i.material {
        Material::Metal => 1.6,
        Material::Wood => 1.1,
        Material::Plastic => 0.9,
        Material::Soft => 0.4,
    };
    (base * ring).min(45.0)
}

fn impact_noise(
    mut impacts: MessageReader<ItemImpact>,
    mut noise: MessageWriter<crate::robots::Noise>,
) {
    for i in impacts.read() {
        let radius = noise_radius(i);
        if radius > 3.0 {
            noise.write(crate::robots::Noise {
                at: i.at,
                radius,
                // Not the player's own: it doesn't get a response team
                // sent, it gets looked at.
                source: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robots::{Brain, Job, JobKind};

    #[test]
    fn a_thrown_can_draws_a_walker_to_look() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let post = zone.on_ground(100.0, 200.0);
        let catalog = app.world().resource::<Catalog>().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let walker = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::robots::walker::spawn_walker(
                &mut c,
                &zone,
                post,
                Job {
                    kind: JobKind::Watch,
                    site: post,
                    duration: 999.0,
                    exit: post,
                },
            )
        };
        queue.apply(app.world_mut());
        for _ in 0..90 * 2 {
            app.update();
        }
        *app.world_mut().get_mut::<Brain>(walker).unwrap() = Brain::Working {
            at: post,
            left: 999.0,
        };
        // A metal tin landing hard 14 m off.
        let spot = zone.on_ground(post.x + 14.0, post.z + 3.0);
        let can = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind("canned_beans").unwrap(),
                    transform: Transform::from_translation(spot + Vec3::Y * 2.5),
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.update();
        app.world_mut().get_mut::<LinearVelocity>(can).unwrap().0 = Vec3::new(0.0, -7.0, 0.0);
        let mut looked = None;
        for _ in 0..90 * 3 {
            app.update();
            if let Brain::Investigate { at, .. } = *app.world().get::<Brain>(walker).unwrap() {
                looked = Some(at);
                break;
            }
        }
        let at = looked.expect("the walker went to look");
        assert!(
            Vec2::new(at.x - spot.x, at.z - spot.z).length() < 2.0,
            "at where the can landed ({at:.1} vs {spot:.1})"
        );
    }
}
