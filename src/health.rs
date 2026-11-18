//! Player health: hits and hard landings take HP, an injector gives it
//! back, and going down ends the raid. Nothing bleeds: a hit's harm is
//! all at once.

use bevy::prelude::*;

use crate::player::{BodyCollider, Player};
use crate::weapons::ballistics::Hit;

#[derive(Component, Debug)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
    pub down: bool,
    /// Seconds since the last hit (for screen and audio effects).
    pub since_hit: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            hp: 100.0,
            max: 100.0,
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
    /// Who did it, when anyone did (a machine, another player, the player
    /// themselves): what a hit's warning is owed by.
    pub by: Option<Entity>,
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
            .add_systems(
                FixedUpdate,
                (take_hits, falls, go_down).chain().in_set(HealthSystems),
            );
    }
}

/// Hits, falls and going down, each fixed tick.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct HealthSystems;

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
    mut armour: crate::weapons::armour::Armour,
) {
    for hit in hits.read() {
        let Ok(parent) = colliders.get(hit.entity) else {
            continue;
        };
        let player = parent.parent();
        let Ok((mut h, gt, fx)) = players.get_mut(player) else {
            continue;
        };
        // A round that carries no harm (a drone's warning burst kicking up
        // the ground short of you) isn't a hit.
        if h.down || hit.damage <= 0.0 {
            continue;
        }
        // Whatever armour it met takes its share first.
        let mut damage = armour.filter(player, hit);
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
        hurt.write(Hurt {
            player,
            amount: damage,
            from: hit.point - hit.velocity.normalize_or_zero() * 5.0,
            by: hit.shooter,
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
        }
    }
}

/// Landing speed (m/s) you walk away from.
pub const SAFE_LANDING: f32 = 8.0;

/// The clock since the last hit, and down when anything (a hit, a fall,
/// an anomaly, the Surge) has taken the last of it.
fn go_down(
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
        if h.hp <= 0.0 {
            h.down = true;
            downed.write(Downed {
                player: e,
                at: gt.translation(),
            });
        }
    }
}
