//! Things landing. A loose item that was falling and suddenly isn't has hit
//! something: that's an [`ItemImpact`] — heard by the player (audio), and
//! by robots, as a noise where it landed. So a dropped pan gives you away,
//! and a tin can thrown down the street sends a walker to look at it.

use avian3d::prelude::*;
use bevy::prelude::*;

use extraction_core::terrain::Surface;

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
    /// The bare ground it came down on, if that's what it hit (not a
    /// crate, a rock or a floor).
    pub ground: Option<Surface>,
}

/// Grass, leaf litter and mud take a fall quietly, whatever lands on them.
pub fn soft(ground: Option<Surface>) -> bool {
    matches!(
        ground,
        Some(Surface::Grass | Surface::Forest | Surface::Dirt | Surface::Mud | Surface::Water)
    )
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
    zone: Option<Res<crate::world::Zone>>,
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
                let at = gt.translation();
                let ground = zone.as_ref().and_then(|z| {
                    (at.y - z.ground(at.x, at.z) < 0.2)
                        .then(|| z.hm.surface([at.x, at.z], z.forest_cover(at.x, at.z) > 0.5))
                });
                out.write(ItemImpact {
                    item: e,
                    at,
                    speed,
                    mass: def.mass,
                    material: material_of(def),
                    ground,
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
    let muffle = if soft(i.ground) { 0.5 } else { 1.0 };
    (base * ring * muffle).min(45.0)
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
        // It heard roughly where: within its guess's spread of the spot.
        let spread = (14.0 * crate::robots::senses::HEARD_SPREAD)
            .max(crate::robots::senses::HEARD_SPREAD_MIN);
        assert!(
            Vec2::new(at.x - spot.x, at.z - spot.z).length() < spread + 0.5,
            "at where the can landed ({at:.1} vs {spot:.1})"
        );
    }

    /// A magazine dropped in the yard's grass lands soft (the ground takes
    /// it: heard as grass, carrying half as far); the same magazine dropped
    /// on a supply crate's lid lands hard.
    #[test]
    fn grass_takes_a_fall_and_a_crate_rings() {
        #[derive(Resource, Default)]
        struct Heard(Vec<ItemImpact>);
        let opts = crate::LaunchOptions {
            robots: false,
            map: crate::camp::TRAINING_YARD.into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.init_resource::<Heard>().add_systems(
            FixedUpdate,
            (|mut r: MessageReader<ItemImpact>, mut h: ResMut<Heard>| {
                h.0.extend(r.read().copied())
            })
            .after(detect_impacts),
        );
        app.finish();
        app.cleanup();
        for _ in 0..30 {
            app.update();
        }
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let catalog = app.world().resource::<Catalog>().clone();
        let crate_top = app
            .world_mut()
            .query_filtered::<&GlobalTransform, With<crate::items::loot::SupplyCrate>>()
            .iter(app.world())
            .next()
            .expect("a supply crate in the yard")
            .translation()
            + Vec3::Y * crate::items::loot::SUPPLY_CRATE.y * 0.5;
        let mut drop = |at: Vec3| {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let e = {
                let mut c = Commands::new(&mut queue, app.world());
                crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind: catalog.kind("pm_mag").unwrap(),
                        transform: Transform::from_translation(at),
                        condition: 1.0,
                    },
                )
            };
            queue.apply(app.world_mut());
            app.world_mut().resource_mut::<Heard>().0.clear();
            for _ in 0..90 {
                app.update();
            }
            app.world()
                .resource::<Heard>()
                .0
                .iter()
                .find(|i| i.item == e)
                .copied()
                .expect("it landed")
        };
        let grass = drop(zone.on_ground(12.0, 40.0) + Vec3::Y * 1.2);
        let lid = drop(crate_top + Vec3::Y * 1.2);
        assert!(soft(grass.ground), "grass: {:?}", grass.ground);
        assert!(!soft(lid.ground), "the crate: {:?}", lid.ground);
        assert!(noise_radius(&grass) < noise_radius(&lid) * 0.7);
    }
}
