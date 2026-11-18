//! Frosts: still cold that steals heat, Rime Fell's own. Step into one and
//! the cold gets into you fast, faster towards its heart: it's the body's
//! one chill, the valley's (`world::cold::Chill`), so a frost and the
//! weather add up, you shiver for minutes after (a roof warms it off
//! quickest), and the stamina it caps is the valley's to give back. In the
//! frost the chill stiffens you too (slower, and past a point too stiff to
//! run), and frozen through it bites. You're told as it starts to take
//! you, well before the bite; out of it the stiffness leaves you in a
//! couple of seconds.
//!
//! The way in is a live hearth (a burner's stone): carried, a frost chills
//! you a quarter as fast and never past a light stiffness, and never
//! bites. A Frost grows a **rimestone**: carried live, a burner's coals
//! don't take you, and a little chill never leaves you.

use bevy::prelude::*;
use extraction_core::anomaly::{
    AnomalyKind, FROST_BITE, FROST_SLOW, FROST_THAW, FROST_WARN, RIMESTONE_CHILL, chill_step,
    frost_rate, frost_stiffness,
};

use super::Anomaly;
use super::artifacts::ArtifactEffects;
use crate::health::Health;
use crate::player::{Body, Player};
use crate::world::cold::Chill;

/// Pace lost past which you're too stiff to run: the warning's.
pub const STIFF: f32 = FROST_SLOW * FROST_WARN;

/// Said as a frost starts to take you, and as it bites.
pub const FROST_WARNS: &str = "The cold is in your joints. Get out of it.";
pub const FROST_BITES: &str = "The frost is biting.";

/// A frost chills whoever's in it, stiffens them, and frozen through
/// bites; out of one the stiffness eases off.
#[allow(clippy::type_complexity)]
pub fn chill(
    anomalies: Query<(&Anomaly, &GlobalTransform), Without<super::surge::Forming>>,
    mut players: Query<
        (
            Entity,
            &GlobalTransform,
            &mut Body,
            &mut Health,
            &mut Chill,
            Option<&ArtifactEffects>,
        ),
        With<Player>,
    >,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    mut raid: ResMut<crate::raid::Raid>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, g, mut body, mut h, mut c, fx) in &mut players {
        let at = g.translation();
        let rate = anomalies
            .iter()
            .filter(|(a, _)| a.kind == AnomalyKind::Frost)
            .filter(|(_, f)| (at.y - f.translation().y) < 2.5)
            .map(|(a, f)| frost_rate(at.xz().distance(f.translation().xz()), a.radius))
            .fold(0.0, f32::max);
        let hearth = fx.is_some_and(|fx| fx.hearth);
        let rime = fx.is_some_and(|fx| fx.rimestone);
        let before = c.level;
        c.level = chill_step(before, rate, hearth, rime, dt);
        if rate > 0.0 {
            body.chilled = frost_stiffness(c.level, hearth);
            if locals.contains(e) {
                if before < FROST_WARN && c.level >= FROST_WARN {
                    raid.say(FROST_WARNS);
                }
                if before < 1.0 && c.level >= 1.0 {
                    raid.say(FROST_BITES);
                }
            }
            if c.level >= 1.0 && !hearth && !h.down {
                h.hp -= FROST_BITE * dt;
                h.since_hit = 0.0;
            }
        } else {
            let rest = if rime {
                frost_stiffness(RIMESTONE_CHILL, false)
            } else {
                0.0
            };
            body.chilled = (body.chilled - FROST_THAW * dt).max(rest);
        }
    }
}

/// Ice a frost lays on water in its reach: a flat floor at the water's
/// level, so a frost by a stream or across a reservoir's arm is a crossing
/// (cold to walk: it's in the frost). One disc the frost's size; under dry
/// ground it's buried and nothing knows it's there.
#[derive(Component, Debug, Clone, Copy)]
pub struct Frozen {
    pub ice: Option<Entity>,
    /// Where and how big the frost was when its ice was laid.
    pub at: Vec3,
    pub radius: f32,
}

