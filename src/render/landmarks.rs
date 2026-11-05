//! Lamps on the landmarks. Everything tall enough to steer by
//! (`map_lint::LANDMARK_HEIGHT`) carries a steady red lamp at its highest
//! point, as masts and chimneys do: the wayfinding gauge
//! (`render::wayfind_shots`) found the valley's landmarks gone in drizzle
//! past about 200 m and all but two gone at night, so a rendezvous had
//! nothing to steer by. A lamp a few pixels across outshines the fog and
//! the dark where the structure's own few percent of contrast doesn't.
//! Steady and red: the drones' beacons are amber and blink.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use extraction_core::map::prefab_footprint;
use extraction_core::map_lint::LANDMARK_HEIGHT;

use crate::world::buildings::{Building, Shape, prefab};
use crate::world::halos::{Halo, HaloReach};

pub struct LandmarkLampsPlugin;

impl Plugin for LandmarkLampsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (light_landmarks, day_glows, flash_lamps));
    }
}

/// Prefabs whose models carry a lamp bright enough to steer by (the radio
/// mast's own is too small to carry past 150 m at night: it gets one too).
const LIT: &[&str] = &[];

/// The lamp's radius, and its halo's (m): a soft `world::halos::Halo`,
/// fogged like every lamp's and shown after dark; a hard-edged sphere
/// read as a marker stuck on the sky.
const LAMP_R: f32 = 0.35;
const HALO_R: f32 = 1.0;

/// The halo's colour (linear; a lantern's core is 1) and how far off it
/// still shows (m): the gauge shoots landmarks out to 320 m. The Halo
/// shows only after dark; by day a soft glow facing the eye, fading to
/// nothing at [`DAY_R`], keeps the lamp more than a few pixels at 200 m
/// (the hunting lodge's only landmark in drizzle is the mast's lamp at
/// 170 m).
const HALO: LinearRgba = LinearRgba::rgb(1.6, 0.1, 0.03);
const HALO_REACH: f32 = 320.0;
const DAY_R: f32 = 1.4;

/// A lamp's character (`map_landmarks::assign`): when in its period
/// it's lit. Its halo and glow, its children, go out with it.
#[derive(Component, Clone, Copy)]
pub(crate) struct Lamp(pub extraction_core::map_landmarks::Character);

