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

/// The highest a prefab's pieces come (m, over its origin), and how near
/// to it another piece's top must be to count as level with it.
const TIED: f32 = 0.05;

/// A piece's top, the middle of it, in its prefab's frame.
fn piece_top(p: &crate::world::buildings::Piece) -> Vec3 {
    // How far up from its origin its top is: a box's and a cylinder's
    // origin is their middle, a gable prism's its base
    // (`materials::gable_prism`: the ridge at its full height).
    let up = match p.shape {
        Shape::Box(s) => s.y * 0.5,
        Shape::Prism(s) => s.y,
        Shape::Cylinder(_, h) => h * 0.5,
    };
    p.at.translation + p.at.rotation * Vec3::Y * up
}

/// How far apart level tops may be and still count as one top (m): a
/// cage's corner posts or a mast's legs, not a dam's two end turrets.
const ONE_TOP: f32 = 10.0;

/// The pieces whose tops are a prefab's highest, level within `TIED`, and
/// of those the ones that make one top: within `ONE_TOP` of the one
/// nearest the prefab's middle.
fn highest(geo: &crate::world::buildings::Geo) -> Vec<&crate::world::buildings::Piece> {
    let peak = geo
        .pieces
        .iter()
        .map(|p| piece_top(p).y)
        .fold(f32::MIN, f32::max);
    let tied: Vec<_> = geo
        .pieces
        .iter()
        .filter(|p| piece_top(p).y >= peak - TIED)
        .collect();
    let Some(seed) = tied
        .iter()
        .map(|p| piece_top(p).xz())
        .min_by(|a, b| a.length().total_cmp(&b.length()))
    else {
        return tied;
    };
    tied.into_iter()
        .filter(|p| (piece_top(p).xz() - seed).length() <= ONE_TOP)
        .collect()
}

/// The highest point of a prefab, in its own frame: the middle of its
/// highest pieces' tops (a cage's four corner posts level at the top put
/// the lamp over the cage, not on one corner of it).
pub fn top_of(id: &str) -> Option<Vec3> {
    let geo = prefab(id)?;
    let tops: Vec<Vec3> = highest(&geo).into_iter().map(piece_top).collect();
    (!tops.is_empty()).then(|| tops.iter().copied().sum::<Vec3>() / tops.len() as f32)
}

/// Whether some piece of a prefab, solid or dressing, has its top just
/// under `t` (within 0.6 m), there to carry a lamp.
fn carried(geo: &crate::world::buildings::Geo, t: Vec3) -> bool {
    geo.pieces.iter().any(|q| {
        let l = q.at.compute_affine().inverse().transform_point3(t);
        let (inside, top) = match q.shape {
            Shape::Box(s) => (l.x.abs() <= s.x * 0.5 && l.z.abs() <= s.z * 0.5, s.y * 0.5),
            Shape::Prism(s) => (l.x.abs() <= s.x * 0.5 && l.z.abs() <= s.z * 0.5, s.y),
            Shape::Cylinder(r, h) => (l.xz().length() <= r, h * 0.5),
        };
        let y = (q.at.translation + q.at.rotation * Vec3::Y * top).y;
        inside && y <= t.y + 0.01 && y >= t.y - 0.6
    })
}

/// The bars a lamp sits on where nothing of its building is under the
/// middle of its top (an open cage, a mast's legs): one from the lamp out
/// to each of the top's pieces, in the prefab's frame.
pub fn lamp_seat(id: &str) -> Vec<(Vec3, Vec3)> {
    let (Some(geo), Some(t)) = (prefab(id), top_of(id)) else {
        return Vec::new();
    };
    if carried(&geo, t) {
        return Vec::new();
    }
    highest(&geo)
        .into_iter()
        .map(|p| (t, piece_top(p)))
        .collect()
}

