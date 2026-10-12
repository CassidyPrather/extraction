//! Player health: bullets hurt, dressings and injectors heal, and going
//! down ends the raid.

use bevy::prelude::*;

use crate::player::{BodyCollider, Player};
use crate::weapons::ballistics::Hit;

#[derive(Component, Debug)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
    /// HP lost per second until bandaged.
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
            .add_observer(|add: On<Add, Player>, mut commands: Commands| {
                commands.entity(add.entity).insert(Health::default());
            })
            .add_systems(FixedUpdate, (take_hits, bleed).chain());
    }
}

fn take_hits(
    mut hits: MessageReader<Hit>,
    colliders: Query<&ChildOf, With<BodyCollider>>,
    mut players: Query<(&mut Health, &GlobalTransform)>,
    mut hurt: MessageWriter<Hurt>,
    mut downed: MessageWriter<Downed>,
) {
    for hit in hits.read() {
        let Ok(parent) = colliders.get(hit.entity) else {
            continue;
        };
        let player = parent.parent();
        let Ok((mut h, gt)) = players.get_mut(player) else {
            continue;
        };
        if h.down {
            continue;
        }
        h.hp -= hit.damage;
        h.since_hit = 0.0;
        // Most hits bleed a little.
        h.bleeding = (h.bleeding + hit.damage * 0.04).min(3.0);
        hurt.write(Hurt {
            player,
            amount: hit.damage,
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

fn bleed(
    mut players: Query<(Entity, &mut Health, &GlobalTransform)>,
    time: Res<Time>,
    mut downed: MessageWriter<Downed>,
) {
    let dt = time.delta_secs();
    for (e, mut h, gt) in &mut players {
        h.since_hit += dt;
        if h.down || h.bleeding <= 0.0 {
            continue;
        }
        h.hp -= h.bleeding * dt;
        // Small wounds clot on their own, slowly.
        h.bleeding = (h.bleeding - 0.02 * dt).max(0.0);
        if h.hp <= 0.0 {
            h.down = true;
            downed.write(Downed {
                player: e,
                at: gt.translation(),
            });
        }
    }
}