/// Every lamp lit or dark by its character on the game's clock; with the
/// clock stopped (a frozen capture), lit.
fn flash_lamps(time: Res<Time<Virtual>>, mut lamps: Query<(&Lamp, &mut Visibility)>) {
    let t = time.elapsed_secs();
    for (lamp, mut v) in &mut lamps {
        let want = if time.is_paused() || lamp.0.lit_at(t) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
}

/// A lamp's daytime glow.
#[derive(Component)]
pub(crate) struct DayGlow;

/// A piece's highest point, in the prefab's frame, and whether it stands
/// level (its own up is the prefab's). Level, it's the middle of its top
/// (a chimney's mouth, a flat roof). Tilted, it's the middle of its
/// highest corners: a hipped roof's slabs lean in, and their centres
/// pushed up along their own tilted up came out lower than a corner
/// post's top, so the lamp went on the post, under the eave (Marrow's
/// watchtower: a sliver of red past the roof's edge, DIM at 150 m).
fn highest(p: &crate::world::buildings::Piece) -> (Vec3, bool) {
    let level = (p.at.rotation * Vec3::Y).y > 0.999;
    let corners: Vec<Vec3> = match p.shape {
        Shape::Box(s) => {
            let h = s * 0.5;
            (0..8)
                .map(|i| {
                    Vec3::new(
                        if i & 1 == 0 { -h.x } else { h.x },
                        if i & 2 == 0 { -h.y } else { h.y },
                        if i & 4 == 0 { -h.z } else { h.z },
                    )
                })
                .collect()
        }
        // `materials::gable_prism`: the base at the origin, the ridge at
        // its full height along Z.
        Shape::Prism(s) => vec![
            Vec3::new(-s.x * 0.5, 0.0, -s.z * 0.5),
            Vec3::new(s.x * 0.5, 0.0, -s.z * 0.5),
            Vec3::new(-s.x * 0.5, 0.0, s.z * 0.5),
            Vec3::new(s.x * 0.5, 0.0, s.z * 0.5),
            Vec3::new(0.0, s.y, -s.z * 0.5),
            Vec3::new(0.0, s.y, s.z * 0.5),
        ],
        Shape::Cylinder(r, h) => (0..16)
            .map(|i| {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let y = if i < 8 { h * 0.5 } else { -h * 0.5 };
                Vec3::new(r * a.cos(), y, r * a.sin())
            })
            .collect(),
    };
    let world: Vec<Vec3> = corners.iter().map(|c| p.at.transform_point(*c)).collect();
    let top = world.iter().map(|c| c.y).fold(f32::NEG_INFINITY, f32::max);
    if level {
        // The middle of its top, at the top's height.
        let mid = match p.shape {
            Shape::Prism(s) => Vec3::new(0.0, s.y, 0.0),
            Shape::Box(s) => Vec3::new(0.0, s.y * 0.5, 0.0),
            Shape::Cylinder(_, h) => Vec3::new(0.0, h * 0.5, 0.0),
        };
        return (p.at.transform_point(mid), true);
    }
    let ties: Vec<Vec3> = world.into_iter().filter(|c| c.y > top - 0.01).collect();
    (
        ties.iter().copied().sum::<Vec3>() / ties.len() as f32,
        false,
    )
}

/// The prefab's highest piece, its highest point, and whether it's level.
fn top_piece(id: &str) -> Option<(crate::world::buildings::Piece, Vec3, bool)> {
    let geo = prefab(id)?;
    geo.pieces
        .iter()
        .map(|p| {
            let (at, level) = highest(p);
            (p.clone(), at, level)
        })
        .max_by(|a, b| a.1.y.total_cmp(&b.1.y))
}

/// The highest point of a prefab, in its own frame (see [`highest`]),
/// or of its dressed art where that stands higher (see [`art_top`]).
pub fn top_of(id: &str) -> Option<Vec3> {
    let pieces = top_piece(id).map(|(_, at, _)| at);
    match (pieces, art_top(id)) {
        (Some(p), Some(a)) if a.y > p.y + 0.05 => Some(a),
        (p, a) => p.or(a),
    }
}

/// The highest point of a prefab's dressed art (its `models/props/` GLBs,
/// posed as `Geo::props` has them): the middle of its highest vertices.
/// A landmark dressed with a model can roof itself in the art alone: the
/// mill race's hipped roof (25 to 27.4 m, a finial to 28.2) has no
/// pieces, so its lamp went on a roof post's top at 25 m, under the eave:
/// a sliver of red past the roof's edge, DIM from 150 m. Read once a
/// prefab; none (no GLB on disk) leaves the pieces to say.
fn art_top(id: &str) -> Option<Vec3> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Vec3>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().ok()?.get(id) {
        return *v;
    }
    let geo = prefab(id)?;
    let root = crate::asset_root_path().join("models/props");
    let mut best: Option<Vec3> = None;
    for prop in &geo.props {
        for (name, at) in &prop.models {
            let Some(local) = glb_top(&root.join(format!("{name}.glb"))) else {
                continue;
            };
            let p = at.transform_point(local);
            if best.is_none_or(|b| p.y > b.y) {
                best = Some(p);
            }
        }
    }
    cache.lock().ok()?.insert(id.to_owned(), best);
    best
}

