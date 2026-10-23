//! The driver's contracts in the game (`extraction_core::contracts` has the
//! rules and the arc): what a step needs out in the Zone (the thing to bring
//! laid by its place, a planted thing noticed, the camera's photograph), and
//! the pay when the last step's in.
//!
//! A contract is started by `offer_job` (its kit comes through the cab's
//! hatch) and each step is a job on the card, handed in at
//! `raid/requests.rs` like any other.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::contracts::{Contract, PHOTO_CONE, PHOTO_RANGE, PLANT_RADIUS, Unlock};
use extraction_core::progression::Job;

use super::{Progression, lines};
use crate::interact::{Held, InSocket, body::InBackpack};
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::launch::LaunchOptions;
use crate::raid::{Phase, Raid};
use crate::rig::RigHead;
use crate::robots::{Breed, Robot};
use crate::world::{Layer, Zone};

/// A contract's thing to bring lies this close to its place (a building's
/// loot spot if there's one; else on the ground there), m.
const BRING_NEAR: f32 = 30.0;

/// Its kit through the cab's hatch, as the contract starts.
pub fn hand_kit(
    c: &Contract,
    raid: &mut Raid,
    handover: &mut MessageWriter<crate::raid::lev::HandOver>,
) {
    for (id, n) in c.kit {
        for _ in 0..*n {
            handover.write(crate::raid::lev::HandOver {
                id: id.to_string(),
                condition: 1.0,
            });
        }
    }
    if !c.kit.is_empty() {
        raid.say(lines::CONTRACT_KIT);
    }
}

/// Its pay, and what it opens, as the last step's handed in.
pub fn pay(
    c: &Contract,
    raid: &mut Raid,
    handover: &mut MessageWriter<crate::raid::lev::HandOver>,
) {
    if c.done.is_empty() {
        raid.say(lines::contract_done(c.name));
    } else {
        raid.say(c.done);
    }
    for (id, n) in c.pay {
        for _ in 0..*n {
            handover.write(crate::raid::lev::HandOver {
                id: id.to_string(),
                condition: 1.0,
            });
        }
    }
    for u in c.unlocks {
        match u {
            Unlock::Recipe(r) => {
                if let Some(r) = extraction_core::fabrication::recipe(r) {
                    raid.say(lines::recipe_taught(r.name));
                }
            }
            Unlock::Valley(v) => raid.say(lines::valley_opened(&super::valley_name(v))),
        }
    }
}

/// A Bring step in this valley with its thing nowhere: laid by its place
/// (again, if the Surge or the convoy took it).
pub fn place_bring(
    mut commands: Commands,
    progression: Res<Progression>,
    opts: Res<LaunchOptions>,
    zone: Res<Zone>,
    spots: Option<Res<crate::world::buildings::BuildingLoot>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut tried: Local<u32>,
) {
    let Some(Job::Bring { map, item, at, .. }) = &progression.0.job else {
        return;
    };
    if *map != opts.map {
        return;
    }
    let Some(kind) = catalog.kind(item) else {
        return;
    };
    if items.iter().any(|i| i.kind() == kind) {
        return;
    }
    // Only so often: the buildings may still be going up.
    *tried += 1;
    if *tried % 30 != 1 {
        return;
    }
    let near = Vec2::from_array(*at);
    let spot = spots
        .iter()
        .flat_map(|s| s.0.iter().map(|s| s.at))
        // Above ground: a place's thing isn't down a site under it.
        .filter(|p| p.y >= zone.ground(p.x, p.z) - 1.5)
        .filter(|p| p.xz().distance(near) < BRING_NEAR)
        .min_by(|a, b| a.xz().distance(near).total_cmp(&b.xz().distance(near)));
    let p = spot.unwrap_or_else(|| zone.on_ground(near.x, near.y) + Vec3::Y * 0.1);
    spawn_item(
        &mut commands,
        &catalog,
        SpawnItem {
            kind,
            transform: Transform::from_translation(p + Vec3::Y * 0.06),
            condition: 0.9,
        },
    );
    info!("{item} laid at {p:?}");
}

