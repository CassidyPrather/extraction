//! Player health: bullets hurt, dressings and injectors heal, and going
//! down ends the raid.

use bevy::prelude::*;

use crate::player::{BodyCollider, Player};
use crate::weapons::ballistics::Hit;

#[derive(Component, Debug)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
    /// HP lost per second until bandaged. (Anything may lower `hp`
    /// directly; `bleed` notices when it reaches zero.)
    pub bleeding: f32,
    pub down: bool,
    /// Seconds since the last hit (for screen and audio effects).
    pub since_hit: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            hp: 100.0,
            max: 100.0,
            bleeding: 0.0,
            down: false,
            since_hit: 99.0,
        }
    }
}

/// A player was hurt (for effects).
#[derive(Message, Clone, Debug)]
pub struct Hurt {
    pub player: Entity,
    pub amount: f32,
    pub from: Vec3,
}

/// A hit was turned aside by the aegis knot.
#[derive(Message, Clone, Debug)]
pub struct Glanced {
    pub player: Entity,
    pub at: Vec3,
}

/// A player went down.
#[derive(Message, Clone, Debug)]
pub struct Downed {
    pub player: Entity,
    pub at: Vec3,
}

pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Hurt>()
            .add_message::<Downed>()
            .add_message::<Glanced>()
            .add_observer(|add: On<Add, Player>, mut commands: Commands| {
                commands.entity(add.entity).insert(Health::default());
            })
            .add_systems(FixedUpdate, (take_hits, falls, bleed).chain());
    }
}

fn take_hits(
    mut hits: MessageReader<Hit>,
    colliders: Query<&ChildOf, With<BodyCollider>>,
    mut players: Query<(
        &mut Health,
        &GlobalTransform,
        Option<&mut crate::anomaly::artifacts::ArtifactEffects>,
    )>,
    mut hurt: MessageWriter<Hurt>,
    mut downed: MessageWriter<Downed>,
    mut glanced: MessageWriter<Glanced>,
) {
    for hit in hits.read() {
        let Ok(parent) = colliders.get(hit.entity) else {
            continue;
        };
        let player = parent.parent();
        let Ok((mut h, gt, fx)) = players.get_mut(player) else {
            continue;
        };
        if h.down {
            continue;
        }
        let mut damage = hit.damage;
        // The aegis knot turns the first hit.
        if let Some(mut fx) = fx {
            if fx.aegis && fx.aegis_recharge <= 0.0 && damage > 5.0 {
                fx.aegis_recharge = crate::anomaly::artifacts::AEGIS_RECHARGE;
                damage *= 0.1;
                glanced.write(Glanced {
                    player,
                    at: hit.point,
                });
            }
        }
        h.hp -= damage;
        h.since_hit = 0.0;
        // Most hits bleed a little.
        h.bleeding = (h.bleeding + damage * 0.04).min(3.0);
        hurt.write(Hurt {
            player,
            amount: damage,
            from: hit.point - hit.velocity.normalize_or_zero() * 5.0,
        });
        if h.hp <= 0.0 {
            h.down = true;
            downed.write(Downed {
                player,
                at: gt.translation(),
            });
        }
    }
}

/// Landing hard hurts: past about 3 m of free fall, more the harder.
fn falls(mut players: Query<(&mut Health, &mut crate::player::Body)>) {
    for (mut h, mut body) in &mut players {
        let v = std::mem::take(&mut body.impact);
        if v > SAFE_LANDING && !h.down {
            let dmg = (v - SAFE_LANDING) * 9.0;
            h.hp -= dmg;
            h.since_hit = 0.0;
            h.bleeding = (h.bleeding + dmg * 0.02).min(3.0);
        }
    }
}

/// Landing speed (m/s) you walk away from.
pub const SAFE_LANDING: f32 = 8.0;

fn bleed(
    mut players: Query<(Entity, &mut Health, &GlobalTransform)>,
    time: Res<Time>,
    mut downed: MessageWriter<Downed>,
) {
    let dt = time.delta_secs();
    for (e, mut h, gt) in &mut players {
        h.since_hit += dt;
        if h.down {
            continue;
        }
        if h.bleeding > 0.0 {
            h.hp -= h.bleeding * dt;
            // Small wounds clot on their own, slowly.
            h.bleeding = (h.bleeding - 0.02 * dt).max(0.0);
        }
        // Whatever took the last of it (bleeding, a fall, an anomaly, the
        // Surge), down is down.
        if h.hp <= 0.0 {
            h.down = true;
            downed.write(Downed {
                player: e,
                at: gt.translation(),
            });
        }
    }
}