/// A GLB's highest vertices' middle, in its own frame (node transforms
/// taken as they are: the structures' art is one untransformed node).
fn glb_top(path: &std::path::Path) -> Option<Vec3> {
    let f = std::fs::read(path).ok()?;
    let u32_at = |o: usize| {
        f.get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let json_len = u32_at(12)? as usize;
    let json: serde_json::Value = serde_json::from_slice(f.get(20..20 + json_len)?).ok()?;
    let bin = 20 + json_len + 8;
    let mut pts: Vec<Vec3> = Vec::new();
    for mesh in json["meshes"].as_array()? {
        for prim in mesh["primitives"].as_array()? {
            let acc = &json["accessors"][prim["attributes"]["POSITION"].as_u64()? as usize];
            let view = &json["bufferViews"][acc["bufferView"].as_u64()? as usize];
            let off = bin
                + view["byteOffset"].as_u64().unwrap_or(0) as usize
                + acc["byteOffset"].as_u64().unwrap_or(0) as usize;
            let stride = view["byteStride"].as_u64().unwrap_or(12) as usize;
            for i in 0..acc["count"].as_u64()? as usize {
                let o = off + i * stride;
                let c = |k: usize| u32_at(o + 4 * k).map(f32::from_bits);
                pts.push(Vec3::new(c(0)?, c(1)?, c(2)?));
            }
        }
    }
    let top = pts.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    let ties: Vec<Vec3> = pts.into_iter().filter(|p| p.y > top - 0.01).collect();
    (!ties.is_empty()).then(|| ties.iter().copied().sum::<Vec3>() / ties.len() as f32)
}

/// How far a landmark's lamp stands up off its top, on a pole: half the
/// top's width times 0.75, so a wide flat top (a chimney's mouth, a
/// tower's roof) doesn't hide it from below: it shows looking up at
/// anything under 53 degrees (from 50 m off a 30 m chimney is 31). A
/// pitched roof's lamp sits on the ridge, a hipped one's on its peak; no
/// pole is over `POLE_MAX`.
pub fn lamp_lift(id: &str) -> f32 {
    let Some((p, at, level)) = top_piece(id) else {
        return 0.0;
    };
    let lift = if level {
        match p.shape {
            Shape::Box(s) => 0.75 * 0.5 * s.x.max(s.z),
            Shape::Cylinder(r, _) => 0.75 * r,
            Shape::Prism(_) => 0.0,
        }
        .min(POLE_MAX)
    } else {
        0.0
    };
    // On the art's own top where that's higher (`top_of`): only as much
    // pole as keeps the lamp where the pieces' top and pole had it, never
    // lower (a wide top's pole is what shows it from below).
    match top_of(id) {
        Some(t) if t.y > at.y + 0.05 => (at.y + lift - t.y).max(0.0),
        _ => lift,
    }
}

/// The tallest a lamp's pole gets (m).
const POLE_MAX: f32 = 1.5;

/// The lamp's pole (m).
const POLE_R: f32 = 0.05;

/// The bulb's light as it reaches the tonemapper: red on screen, not white
/// (`render::grade::ToTheEye`).
const BULB: LinearRgba = LinearRgba::rgb(0.9, 0.045, 0.014);

fn light_landmarks(
    mut commands: Commands,
    zone: Option<Res<crate::world::Zone>>,
    buildings: Query<(Entity, &Building, &Transform), Added<Building>>,
    mut lamps_of: Local<Option<Vec<(String, [f32; 2], extraction_core::map_landmarks::Character)>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut pole_mat: Local<Option<Handle<StandardMaterial>>>,
    mut made: Local<
        Option<(
            Handle<Mesh>,
            Handle<StandardMaterial>,
            Handle<Mesh>,
            Handle<StandardMaterial>,
        )>,
    >,
) {
    // Each landmark's lamp character, by its placement (a building is
    // found by its prefab and where it stands).
    let lamps_of = lamps_of.get_or_insert_with(|| {
        zone.as_ref().map_or(Vec::new(), |z| {
            extraction_core::map_landmarks::placement_lamps(&z.map, &z.hm)
                .into_iter()
                .map(|(i, c)| {
                    (
                        z.map.placements[i].prefab.clone(),
                        z.map.placements[i].at,
                        c,
                    )
                })
                .collect()
        })
    });
    for (e, b, xf) in &buildings {
        if prefab_footprint(&b.prefab)[1] < LANDMARK_HEIGHT || LIT.contains(&b.prefab.as_str()) {
            continue;
        }
        let here = [xf.translation.x, xf.translation.z];
        let character = lamps_of
            .iter()
            .filter(|(id, _, _)| *id == b.prefab)
            .min_by(|a, b| {
                extraction_core::geom::dist(a.1, here)
                    .total_cmp(&extraction_core::geom::dist(b.1, here))
            })
            .map(|(_, _, c)| *c)
            .unwrap_or(
                extraction_core::map_landmarks::family(extraction_core::map_landmarks::kind(
                    &b.prefab,
                ))[0],
            );
        let Some(top) = top_of(&b.prefab) else {
            continue;
        };
        let (lamp_mesh, lamp, glow_mesh, glow) = made
            .get_or_insert_with(|| {
                (
                    meshes.add(Sphere::new(LAMP_R).mesh().uv(12, 8)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.5, 0.02, 0.01),
                        // Set each frame against the eyes' adaptation
                        // (`render::grade::ToTheEye`).
                        emissive: BULB,
                        ..default()
                    }),
                    meshes.add(Rectangle::new(DAY_R * 2.0, DAY_R * 2.0)),
                    materials.add(StandardMaterial {
                        base_color: Color::LinearRgba(LinearRgba::rgb(3.0, 0.15, 0.05)),
                        base_color_texture: Some(images.add(falloff())),
                        unlit: true,
                        alpha_mode: AlphaMode::Add,
                        // Fog on an added quad adds the fog's colour over
                        // all of it: a pale square (or the old sphere's
                        // flat pink coin).
                        fog_enabled: false,
                        ..default()
                    }),
                )
            })
            .clone();
        commands.entity(e).insert(crate::render::grade::ToTheEye {
            material: lamp.clone(),
            emissive: BULB,
        });
        let lift = lamp_lift(&b.prefab);
        let at = top + Vec3::Y * (lift + LAMP_R + 0.05);
        if lift > 0.05 {
            let mat = pole_mat
                .get_or_insert_with(|| {
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.08, 0.08, 0.08),
                        perceptual_roughness: 0.7,
                        metallic: 0.6,
                        ..default()
                    })
                })
                .clone();
            let pole = meshes.add(Cylinder::new(POLE_R, lift).mesh().resolution(6));
            commands.entity(e).with_child((
                Name::new("Landmark lamp pole"),
                Mesh3d(pole),
                MeshMaterial3d(mat),
                Transform::from_translation(top + Vec3::Y * lift * 0.5),
            ));
        }
        commands.entity(e).with_children(|c| {
            c.spawn((
                Name::new("Landmark lamp"),
                Lamp(character),
                Visibility::default(),
                Mesh3d(lamp_mesh),
                MeshMaterial3d(lamp),
                Transform::from_translation(at),
                NotShadowCaster,
                Halo {
                    color: HALO,
                    radius: HALO_R,
                },
                HaloReach(HALO_REACH),
            ))
            .with_child((
                Name::new("Landmark lamp glow"),
                DayGlow,
                Mesh3d(glow_mesh),
                MeshMaterial3d(glow),
                Transform::default(),
                NotShadowCaster,
            ));
        });
    }
}