/// The ice sheet's thickness, m, and how far its top stands over the
/// water (so a foot on it is dry).
pub const ICE_THICK: f32 = 0.15;
pub const ICE_PROUD: f32 = 0.08;
/// Water shallower than this over the ground isn't worth a sheet, m.
const ICE_MIN_DEPTH: f32 = 0.05;
/// How finely the frost's reach is sampled for water, m.
const ICE_STEP: f32 = 0.5;

/// Where the water in a frost's reach lies (centre `c`, radius `r`): its
/// mean level, if any of the disc is water.
pub fn frozen_level(hm: &extraction_core::terrain::Heightmap, c: Vec3, r: f32) -> Option<f32> {
    let n = (r / ICE_STEP).ceil() as i32;
    let (mut sum, mut count) = (0.0f32, 0u32);
    for i in -n..=n {
        for j in -n..=n {
            let d = Vec2::new(i as f32, j as f32) * ICE_STEP;
            if d.length() > r {
                continue;
            }
            let p = [c.x + d.x, c.z + d.y];
            if let Some(w) = hm.water_level(p)
                && w - hm.height(p) > ICE_MIN_DEPTH
            {
                sum += w;
                count += 1;
            }
        }
    }
    (count > 0).then(|| sum / count as f32)
}

/// A cell of ice rising more than this across it (m) is a step: the sheet
/// breaks there.
const ICE_STEP_BREAK: f32 = 0.15;
/// Rime creeps up the banks this far over the ice, m, thinning out.
const RIME_UP: f32 = 1.0;
/// The sheet's mesh: cells this fine, m...
const ICE_CELL: f32 = 0.25;
/// ...its rim this ragged, a share of the radius...
const ICE_RAGGED: f32 = 0.08;
/// ...and its crack texture tiles every so many metres.
const ICE_TILE: f32 = 3.0;

