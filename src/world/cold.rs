//! Cold in a high valley (`extraction_core::cold`, `docs/FIFTH_VALLEY.md`).
//!
//! Every player carries a [`Chill`]. Out in the open up high it rises, under
//! a roof it falls, and a chilled body shivers: its stamina is capped (so
//! sprints are short and, through the guns' tired sway, the aim shakes) and
//! now and then the hands shiver. In the client your breath shows in the
//! cold air. A valley without `cold` leaves everyone warm.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::cold::{self, Air, Weather};
use extraction_core::map::MarkerKind;

use super::Zone;
use super::weather::{Conditions, WeatherKind};
use crate::player::{Body, Player};
use crate::rig::LocalPlayer;

/// How often the roof over each head is looked for, seconds.
const PROBE_EVERY: f32 = 0.25;
/// A roof counts this far over the head, metres.
const ROOF_REACH: f32 = 12.0;
/// The head over the feet, metres (standing; near enough crouched).
const HEAD: f32 = 1.5;

pub struct ColdPlugin;

impl Plugin for ColdPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (give_chill, chill, shiver)
                .chain()
                .after(crate::player::move_bodies),
        );
    }
}

/// How chilled a body is (`extraction_core::cold`), and what the air is
/// doing to it.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Chill {
    /// 0 warm ..= 1 frozen through.
    pub level: f32,
    /// A roof overhead, as last looked.
    pub sheltered: bool,
    /// How hard the air bites here (the valley's cold, the height, the
    /// weather), 0 where it doesn't.
    pub bite: f32,
    probe: f32,
    tremor: f32,
}

impl Chill {
    pub fn shiver(&self) -> f32 {
        cold::shiver(self.level)
    }
}

fn give_chill(mut commands: Commands, new: Query<Entity, (With<Player>, Without<Chill>)>) {
    for e in &new {
        commands.entity(e).insert(Chill::default());
    }
}

/// The weather as the cold sees it.
fn weather_of(c: &Conditions) -> Weather {
    match c.kind {
        WeatherKind::Clearing => Weather::Clear,
        WeatherKind::Drizzle => Weather::Ordinary,
        WeatherKind::PeaSouper => Weather::Fog,
        WeatherKind::Downpour => Weather::Rain,
    }
}

/// The valley's floor for exposure: the insertion's ground.
fn floor(zone: &Zone) -> f32 {
    zone.map
        .markers_of(&MarkerKind::Insertion)
        .next()
        .map_or(zone.map.base_height, |m| zone.ground(m.at[0], m.at[1]))
}

fn chill(
    zone: Res<Zone>,
    conditions: Option<Res<Conditions>>,
    time: Res<Time>,
    spatial: SpatialQuery,
    mut bodies: Query<(Entity, &GlobalTransform, &mut Chill), With<Player>>,
    mut low: Local<Option<f32>>,
) {
    let dt = time.delta_secs();
    let valley = zone.map.cold;
    let low = *low.get_or_insert_with(|| floor(&zone));
    let (weather, night) = conditions.map_or((Weather::Ordinary, false), |c| {
        (weather_of(&c), c.light < 0.25)
    });
    let filter = SpatialQueryFilter::from_mask([super::Layer::World]);
    for (e, gt, mut c) in &mut bodies {
        let head = gt.translation() + Vec3::Y * HEAD;
        c.probe -= dt;
        if c.probe <= 0.0 {
            c.probe = PROBE_EVERY;
            c.sheltered = valley > 0.0
                && spatial
                    .cast_ray_predicate(head, Dir3::Y, ROOF_REACH, true, &filter, &|h| h != e)
                    .is_some();
        }
        let air = Air {
            cold: valley,
            height: head.y - HEAD - low,
            weather,
            night,
            sheltered: c.sheltered,
        };
        c.bite = if c.sheltered {
            0.0
        } else {
            valley * cold::exposure(air.height) * weather.factor()
        };
        c.level = cold::step(c.level, &air, dt);
    }
}

/// A shivering body: its stamina capped, and a tremor in the hands now and
/// then.
fn shiver(
    time: Res<Time>,
    mut bodies: Query<(&mut Body, &mut Chill, Has<LocalPlayer>)>,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
) {
    let dt = time.delta_secs();
    for (mut body, mut c, local) in &mut bodies {
        body.stamina = body.stamina.min(cold::stamina_cap(c.level));
        let s = c.shiver();
        if s <= 0.0 {
            c.tremor = 0.0;
            continue;
        }
        // A fit of shivering every few seconds, closer together the colder.
        c.tremor -= dt;
        if c.tremor <= 0.0 {
            c.tremor = 6.0 - 4.0 * s;
            if local {
                for side in crate::rig::Side::BOTH {
                    haptics.write(crate::rig::haptics::Haptic {
                        side,
                        amplitude: 0.08 + 0.12 * s,
                        seconds: 0.18 + 0.2 * s,
                    });
                }
            }
        }
    }
}

// --- The client: breath in the cold air ------------------------------------

