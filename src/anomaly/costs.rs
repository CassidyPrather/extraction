//! What carrying an artifact live costs you, besides the Zone's attention.
//! Nothing here shows a number: you feel it, see it or hear it. Sealed in
//! a lead-lined case ([`Shielded`]) an artifact costs nothing, and does
//! nothing for you either.
//!
//! - **Featherstone**: it wants home. Lifts near you wind up and pop.
//! - **Hearth**: you run hot. Your wind comes back at half the pace.
//! - **Aegis knot**: it holds a charge, and wet (soaked ground, wading) it
//!   bites you every few seconds, with a crack anything near can hear.
//! - **Lodestone**: heavy, and heavier the nearer you hold it: five
//!   kilograms more on your load (`lift_kg: -5` in the catalogue).
//! - **Mirror shard**: your reflection runs a moment late. Every half
//!   minute or so it glints, and whatever's near sees where you are.

use bevy::prelude::*;
use extraction_core::anomaly::{
    AEGIS_SHOCK, AnomalyKind, HEARTH_TIRE, SHARD_GLINT_RADIUS, aegis_shock_in, aegis_wet,
    feather_wake, shard_glint_in,
};
use extraction_core::noise::Rng;

use super::Anomaly;
use super::artifacts::{CarriedBy, Shielded};
use crate::health::Health;
use crate::items::{Catalog, Item};
use crate::player::{Body, Player};
use crate::robots::Noise;

/// An artifact made itself felt (for sound, haptics and a glint).
#[derive(Message, Clone, Copy, Debug)]
pub struct ArtifactBite {
    pub player: Entity,
    pub at: Vec3,
    pub what: Bite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bite {
    /// A wet aegis shocked its carrier.
    Shock,
    /// A shard's late reflection glinted.
    Glint,
}

/// Per player: seconds to the next shock and glint, and the player's own
/// dice for them (how many others carry stones never changes yours).
#[derive(Component, Debug)]
pub struct Bites {
    shock: f32,
    glint: f32,
    dice: Rng,
    /// Seconds since the aegis last shocked you, and whether you've been
    /// told it's killing you this wet spell.
    since_shock: f32,
    told_dying: bool,
}

impl Bites {
    fn new() -> Self {
        let mut dice = Rng::new(0xB17E);
        Self {
            shock: aegis_shock_in(dice.f32()),
            glint: shard_glint_in(dice.f32()),
            dice,
            since_shock: f32::INFINITY,
            told_dying: false,
        }
    }
}

/// Said at a wet aegis's first shock (after a minute without one), and
/// once if its shocks have you low: it never kills you without a word.
pub const AEGIS_WET: &str =
    "The aegis knot bites: wet, it holds a charge. Get dry, or seal it in lead.";
pub const AEGIS_DYING: &str = "The aegis is killing you. Drop it, or get out of the wet.";

/// What each player carries live, by catalogue id.
fn live<'a>(
    carried: &'a Query<(&Item, &CarriedBy), Without<Shielded>>,
    catalog: &'a Catalog,
    id: &'a str,
) -> impl Iterator<Item = Entity> + 'a {
    carried
        .iter()
        .filter(move |(i, _)| catalog.get(i.kind()).id == id)
        .map(|(_, c)| c.0)
}

/// Featherstones wake the lifts they pass.
pub fn feathers_wake_lifts(
    carried: Query<(&Item, &CarriedBy), Without<Shielded>>,
    players: Query<&GlobalTransform, With<Player>>,
    mut lifts: Query<(&mut Anomaly, &GlobalTransform)>,
    catalog: Res<Catalog>,
    time: Res<Time>,
) {
    let carriers: Vec<Vec3> = live(&carried, &catalog, "art_featherstone")
        .filter_map(|p| players.get(p).ok().map(|g| g.translation()))
        .collect();
    if carriers.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    for (mut a, gt) in &mut lifts {
        if a.kind != AnomalyKind::Lift {
            continue;
        }
        let wake: f32 = carriers
            .iter()
            .map(|p| feather_wake(p.distance(gt.translation())))
            .sum();
        if wake > 0.0 {
            a.charge = (a.charge + wake * dt).min(1.0);
        }
    }
}

