//! The yard lamps' light (`world::way_lamps`): a warm bulb under the hood,
//! a glow that carries a valley's width after dark, and a small pool of
//! light on the ground under it. Warm amber: the landmarks' lamps are red
//! and the way-out beacons green, so a yard lamp is neither.
//!
//! And a marker post's bands of reflective tape, in the models'
//! `Reflector` material, so `render::reflectors` lights them for the eye's
//! own torch as it does a snow pole's.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::models::ModelMaterials;
use crate::render::materials::MaterialLibrary;
use crate::world::buildings::Building;
use crate::world::halos::{Halo, HaloReach};
use crate::world::way_lamps::{BAND_H, BANDS, BULB_AT, MARKER_W};

pub struct WayLampsPlugin;

impl Plugin for WayLampsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (light_way_lamps, band_marker_posts));
    }
}

/// The bulb as the tonemapper gets it (`render::grade::ToTheEye`), its
/// halo's colour and size, and how far off the halo shows (m).
const BULB: LinearRgba = LinearRgba::rgb(0.9, 0.55, 0.18);
const HALO: LinearRgba = LinearRgba::rgb(1.6, 0.8, 0.22);
const HALO_R: f32 = 0.8;
const HALO_REACH: f32 = 240.0;

fn light_way_lamps(
    mut commands: Commands,
    lamps: Query<(Entity, &Building), Added<Building>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    for (e, b) in &lamps {
        if b.prefab != "way_lamp" {
            continue;
        }
        let (mesh, bulb) = made
            .get_or_insert_with(|| {
                (
                    meshes.add(Sphere::new(0.08).mesh().uv(10, 6)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.5, 0.3, 0.1),
                        // Set each frame against the eyes' adaptation.
                        emissive: BULB,
                        ..default()
                    }),
                )
            })
            .clone();
        commands.entity(e).insert(crate::render::grade::ToTheEye {
            material: bulb.clone(),
            emissive: BULB,
        });
        commands.entity(e).with_child((
            Name::new("Way lamp bulb"),
            Mesh3d(mesh),
            MeshMaterial3d(bulb),
            Transform::from_translation(BULB_AT),
            NotShadowCaster,
            Halo {
                color: HALO,
                radius: HALO_R,
            },
            HaloReach(HALO_REACH),
            PointLight {
                color: Color::srgb(1.0, 0.75, 0.45),
                intensity: 40_000.0,
                range: 10.0,
                radius: 0.05,
                shadow_maps_enabled: false,
                ..default()
            },
        ));
    }
}

/// A marker post with its bands on.
#[derive(Component)]
struct Banded;

/// The bands, a hair proud of the post all round.
const BAND_W: f32 = MARKER_W + 0.006;

/// Bands a marker post (`world::way_lamps::BANDS`) once the tape's
/// material is made and `render::reflectors` has its glowing copies, so
/// the bands are there to be found when they arrive.
fn band_marker_posts(
    mut commands: Commands,
    posts: Query<(Entity, &Building), Without<Banded>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    names: Query<&Name>,
    mut band: Local<Option<Handle<Mesh>>>,
) {
    let mut posts = posts
        .iter()
        .filter(|(_, b)| b.prefab == "marker_post")
        .peekable();
    if posts.peek().is_none() {
        return;
    }
    let Some(tape) = model_mats.cached("Reflector").cloned() else {
        model_mats.get("Reflector", &mut lib, &server, &mut materials);
        return;
    };
    // `render::reflectors` finds the tape as it's added, and only once it
    // has made its glowing copies (it names them): in a valley with no
    // snow poles the tape is first made here, so wait for those.
    if !names
        .iter()
        .any(|n| n.as_str().starts_with("Reflector glow"))
    {
        return;
    }
    let mesh = band
        .get_or_insert_with(|| meshes.add(Cuboid::new(BAND_W, BAND_H, BAND_W)))
        .clone();
    for (e, _) in posts {
        commands.entity(e).insert(Banded).with_children(|p| {
            for y in BANDS {
                p.spawn((
                    Name::new("Marker post band"),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(tape.clone()),
                    Transform::from_xyz(0.0, y, 0.0),
                    NotShadowCaster,
                ));
            }
        });
    }
}