/// The ice a frost lays, as one mesh in the frost's own space (centre `c`,
/// radius `r`, the water's level `w`): over the water a flat sheet, its
/// rim ragged; where it meets the banks, rime lying on the ground and
/// thinning to nothing a little way up them. Vertex colour carries the
/// rime's fade. None if there's no ice to lay.
pub fn ice_mesh(hm: &extraction_core::terrain::Heightmap, c: Vec3, r: f32, w: f32) -> Option<Mesh> {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::Indices;
    use bevy::render::render_resource::PrimitiveTopology;
    let n = ((r * (1.0 + ICE_RAGGED)) / ICE_CELL).ceil() as i32 + 1;
    let side = (2 * n + 1) as usize;
    let top = w + ICE_PROUD;
    // Each lattice point: where (frost space) and how opaque, or none.
    let mut pts: Vec<Option<([f32; 3], f32)>> = Vec::with_capacity(side * side);
    for j in -n..=n {
        for i in -n..=n {
            let d = Vec2::new(i as f32, j as f32) * ICE_CELL;
            let a = d.y.atan2(d.x);
            let rim = r
                * (1.0
                    + ICE_RAGGED
                        * (super::visuals::value_noise(
                            a.cos() * 3.0 + 7.0,
                            a.sin() * 3.0 + 7.0,
                            41,
                        ) * 2.0
                            - 1.0));
            let p = [c.x + d.x, c.z + d.y];
            let g = hm.height(p);
            // On the water where it lies: a beck falls along its run, and a
            // sheet at its mean level sank under it upstream.
            // Off the water, it lies on the ground (pinned to the water's
            // mean level, a dry low bank stood up as a wall against the
            // water upstream).
            let here = hm.water_level(p).filter(|l| *l > g);
            let top = here.map_or(top, |l| l + ICE_PROUD);
            pts.push(if d.length() > rim {
                None
            } else if let Some(l) = here {
                Some(([d.x, (l + ICE_PROUD).max(g + 0.02) - c.y, d.y], 1.0))
            } else if g < top {
                Some(([d.x, g + 0.02 - c.y, d.y], 1.0))
            } else if g < top + RIME_UP {
                let fade = 1.0 - (g - top) / RIME_UP;
                Some(([d.x, g + 0.02 - c.y, d.y], fade))
            } else {
                None
            });
        }
    }
    // The rim: a lattice point at the sheet's edge (a neighbour off it) is
    // moved out onto the ragged rim along its own bearing, so the edge
    // wanders as ice does rather than stepping as the lattice does.
    let rim_at = |d: Vec2| {
        let a = d.y.atan2(d.x);
        r * (1.0
            + ICE_RAGGED
                * (super::visuals::value_noise(a.cos() * 3.0 + 7.0, a.sin() * 3.0 + 7.0, 41) * 2.0
                    - 1.0))
    };
    let edge: Vec<bool> = (0..pts.len())
        .map(|k| {
            let (i, j) = ((k % side) as i32, (k / side) as i32);
            pts[k].is_some_and(|(_, o)| o >= 1.0)
                && [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(di, dj)| {
                    let (a, b) = (i + di, j + dj);
                    a < 0
                        || b < 0
                        || a >= side as i32
                        || b >= side as i32
                        || pts[b as usize * side + a as usize].is_none()
                })
        })
        .collect();
    for k in 0..pts.len() {
        if !edge[k] {
            continue;
        }
        if let Some((q, o)) = pts[k].as_mut().map(|(q, o)| (q, *o)) {
            let d = Vec2::new(q[0], q[2]);
            if d.length() > 1e-3 && o >= 1.0 {
                let to = d.normalize() * rim_at(d);
                // Only outward, and never past where the lattice would
                // have had its next point.
                if to.length() > d.length() && to.length() < d.length() + ICE_CELL * 1.5 {
                    q[0] = to.x;
                    q[2] = to.y;
                }
            }
        }
    }
    let mut positions = Vec::new();
    let mut colours = Vec::new();
    let mut uvs = Vec::new();
    let mut index_of = vec![u32::MAX; pts.len()];
    let mut indices: Vec<u32> = Vec::new();
    let mut any_ice = false;
    for j in 0..side - 1 {
        for i in 0..side - 1 {
            let corners = [
                j * side + i,
                j * side + i + 1,
                (j + 1) * side + i,
                (j + 1) * side + i + 1,
            ];
            if corners.iter().any(|&k| pts[k].is_none()) {
                continue;
            }
            // A cell all rime, thinned to nothing, isn't laid; where the
            // rest gives out is the mask's (its edge follows the texture's
            // grain, not the cells).
            if corners
                .iter()
                .all(|&k| pts[k].is_some_and(|(_, o)| o < 0.02))
            {
                continue;
            }
            // Over a step in the water the sheet breaks into plates, each
            // tilted with its own run, rather than folding up into a wall.
            let ys = corners.map(|k| pts[k].unwrap().0[1]);
            let spread = ys.iter().cloned().fold(f32::MIN, f32::max)
                - ys.iter().cloned().fold(f32::MAX, f32::min);
            if spread > ICE_STEP_BREAK {
                continue;
            }
            any_ice |= corners
                .iter()
                .any(|&k| pts[k].is_some_and(|(_, o)| o >= 1.0));
            let mut ids = [0u32; 4];
            for (m, &k) in corners.iter().enumerate() {
                if index_of[k] == u32::MAX {
                    let (q, o) = pts[k].unwrap();
                    index_of[k] = positions.len() as u32;
                    positions.push(q);
                    // Rime's whiter than the sheet.
                    let b = 1.0 - o;
                    colours.push([1.0, 1.0 + 0.04 * b, 1.0 + 0.06 * b, o]);
                    uvs.push([(c.x + q[0]) / ICE_TILE, (c.z + q[2]) / ICE_TILE]);
                }
                ids[m] = index_of[k];
            }
            indices.extend_from_slice(&[ids[0], ids[2], ids[1], ids[1], ids[2], ids[3]]);
        }
    }
    if !any_ice {
        return None;
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));
    mesh.compute_smooth_normals();
    Some(mesh)
}