/// The hearth, the aegis and the shard, on whoever carries them.
#[allow(clippy::too_many_arguments)]
pub fn bites(
    mut commands: Commands,
    carried: Query<(&Item, &CarriedBy), Without<Shielded>>,
    mut players: Query<
        (
            Entity,
            &GlobalTransform,
            &mut Body,
            &mut Health,
            Option<&mut Bites>,
        ),
        With<Player>,
    >,
    catalog: Res<Catalog>,
    wetness: Option<Res<crate::world::weather::Wetness>>,
    time: Res<Time>,
    mut noises: MessageWriter<Noise>,
    mut out: MessageWriter<ArtifactBite>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    let dt = time.delta_secs();
    let wet = wetness.map_or(0.0, |w| w.level);
    let hearth: Vec<Entity> = live(&carried, &catalog, "art_hearth").collect();
    let aegis: Vec<Entity> = live(&carried, &catalog, "art_aegis").collect();
    let shard: Vec<Entity> = live(&carried, &catalog, "art_shard").collect();
    for (p, gt, mut body, mut health, bites) in &mut players {
        let Some(mut b) = bites else {
            commands.entity(p).insert(Bites::new());
            continue;
        };
        let at = gt.translation() + Vec3::Y * 1.2;
        if hearth.contains(&p) {
            body.stamina = (body.stamina - HEARTH_TIRE * dt).max(0.0);
        }
        b.since_shock += dt;
        if aegis.contains(&p) && aegis_wet(wet, body.wading) {
            b.shock -= dt;
            if b.shock <= 0.0 {
                b.shock = aegis_shock_in(b.dice.f32());
                if !health.down {
                    health.hp -= AEGIS_SHOCK;
                    health.since_hit = 0.0;
                }
                let me = locals.contains(p);
                if b.since_shock > 60.0 {
                    b.told_dying = false;
                    if me {
                        raid.say(AEGIS_WET);
                    }
                }
                if health.hp < 30.0 && !b.told_dying {
                    b.told_dying = true;
                    if me {
                        raid.say(AEGIS_DYING);
                    }
                }
                b.since_shock = 0.0;
                noises.write(Noise {
                    at,
                    radius: 25.0,
                    source: None,
                });
                out.write(ArtifactBite {
                    player: p,
                    at,
                    what: Bite::Shock,
                });
            }
        }
        if shard.contains(&p) {
            b.glint -= dt;
            if b.glint <= 0.0 {
                b.glint = shard_glint_in(b.dice.f32());
                noises.write(Noise {
                    at,
                    radius: SHARD_GLINT_RADIUS,
                    source: None,
                });
                out.write(ArtifactBite {
                    player: p,
                    at,
                    what: Bite::Glint,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::body::{Backpack, InBackpack};
    use crate::launch::LaunchOptions;
    use avian3d::prelude::{ColliderDisabled, RigidBodyDisabled};

    fn app() -> (App, Entity) {
        let mut app = crate::headless_app(
            LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let p = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
        (app, p)
    }

    fn spawn(app: &mut App, id: &str, at: Vec3) -> Entity {
        let catalog = app.world().resource::<Catalog>().clone();
        let kind = catalog.kind(id).unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let e = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(at),
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        e
    }

    /// Puts `id` in the player's worn pack; returns it.
    fn carry(app: &mut App, p: Entity, id: &str) -> Entity {
        let pack = app
            .world_mut()
            .query::<(Entity, &Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == p && b.carried)
            .map(|(e, _)| e)
            .unwrap();
        let e = spawn(app, id, Vec3::ZERO);
        app.world_mut()
            .get_mut::<Backpack>(pack)
            .unwrap()
            .contents
            .push(e);
        app.world_mut().entity_mut(e).insert((
            InBackpack { bag: pack },
            ChildOf(pack),
            RigidBodyDisabled,
            ColliderDisabled,
        ));
        e
    }

    fn soak(app: &mut App, level: f32) {
        if let Some(mut w) = app
            .world_mut()
            .get_resource_mut::<crate::world::weather::Wetness>()
        {
            w.level = level;
        }
    }

    fn shocks(app: &mut App, p: Entity, secs: usize) -> usize {
        let mut cursor = app
            .world()
            .resource::<Messages<ArtifactBite>>()
            .get_cursor_current();
        let mut n = 0;
        for _ in 0..secs * 90 {
            app.update();
            n += cursor
                .read(app.world().resource::<Messages<ArtifactBite>>())
                .filter(|b| b.player == p && b.what == Bite::Shock)
                .count();
        }
        n
    }

    #[test]
    fn a_wet_aegis_bites_and_a_dry_or_sealed_one_doesnt() {
        let (mut app, p) = app();
        carry(&mut app, p, "art_aegis");
        soak(&mut app, 0.0);
        assert_eq!(shocks(&mut app, p, 20), 0, "dry, it keeps its charge");
        soak(&mut app, 1.0);
        let hp = app.world().get::<Health>(p).unwrap().hp;
        // A second at a time, looking for the word (the raid's log lines
        // fade after a few seconds).
        let (mut n, mut said) = (0, false);
        for _ in 0..32 {
            n += shocks(&mut app, p, 1);
            said |= app
                .world()
                .resource::<crate::raid::Raid>()
                .log
                .iter()
                .any(|(t, _)| t == AEGIS_WET);
        }
        assert!(n >= 2, "soaked, it bites every few seconds ({n} in 32 s)");
        assert!(app.world().get::<Health>(p).unwrap().hp < hp);
        assert!(said, "and its first bite says why");

        // Sealed in a lead case, it's quiet even soaked.
        let (mut app, p) = self::app();
        let case = carry(&mut app, p, "lead_case");
        let knot = spawn(&mut app, "art_aegis", Vec3::ZERO);
        app.update();
        crate::persist::seat_in_mount(app.world_mut(), case, knot);
        soak(&mut app, 1.0);
        assert_eq!(shocks(&mut app, p, 32), 0, "sealed, nothing");
    }

    #[test]
    fn a_featherstone_wakes_the_lifts_it_passes() {
        let (mut app, p) = app();
        let lift = app
            .world_mut()
            .query::<(Entity, &Anomaly, &GlobalTransform)>()
            .iter(app.world())
            .find(|(_, a, _)| a.kind == AnomalyKind::Lift && a.bloom.is_none())
            .map(|(e, a, g)| (e, g.translation(), a.radius))
            .unwrap();
        // Stood off its edge, out of its reach.
        let beside = {
            let x = lift.1 + Vec3::new(lift.2 + 2.5, 0.0, 0.0);
            app.world()
                .resource::<crate::world::Zone>()
                .on_ground(x.x, x.z)
        };
        app.world_mut().get_mut::<Transform>(p).unwrap().translation = beside;
        for _ in 0..90 * 3 {
            app.update();
        }
        let quiet = app.world().get::<Anomaly>(lift.0).unwrap().charge;
        assert!(quiet < 0.05, "empty-handed, it stays asleep ({quiet})");
        carry(&mut app, p, "art_featherstone");
        let mut cursor = app
            .world()
            .resource::<Messages<super::super::AnomalyEvent>>()
            .get_cursor_current();
        let mut popped = false;
        for _ in 0..90 * 10 {
            app.update();
            popped |= cursor
                .read(
                    app.world()
                        .resource::<Messages<super::super::AnomalyEvent>>(),
                )
                .any(|e| e.anomaly == lift.0 && e.what == super::super::AnomalyAct::Pop);
        }
        assert!(popped, "carrying the stone past it winds it up to pop");
    }

    #[test]
    fn a_lodestone_weighs_on_you_and_a_hearth_runs_you_hot() {
        let (mut app, p) = app();
        let before = app.world().get::<Body>(p).unwrap().carried_kg;
        carry(&mut app, p, "art_lodestone");
        for _ in 0..5 {
            app.update();
        }
        let after = app.world().get::<Body>(p).unwrap().carried_kg;
        assert!(
            after - before > 6.0,
            "1.6 kg of stone weighs like 6.6 ({before:.1} -> {after:.1})"
        );
        // Winded, then rested for four seconds: half the recovery.
        let rest = |app: &mut App| {
            app.world_mut().get_mut::<Body>(p).unwrap().stamina = 0.2;
            for _ in 0..90 * 4 {
                app.update();
            }
            app.world().get::<Body>(p).unwrap().stamina
        };
        let cool = rest(&mut app);
        carry(&mut app, p, "art_hearth");
        for _ in 0..3 {
            app.update();
        }
        let hot = rest(&mut app);
        assert!(
            hot < cool - 0.1,
            "hot, you get your wind back slower ({cool:.2} vs {hot:.2})"
        );
        assert!(hot > 0.2, "but you do get it back ({hot:.2})");
    }

    #[test]
    fn a_shard_glints_and_gives_you_away() {
        let (mut app, p) = app();
        carry(&mut app, p, "art_shard");
        let mut cursor = app
            .world()
            .resource::<Messages<ArtifactBite>>()
            .get_cursor_current();
        let mut glints = 0;
        for _ in 0..90 * 45 {
            app.update();
            glints += cursor
                .read(app.world().resource::<Messages<ArtifactBite>>())
                .filter(|b| b.player == p && b.what == Bite::Glint)
                .count();
        }
        assert!(
            (1..=3).contains(&glints),
            "a glint or two in 45 s ({glints})"
        );
    }
}
