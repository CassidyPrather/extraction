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

/// A lamp's character (`map_landmarks::character`): when in its period
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

/// The highest point of a prefab, in its own frame: the top middle of its
/// highest piece.
pub fn top_of(id: &str) -> Option<Vec3> {
    let geo = prefab(id)?;
    geo.pieces
        .iter()
        .map(|p| {
            let half_h = match p.shape {
                Shape::Box(s) | Shape::Prism(s) => s.y * 0.5,
                Shape::Cylinder(_, h) => h * 0.5,
            };
            p.at.translation + p.at.rotation * Vec3::Y * half_h
        })
        .max_by(|a, b| a.y.total_cmp(&b.y))
}

fn light_landmarks(
    mut commands: Commands,
    buildings: Query<(Entity, &Building), Added<Building>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut made: Local<
        Option<(
            Handle<Mesh>,
            Handle<StandardMaterial>,
            Handle<Mesh>,
            Handle<StandardMaterial>,
        )>,
    >,
) {
    for (e, b) in &buildings {
        if prefab_footprint(&b.prefab)[1] < LANDMARK_HEIGHT || LIT.contains(&b.prefab.as_str()) {
            continue;
        }
        let Some(top) = top_of(&b.prefab) else {
            continue;
        };
        let (lamp_mesh, lamp, glow_mesh, glow) = made
            .get_or_insert_with(|| {
                (
                    meshes.add(Sphere::new(LAMP_R).mesh().uv(12, 8)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.5, 0.02, 0.01),
                        emissive: LinearRgba::rgb(120.0, 6.0, 2.0),
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
        let at = top + Vec3::Y * (LAMP_R + 0.05);
        commands.entity(e).with_children(|c| {
            c.spawn((
                Name::new("Landmark lamp"),
                Lamp(extraction_core::map_landmarks::character(
                    extraction_core::map_landmarks::kind(&b.prefab),
                )),
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