/// Frosted ice: white-blue, mottled with trapped air, cut into plates by
/// cracks (cell edges), tiling. Opaque: you see it's solid.
fn ice_texture(images: &mut Assets<Image>) -> Handle<Image> {
    use super::visuals::{smooth, value_noise};
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let n = 256u32;
    // Plates: a few seeds in the tile, wrapped so it repeats.
    let seeds: Vec<Vec2> = (0..14)
        .map(|k| {
            Vec2::new(
                value_noise(k as f32 * 3.7, 1.3, 51),
                value_noise(2.1, k as f32 * 5.3, 52),
            )
        })
        .collect();
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let p = Vec2::new((x as f32 + 0.5) / n as f32, (y as f32 + 0.5) / n as f32);
            // Wandering edges: the point bent by noise before it's placed.
            let bend = Vec2::new(
                value_noise(p.x * 9.0, p.y * 9.0, 53) - 0.5,
                value_noise(p.x * 9.0, p.y * 9.0, 54) - 0.5,
            ) * 0.05;
            let q = p + bend;
            let (mut f1, mut f2) = (f32::MAX, f32::MAX);
            for s in &seeds {
                for ox in -1..=1 {
                    for oy in -1..=1 {
                        let d = (q - (*s + Vec2::new(ox as f32, oy as f32))).length();
                        if d < f1 {
                            f2 = f1;
                            f1 = d;
                        } else if d < f2 {
                            f2 = d;
                        }
                    }
                }
            }
            let crack = 1.0 - smooth(0.0, 0.012, f2 - f1);
            // Hairlines within plates, fainter.
            let hair =
                (1.0 - smooth(
                    0.0,
                    0.05,
                    ((p.x * 31.0 + value_noise(p.x * 6.0, p.y * 6.0, 55) * 4.0).sin()).abs(),
                )) * value_noise(p.x * 4.0, p.y * 4.0, 56)
                    * 0.35;
            let air = value_noise(p.x * 24.0, p.y * 24.0, 57) * 0.5
                + value_noise(p.x * 70.0, p.y * 70.0, 58) * 0.5;
            let base = Vec3::new(0.78, 0.86, 0.94) + Vec3::splat(0.1) * (air - 0.5);
            let line = Vec3::new(0.36, 0.48, 0.62);
            let col = base.lerp(line, crack.max(hair));
            let px = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
            // Alpha is grain for the mask: the sheet (vertex alpha 1) is
            // whole, and rime thinning up a bank (vertex alpha falling)
            // gives out in patches, not along a line.
            let grain = 0.55
                + 0.45
                    * (0.6 * value_noise(p.x * 12.0, p.y * 12.0, 59)
                        + 0.4 * value_noise(p.x * 40.0, p.y * 40.0, 60));
            data.extend_from_slice(&[px(col.x), px(col.y), px(col.z), px(grain)]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    images.add(image)
}

/// Lays (and relays, when a frost creeps or grows) the ice on water in
/// each frost's reach: one mesh, and the collider is that mesh, so the
/// ice ends where you see it end. Its footsteps are hard (glass's).
#[allow(clippy::type_complexity)]
pub fn freeze_water(
    mut commands: Commands,
    frosts: Query<(Entity, &Anomaly, &GlobalTransform, Option<&Frozen>)>,
    zone: Res<crate::world::Zone>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut ice_look: Local<Option<Handle<StandardMaterial>>>,
) {
    use avian3d::prelude::*;
    for (e, a, gt, frozen) in &frosts {
        if a.kind != AnomalyKind::Frost {
            continue;
        }
        let c = gt.translation();
        if frozen.is_some_and(|f| f.at.distance(c) < 0.05 && (f.radius - a.radius).abs() < 0.05) {
            continue;
        }
        if let Some(old) = frozen.and_then(|f| f.ice) {
            commands.entity(old).try_despawn();
        }
        let sheet = frozen_level(&zone.hm, c, a.radius)
            .and_then(|w| ice_mesh(&zone.hm, c, a.radius, w))
            .and_then(|mesh| Some((Collider::trimesh_from_mesh(&mesh)?, mesh)));
        let ice = sheet.map(|(collider, mesh)| {
            let mut sheet = commands.spawn((
                Name::new("Frost ice"),
                RigidBody::Static,
                collider,
                crate::world::surfaces::Surfaces(vec![(
                    Vec3::new(
                        a.radius * (1.0 + ICE_RAGGED),
                        0.2,
                        a.radius * (1.0 + ICE_RAGGED),
                    ),
                    Transform::IDENTITY,
                    crate::render::materials::Mat::Glass,
                )]),
                Transform::IDENTITY,
                Visibility::default(),
            ));
            if let (Some(meshes), Some(materials), Some(images)) =
                (meshes.as_mut(), materials.as_mut(), images.as_mut())
            {
                let look = ice_look
                    .get_or_insert_with(|| {
                        materials.add(StandardMaterial {
                            base_color: Color::WHITE,
                            base_color_texture: Some(ice_texture(images)),
                            perceptual_roughness: 0.55,
                            reflectance: 0.5,
                            // Masked, not blended: solid, so it hides the
                            // water and the frost's mark on the bed under it
                            // (blended, the mark drew over it); the rime's
                            // thinning up the banks is where the mask gives
                            // out, in patches.
                            alpha_mode: AlphaMode::Mask(0.5),
                            ..default()
                        })
                    })
                    .clone();
                sheet.insert((Mesh3d(meshes.add(mesh)), MeshMaterial3d(look)));
            }
            let id = sheet.id();
            commands.entity(e).add_child(id);
            id
        });
        commands.entity(e).try_insert(Frozen {
            ice,
            at: c,
            radius: a.radius,
        });
    }
}

/// A foot on the ice: it creaks as you step on, and now and then as you
/// cross.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct OnIce {
    pub next: f32,
}