/// How far a landmark's lamp stands up off its top, on a pole: half the
/// top's width times 0.75, so a wide flat top (a chimney's mouth, a
/// tower's roof) doesn't hide it from below: it shows looking up at
/// anything under 53 degrees (from 50 m off a 30 m chimney is 31). A
/// pitched roof's lamp sits on the ridge; no pole is over `POLE_MAX`.
pub fn lamp_lift(id: &str) -> f32 {
    let Some(geo) = prefab(id) else {
        return 0.0;
    };
    // The widest of the highest pieces sets it.
    highest(&geo)
        .into_iter()
        .map(|p| match p.shape {
            Shape::Box(s) => 0.75 * 0.5 * s.x.max(s.z),
            Shape::Cylinder(r, _) => 0.75 * r,
            Shape::Prism(_) => 0.0,
        })
        .fold(0.0, f32::max)
        .min(POLE_MAX)
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
        // Bars to sit on, where its top is an open frame.
        let seat = lamp_seat(&b.prefab);
        if !seat.is_empty() {
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
            for (from, to) in seat {
                let run = (to - from).xz();
                let bar = meshes.add(Cuboid::new(0.08, 0.08, run.length() + 0.1));
                commands.entity(e).with_child((
                    Name::new("Landmark lamp seat"),
                    Mesh3d(bar),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_translation((from + to) * 0.5 - Vec3::Y * 0.04)
                        .with_rotation(Quat::from_rotation_y(run.x.atan2(run.y))),
                ));
            }
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

    /// A lamp on a top that's several pieces level with each other (the
    /// mill race's cage, four corner posts) stands over the middle of
    /// them, not on whichever came last.
    #[test]
    fn a_lamp_stands_over_the_middle_of_a_level_top() {
        let geo = prefab("mill_race").unwrap();
        let tied = highest(&geo);
        assert!(tied.len() > 1, "the mill race's top is its cage's posts");
        let mid = tied.iter().map(|p| piece_top(p)).sum::<Vec3>() / tied.len() as f32;
        let top = top_of("mill_race").unwrap();
        assert!(
            (top - mid).length() < 0.01,
            "lamp at {top:?}, the posts' middle {mid:?}"
        );
        for p in tied {
            let t = piece_top(p);
            assert!(
                (t.xz() - top.xz()).length() > 0.5,
                "the lamp still on a post at {t:?}"
            );
        }
    }

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

    /// Every landmark's lamp stands on its building: inside the prefab's
    /// footprint, and on something, solid or dressing, whose top is just
    /// under it there, or on its seat's bars (no lamp in the air over an
    /// open frame, none on one post of several).
    #[test]
    fn every_landmark_lamp_stands_on_its_building() {
        let mut bad = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for (_, text) in extraction_core::map::VALLEYS {
            let map = extraction_core::map::MapDef::from_ron(text).unwrap();
            for p in &map.placements {
                let [w, h, d] = prefab_footprint(&p.prefab);
                if h < LANDMARK_HEIGHT || LIT.contains(&p.prefab.as_str()) {
                    continue;
                }
                if !seen.insert(p.prefab.clone()) {
                    continue;
                }
                let (Some(t), Some(geo)) = (top_of(&p.prefab), prefab(&p.prefab)) else {
                    continue;
                };
                if t.x.abs() > w * 0.5 + 0.1 || t.z.abs() > d * 0.5 + 0.1 {
                    bad.push(format!(
                        "{}: lamp at ({:.1}, {:.1}) outside its {w} x {d} footprint",
                        p.prefab, t.x, t.z
                    ));
                }
                let under = carried(&geo, t) || !lamp_seat(&p.prefab).is_empty();
                if !under {
                    bad.push(format!(
                        "{}: nothing under its lamp at ({:.1}, {:.1}, {:.1})",
                        p.prefab, t.x, t.y, t.z
                    ));
                }
            }
        }
        assert!(seen.len() > 5, "only {} landmark prefabs", seen.len());
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