/// A Plant step: its thing set down (not held, not packed) by its place,
/// out on a raid in its valley.
pub fn watch_plant(
    mut commands: Commands,
    mut progression: ResMut<Progression>,
    mut raid: ResMut<Raid>,
    opts: Res<LaunchOptions>,
    items: Query<
        (&Item, &GlobalTransform),
        (Without<Held>, Without<InBackpack>, Without<InSocket>),
    >,
    catalog: Res<Catalog>,
) {
    let out = raid.phase != Phase::AtBase;
    let Some(Job::Plant {
        map,
        place,
        item,
        at,
        planted,
    }) = &mut progression.0.job
    else {
        return;
    };
    if *planted || !out || *map != opts.map {
        return;
    }
    let Some(kind) = catalog.kind(item) else {
        return;
    };
    let near = Vec2::from_array(*at);
    if items
        .iter()
        .any(|(i, g)| i.kind() == kind && g.translation().xz().distance(near) < PLANT_RADIUS)
    {
        *planted = true;
        raid.say(lines::planted(place));
        crate::rig::haptics::step_done(&mut commands, None);
    }
}

/// The camera used (`items::usage::UseItem`) with a Photograph step on the
/// card: a machine of the step's kind in the middle of the view, in range
/// and in sight, is the picture. Going down spoils the film.
#[allow(clippy::too_many_arguments)]
pub fn photograph(
    mut commands: Commands,
    mut uses: MessageReader<crate::items::usage::UseItem>,
    mut progression: ResMut<Progression>,
    mut raid: ResMut<Raid>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    robots: Query<(
        Entity,
        &Robot,
        &GlobalTransform,
        Option<&Breed>,
        (
            Has<crate::robots::indoor::Crawler>,
            Has<crate::robots::hound::Hound>,
            Has<crate::robots::grappler::Grappler>,
            Has<crate::robots::indoor::Dormant>,
        ),
    )>,
    spatial: SpatialQuery,
) {
    let down = matches!(raid.phase, Phase::Down { .. });
    if let Some(Job::Photograph { taken, .. }) = &mut progression.0.job {
        if *taken && down {
            *taken = false;
            raid.say(lines::PHOTO_SPOILT);
        }
    }
    for u in uses.read() {
        let Ok(item) = items.get(u.item) else {
            continue;
        };
        if catalog.get(item.kind()).id != "camera" {
            continue;
        }
        let Some((head, _)) = heads.iter().find(|(_, c)| c.parent() == u.player) else {
            continue;
        };
        let eye = head.translation();
        let look = head.forward().as_vec3();
        let subject = match &progression.0.job {
            Some(Job::Photograph {
                subject,
                taken: false,
            }) => Some(*subject),
            _ => None,
        };
        let framed = subject.is_some_and(|subject| {
            robots
                .iter()
                .any(|(e, r, g, breed, (crawler, hound, grappler, dormant))| {
                    // Only the kind the card names (`robots::subjects`).
                    let is = crate::robots::subjects::counts(
                        subject,
                        r,
                        breed,
                        crate::robots::subjects::Shape {
                            crawler,
                            hound,
                            grappler,
                            dormant,
                        },
                    );
                    let to = g.translation() - eye;
                    let d = to.length();
                    if r.wrecked || !is || d > PHOTO_RANGE || d < 0.5 {
                        return false;
                    }
                    if look.angle_between(to) > PHOTO_CONE {
                        return false;
                    }
                    // In sight: nothing of the world's between (its own body
                    // may be what the ray meets).
                    let dir = Dir3::new(to).unwrap_or(Dir3::NEG_Z);
                    spatial
                        .cast_ray(
                            eye,
                            dir,
                            d,
                            true,
                            &SpatialQueryFilter::from_mask(Layer::World),
                        )
                        .is_none_or(|hit| hit.entity == e || hit.distance > d - 1.5)
                })
        });
        match (&mut progression.0.job, framed) {
            (Some(Job::Photograph { subject, taken }), true) => {
                *taken = true;
                raid.say(lines::photo_taken(subject.name()));
                crate::rig::haptics::step_done(&mut commands, None);
            }
            _ => raid.say(lines::PHOTO_EMPTY),
        }
    }
}

pub mod scenario;