/// Breath shows when the air bites at least this much.
const BREATH_FROM: f32 = 0.05;
/// A breath's life, seconds.
const BREATH_LIFE: f32 = 1.6;

pub struct ColdVisualsPlugin;

impl Plugin for ColdVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (breathe, drift_breath).chain());
    }
}

#[derive(Component)]
struct Breath {
    age: f32,
    drift: Vec3,
}

#[derive(Default)]
struct BreathAssets {
    mesh: Handle<Mesh>,
    clock: f32,
}

fn breathe(
    mut commands: Commands,
    time: Res<Time>,
    zone: Res<Zone>,
    player: Query<(&Chill, &Body), With<LocalPlayer>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut assets: Local<Option<BreathAssets>>,
) {
    if zone.map.cold <= 0.0 {
        return;
    }
    let Ok((chill, body)) = player.single() else {
        return;
    };
    let Some(cam) = cams.iter().next() else {
        return;
    };
    // Cold air shows a breath even indoors (unheated); out of it, only
    // where the air bites.
    let air = chill
        .bite
        .max(0.06 * zone.map.cold * f32::from(chill.sheltered));
    if air < BREATH_FROM {
        return;
    }
    let a = assets.get_or_insert_with(|| BreathAssets {
        mesh: meshes.add(Sphere::new(0.06).mesh().ico(2).unwrap()),
        clock: 0.0,
    });
    // Out of breath you breathe faster.
    let pace = 3.6 - 1.6 * (1.0 - body.stamina).clamp(0.0, 1.0);
    a.clock += time.delta_secs();
    if a.clock < pace {
        return;
    }
    a.clock = 0.0;
    let fwd = cam.forward().as_vec3();
    let at = cam.translation() + fwd * 0.28 - Vec3::Y * 0.16;
    let alpha = (0.05 + 0.12 * air.min(1.0)).min(0.16);
    commands.spawn((
        Name::new("Breath"),
        Breath {
            age: 0.0,
            drift: fwd * 0.35 + Vec3::Y * 0.12,
        },
        Mesh3d(a.mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.92, 0.94, 0.97, alpha),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_translation(at).with_scale(Vec3::splat(0.6)),
        bevy::light::NotShadowCaster,
        bevy::light::NotShadowReceiver,
    ));
}

fn drift_breath(
    mut commands: Commands,
    time: Res<Time>,
    mut puffs: Query<(
        Entity,
        &mut Breath,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    for (e, mut b, mut t, mat) in &mut puffs {
        b.age += dt;
        if b.age >= BREATH_LIFE {
            materials.remove(&mat.0);
            commands.entity(e).despawn();
            continue;
        }
        let k = b.age / BREATH_LIFE;
        t.translation += b.drift * dt * (1.0 - k);
        t.scale = Vec3::splat(0.6 + 2.4 * k);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let a = m.base_color.alpha();
            m.base_color
                .set_alpha(a * (1.0 - dt / (BREATH_LIFE - b.age + dt)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vr_harness::Vr;

    fn chill(vr: &Vr) -> Chill {
        *vr.app.world().get::<Chill>(vr.player).unwrap()
    }

    /// Standing on Rime Fell's col the air bites and the chill climbs;
    /// frozen through, the body's stamina is capped; inside the col hut
    /// there's a roof overhead and it falls again.
    #[test]
    fn the_col_chills_and_a_roof_warms() {
        let mut vr = Vr::new("rime_fell");
        let ground = |vr: &Vr, x: f32, z: f32| vr.app.world().resource::<Zone>().ground(x, z);
        let col = Vec3::new(70.0, ground(&vr, 70.0, -262.0) + 0.05, -262.0);
        vr.stand(col, 0.0);
        vr.step(90 * 20);
        let c = chill(&vr);
        assert!(!c.sheltered, "nothing over the col");
        assert!(c.bite > 0.7, "the col is exposed: {c:?}");
        assert!(c.level > 0.04, "twenty seconds on the col: {c:?}");

        // Frozen through: no more stamina than a shivering body has.
        vr.app
            .world_mut()
            .get_mut::<Chill>(vr.player)
            .unwrap()
            .level = 1.0;
        vr.step(5);
        let body = vr.app.world().get::<Body>(vr.player).unwrap();
        assert!(
            body.stamina <= cold::stamina_cap(0.99) + 1e-3,
            "{}",
            body.stamina
        );

        // The col hut (a mast_hut at (20, -245)): under its roof.
        let hut = Vec3::new(20.0, ground(&vr, 20.0, -245.0) + 0.3, -245.0);
        vr.stand(hut, 0.0);
        vr.step(90 * 2);
        let warm = chill(&vr);
        assert!(warm.sheltered, "the hut's roof is overhead: {warm:?}");
        assert!(warm.level < 0.97, "warming under the roof: {warm:?}");
    }

    /// The other valleys are never cold.
    #[test]
    fn marrow_creek_is_never_cold() {
        let mut vr = Vr::new("marrow_creek");
        vr.step(90 * 5);
        let c = chill(&vr);
        assert_eq!(c.level, 0.0);
        assert_eq!(c.bite, 0.0);
    }
}