/// A disc of light fading from the middle to nothing at its edge (white;
/// the material tints it).
pub(crate) fn falloff() -> Image {
    const N: u32 = 64;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let d = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / N as f32 * 2.0 - Vec2::ONE;
            let a = (1.0 - d.length()).clamp(0.0, 1.0).powi(2);
            let v = (a * 255.0) as u8;
            data.extend_from_slice(&[v, v, v, 255]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}

/// The day glows face the eye, and give way to the Halo after dark.
fn day_glows(
    conditions: Res<crate::world::weather::Conditions>,
    eyes: Query<(&GlobalTransform, &Camera), With<Camera3d>>,
    parents: Query<&GlobalTransform, Without<DayGlow>>,
    mut glows: Query<(&mut Transform, &mut Visibility, &ChildOf), With<DayGlow>>,
) {
    let show = !crate::world::halos::halos_show(&conditions);
    let eye = eyes
        .iter()
        .filter(|(_, c)| c.is_active)
        .map(|(g, _)| g.translation())
        .next();
    for (mut t, mut v, parent) in &mut glows {
        let want = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
        let (Some(eye), Ok(p)) = (eye, parents.get(parent.parent())) else {
            continue;
        };
        if !show {
            continue;
        }
        let (_, rot, at) = p.to_scale_rotation_translation();
        let face = Quat::from_rotation_arc(Vec3::Z, (eye - at).normalize_or(Vec3::Z));
        t.rotation = rot.inverse() * face;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the clock stopped every lamp shows, whatever its phase: the
    /// captures' stills stop it to shoot every lamp lit, the same each
    /// build. Running, a lamp goes dark between flashes.
    #[test]
    fn a_stopped_clock_lights_every_lamp() {
        use extraction_core::map_landmarks::MASTS;
        let quick = MASTS[0];
        // A moment it's dark.
        let dark = (0..400)
            .map(|k| k as f32 * 0.025)
            .find(|t| !quick.lit_at(*t))
            .unwrap();
        for paused in [false, true] {
            let mut app = App::new();
            let mut time = Time::<Virtual>::default();
            time.advance_by(std::time::Duration::from_secs_f32(dark));
            if paused {
                time.pause();
            }
            app.insert_resource(time);
            app.add_systems(Update, flash_lamps);
            let lamp = app
                .world_mut()
                .spawn((Lamp(quick), Visibility::Inherited))
                .id();
            app.update();
            let v = *app.world().get::<Visibility>(lamp).unwrap();
            let want = if paused {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            assert_eq!(v, want, "paused {paused}: at {dark} s");
        }
    }

    /// A chimney's lamp stands clear of its mouth: seen from 50 m off and
    /// a whole chimney below (the lamp gauge's nearest view), the line to
    /// the bulb's top clears the rim.
    #[test]
    fn a_chimney_lamp_shows_over_its_rim_from_below() {
        let top = top_of("chimney").expect("a chimney has a top");
        let lift = lamp_lift("chimney");
        assert!(lift > 0.5, "{lift}");
        let geo = prefab("chimney").unwrap();
        let r = geo
            .pieces
            .iter()
            .filter_map(|p| match p.shape {
                Shape::Cylinder(r, _) => Some(r),
                _ => None,
            })
            .fold(0.0f32, f32::max);
        // Looking up from 50 m off at the chimney's foot: over the rim
        // (r out from the axis) the line has risen r * tan by the axis.
        let tan = top.y / 50.0;
        assert!(
            lift + 2.0 * LAMP_R > r * tan,
            "lift {lift}, rim {r}, tan {tan}"
        );
        // No pole past POLE_MAX, whatever the top.
        assert!(lamp_lift("house_plank") <= POLE_MAX);
    }

    /// A landmark's lamp sits at its true highest point, whatever its roof:
    /// no piece of the building reaches above the lamp's foot (a hipped
    /// roof's slabs, leaning in, once left it on a corner post under the
    /// eave), and on a tilted top it sits on the peak with no pole.
    #[test]
    fn every_landmark_lamp_is_at_its_highest_point() {
        let mut bad = Vec::new();
        for id in crate::world::buildings::PREFABS {
            if prefab_footprint(id)[1] < LANDMARK_HEIGHT {
                continue;
            }
            let (Some(top), Some(geo)) = (top_of(id), prefab(id)) else {
                continue;
            };
            // Its pieces' and its dressed art's (the mill race's roof is
            // art alone).
            let peak = geo
                .pieces
                .iter()
                .map(|p| highest(p).0.y)
                .fold(f32::NEG_INFINITY, f32::max)
                .max(art_top(id).map_or(f32::NEG_INFINITY, |a| a.y));
            if top.y < peak - 0.02 {
                bad.push(format!("{id}: lamp at {:.2}, its top at {peak:.2}", top.y));
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// Taking the art's top never lowers a lamp: it stands at least where
    /// the pieces' top and pole had it.
    #[test]
    fn the_art_top_never_lowers_a_lamp() {
        for id in crate::world::buildings::PREFABS {
            let Some((p, at, level)) = top_piece(id) else {
                continue;
            };
            let pole = if level {
                match p.shape {
                    Shape::Box(s) => 0.75 * 0.5 * s.x.max(s.z),
                    Shape::Cylinder(r, _) => 0.75 * r,
                    Shape::Prism(_) => 0.0,
                }
                .min(POLE_MAX)
            } else {
                0.0
            };
            let lamp = top_of(id).unwrap().y + lamp_lift(id);
            assert!(
                lamp >= at.y + pole - 1e-3,
                "{id}: {lamp} under {}",
                at.y + pole
            );
        }
    }

    /// The mill race's lamp is on its roof's peak (the art's finial at
    /// 28.2 m), not on a roof post's top under the eave.
    #[test]
    fn the_mill_race_lamp_tops_its_roof() {
        let top = top_of("mill_race").unwrap();
        assert!(top.y > 28.0, "{top}");
        assert!(top.x.abs() < 0.2, "over the roof's middle: {top}");
        assert_eq!(lamp_lift("mill_race"), 0.0);
    }

    /// A hipped roof's slab: its highest point is its upper edge, not its
    /// middle pushed up along its own tilt.
    #[test]
    fn a_leaning_slab_tops_out_at_its_upper_edge() {
        let slab = crate::world::buildings::Piece {
            at: Transform::from_rotation(Quat::from_rotation_x(0.5)),
            ..prefab("house_plank").unwrap().pieces[0]
        };
        let slab = crate::world::buildings::Piece {
            shape: Shape::Box(Vec3::new(4.0, 0.1, 3.0)),
            ..slab
        };
        let (at, level) = highest(&slab);
        assert!(!level);
        // Tilted 0.5 rad about X: the edge 1.5 m along Z comes up by
        // 1.5 sin 0.5 (plus half the thickness on the cos).
        let want = 1.5 * 0.5f32.sin() + 0.05 * 0.5f32.cos();
        assert!((at.y - want).abs() < 0.01, "{} against {want}", at.y);
        assert!(at.x.abs() < 1e-3, "the middle of the edge: {at}");
    }

    /// Every landmark the valleys place has a top to carry a lamp, near the
    /// height it's listed at.
    #[test]
    fn every_landmark_has_a_top_for_its_lamp() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/maps");
        let mut bad = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let text = std::fs::read_to_string(e.path()).unwrap();
            let Ok(map) = extraction_core::map::MapDef::from_ron(&text) else {
                continue;
            };
            for p in &map.placements {
                let h = prefab_footprint(&p.prefab)[1];
                if h < LANDMARK_HEIGHT || LIT.contains(&p.prefab.as_str()) {
                    continue;
                }
                if !seen.insert(p.prefab.clone()) {
                    continue;
                }
                match top_of(&p.prefab) {
                    None => bad.push(format!("{}: no geometry", p.prefab)),
                    Some(t) if t.y < h * 0.6 => bad.push(format!(
                        "{}: top at {:.1} m, listed {h:.1} m",
                        p.prefab, t.y
                    )),
                    _ => {}
                }
            }
        }
        assert!(!seen.is_empty());
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