/// Seconds between creaks while you cross, and the random spread.
pub const CREAK_EVERY: f32 = 2.2;

/// Creaks underfoot: an `AnomalyAct::Creak` as a walker steps onto a
/// frost's ice and every couple of seconds on it while they move.
#[allow(clippy::type_complexity)]
pub fn creak(
    mut commands: Commands,
    frosts: Query<(Entity, &Anomaly, &GlobalTransform, &Frozen)>,
    mut walkers: Query<(Entity, &GlobalTransform, &Body, Option<&mut OnIce>), With<Player>>,
    zone: Res<crate::world::Zone>,
    time: Res<Time>,
    mut events: MessageWriter<super::AnomalyEvent>,
) {
    let dt = time.delta_secs();
    for (who, gt, body, on) in &mut walkers {
        let feet = gt.translation();
        let p = [feet.x, feet.z];
        let ice = frosts.iter().find(|(_, a, g, f)| {
            f.ice.is_some()
                && Vec2::new(feet.x - g.translation().x, feet.z - g.translation().z).length()
                    < a.radius
                && zone
                    .hm
                    .water_level(p)
                    .is_some_and(|w| w > zone.hm.height(p) && (feet.y - w).abs() < 0.3)
        });
        let Some((frost, ..)) = ice else {
            if on.is_some() {
                commands.entity(who).remove::<OnIce>();
            }
            continue;
        };
        let moving = Vec2::new(body.velocity.x, body.velocity.z).length() > 0.5;
        let mut say = || {
            events.write(super::AnomalyEvent {
                anomaly: frost,
                kind: AnomalyKind::Frost,
                at: feet,
                what: super::AnomalyAct::Creak,
            });
        };
        match on {
            None => {
                say();
                let jitter = (time.elapsed_secs() * 7.13).fract();
                commands.entity(who).insert(OnIce {
                    next: CREAK_EVERY * (0.7 + 0.6 * jitter),
                });
            }
            Some(mut o) => {
                if moving {
                    o.next -= dt;
                }
                if o.next <= 0.0 {
                    say();
                    let jitter = (time.elapsed_secs() * 7.13).fract();
                    o.next = CREAK_EVERY * (0.7 + 0.6 * jitter);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::body::{Backpack, InBackpack};
    use avian3d::prelude::{ColliderDisabled, RigidBodyDisabled};

    /// A player standing at a frost's heart (one put down for the test).
    fn app() -> (App, Entity) {
        let mut app = crate::headless_app(Default::default(), false);
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
        let me = app.world().get::<GlobalTransform>(p).unwrap().translation();
        let site = app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(me.x, me.z);
        super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Frost, site, 3.0, None);
        (app, p)
    }

    fn carry(app: &mut App, p: Entity, id: &str) -> Entity {
        let pack = app
            .world_mut()
            .query::<(Entity, &Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == p && b.carried)
            .map(|(e, _)| e)
            .unwrap();
        let e = crate::anomaly::surge::grow(app.world_mut(), id, Vec3::ZERO).unwrap();
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

    fn said(app: &App, line: &str) -> bool {
        app.world()
            .resource::<crate::raid::Raid>()
            .log
            .iter()
            .any(|(t, _)| t == line)
    }

    fn hp(app: &App, p: Entity) -> f32 {
        app.world().get::<Health>(p).unwrap().hp
    }

    #[test]
    fn a_frost_warns_stiffens_then_bites() {
        let (mut app, p) = app();
        let full = hp(&app, p);
        // A moment in: stiffening, not yet told, not yet bitten.
        for _ in 0..20 {
            app.update();
        }
        let stiff = app.world().get::<Body>(p).unwrap().chilled;
        assert!(stiff > 0.05, "stiffening ({stiff:.2})");
        let cold = app.world().get::<Chill>(p).unwrap().level;
        assert!(cold > 0.1, "the cold is in you ({cold:.2})");
        assert!(!said(&app, FROST_WARNS));
        assert_eq!(hp(&app, p), full, "no bite yet");
        // Told before the bite.
        for _ in 0..40 {
            app.update();
        }
        assert!(said(&app, FROST_WARNS), "told as it takes you");
        assert_eq!(hp(&app, p), full, "and still no bite");
        for _ in 0..90 * 3 {
            app.update();
        }
        assert!(said(&app, FROST_BITES));
        assert!(hp(&app, p) < full - 5.0, "bitten ({:.0})", hp(&app, p));
        assert!(app.world().get::<Body>(p).unwrap().chilled > 0.65);
        // Out of it: loose again in seconds, but still frozen through,
        // for the valley to warm off.
        let me = app.world().get::<GlobalTransform>(p).unwrap().translation();
        let out = app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(me.x + 6.0, me.z);
        app.world_mut().get_mut::<Transform>(p).unwrap().translation = out;
        for _ in 0..90 * 4 {
            app.update();
        }
        assert_eq!(app.world().get::<Body>(p).unwrap().chilled, 0.0);
        assert!(app.world().get::<Chill>(p).unwrap().level > 0.9);
        let after = hp(&app, p);
        for _ in 0..90 {
            app.update();
        }
        assert!(hp(&app, p) >= after, "and no longer bitten");
    }

    #[test]
    fn a_hearth_keeps_the_frost_off() {
        let (mut app, p) = app();
        carry(&mut app, p, "art_hearth");
        let full = hp(&app, p);
        for _ in 0..90 * 20 {
            app.update();
        }
        assert_eq!(hp(&app, p), full, "never bitten");
        let stiff = app.world().get::<Body>(p).unwrap().chilled;
        assert!(stiff < STIFF, "never too stiff to run ({stiff:.2})");
        assert!(!said(&app, FROST_WARNS));
    }

    #[test]
    fn a_rimestone_walks_you_through_coals_but_keeps_you_stiff() {
        let mut app = crate::headless_app(Default::default(), false);
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
        let me = app.world().get::<GlobalTransform>(p).unwrap().translation();
        let site = app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(me.x, me.z);
        carry(&mut app, p, "art_rimestone");
        for _ in 0..90 * 2 {
            app.update();
        }
        let stiff = app.world().get::<Body>(p).unwrap().chilled;
        assert!(stiff > 0.05 && stiff < STIFF, "a little stiff ({stiff:.2})");
        super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Burner, site, 2.0, None);
        let full = hp(&app, p);
        for _ in 0..90 * 3 {
            app.update();
        }
        assert_eq!(hp(&app, p), full, "the coals don't take");
    }

    /// A frost over the canal freezes it: you stand on the ice, dry, where
    /// without it you'd be swimming.
    #[test]
    fn a_frost_freezes_water_walkable() {
        let opts = crate::LaunchOptions {
            robots: false,
            map: "cinder_works".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let p = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let (at, w) = (25..60)
            .filter_map(|z| {
                let q = [-70.0, z as f32];
                zone.hm
                    .water_level(q)
                    .map(|w| (q, w, w - zone.hm.height(q)))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(q, w, _)| (q, w))
            .expect("the canal");
        let frost = super::super::spawn_anomaly(
            app.world_mut(),
            AnomalyKind::Frost,
            Vec3::new(at[0], zone.hm.height(at), at[1]),
            3.0,
            None,
        );
        app.update();
        app.update();
        let ice = app.world().get::<Frozen>(frost).and_then(|f| f.ice);
        assert!(ice.is_some(), "ice on the canal");
        app.world_mut().get_mut::<Transform>(p).unwrap().translation =
            Vec3::new(at[0] + 1.0, w + 0.4, at[1]);
        for _ in 0..90 {
            app.update();
        }
        let feet = app.world().get::<Transform>(p).unwrap().translation;
        let body = app.world().get::<Body>(p).unwrap();
        assert!(
            !body.swimming && body.wading < 0.01,
            "on the ice, dry (wading {:.2}, swimming {})",
            body.wading,
            body.swimming
        );
        assert!(
            (feet.y - (w + ICE_PROUD)).abs() < 0.1,
            "standing on the ice: feet {:.2}, water {w:.2}",
            feet.y
        );
        assert!(
            app.world().get::<OnIce>(p).is_some(),
            "it creaked as you stepped on"
        );
        // Gone with its frost.
        app.world_mut().entity_mut(frost).despawn();
        app.update();
        assert!(app.world().get_entity(ice.unwrap()).is_err(), "it melts");
    }

    /// Frosts on dry ground lay no ice.
    #[test]
    fn a_frost_on_dry_ground_lays_no_ice() {
        let (mut app, _) = app();
        app.update();
        app.update();
        let f = app
            .world_mut()
            .query::<&Frozen>()
            .iter(app.world())
            .next()
            .copied();
        assert!(f.is_some_and(|f| f.ice.is_none()), "no water, no ice");
    }

    /// A frost standing on Kettle Beck (where Map design puts it) freezes
    /// it right across: its reach has dry ground on two opposite sides,
    /// water between, and ice on the water.
    #[test]
    fn a_frost_on_the_beck_spans_it() {
        let opts = crate::LaunchOptions {
            robots: false,
            map: "kettle_pass".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        // The beck's narrowest crossing there (about 4.75 m bank to bank),
        // its middle.
        let (c, r) = (Vec2::new(-144.4, 12.5), 4.0);
        assert!(
            zone.hm.water_level([c.x, c.y]).is_some(),
            "its centre's on the water"
        );
        assert!(
            extraction_core::anomaly::placeable_for(
                &zone.map,
                &zone.hm,
                [c.x, c.y],
                AnomalyKind::Frost
            ),
            "a frost may stand there"
        );
        let dry = |p: Vec2| zone.hm.water_level([p.x, p.y]).is_none();
        let spans = (0..36).any(|k| {
            let a = k as f32 / 36.0 * std::f32::consts::TAU;
            let d = Vec2::new(a.cos(), a.sin());
            let side = |sign: f32| (1..=8).any(|i| dry(c + d * sign * r * 0.9 * i as f32 / 8.0));
            side(1.0) && side(-1.0)
        });
        assert!(
            spans,
            "dry ground within its reach on both sides of the beck"
        );
        let at = zone.on_ground(c.x, c.y);
        let frost = super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Frost, at, r, None);
        app.update();
        app.update();
        assert!(
            app.world()
                .get::<Frozen>(frost)
                .and_then(|f| f.ice)
                .is_some(),
            "the beck's iced over"
        );
    }
}
