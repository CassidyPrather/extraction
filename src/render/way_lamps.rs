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
use crate::world::halos::{Halo, HaloAngle, HaloReach};
use crate::world::way_lamps::{BAND_H, BANDS, BULB_AT, MARKER_W};

pub struct WayLampsPlugin;

impl Plugin for WayLampsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                light_way_lamps,
                band_marker_posts,
                halo_machine_markers,
                light_marker_lens,
                swell_machine_markers,
            ),
        );
    }
}

/// The bulb as the tonemapper gets it (`render::grade::ToTheEye`), its
/// halo's colour and size, and how far off the halo shows (m).
const BULB: LinearRgba = LinearRgba::rgb(0.9, 0.55, 0.18);
const HALO: LinearRgba = LinearRgba::rgb(1.6, 0.8, 0.22);
const HALO_R: f32 = 0.8;
const HALO_REACH: f32 = 240.0;

/// A machine boundary marker's lamp (`machine_marker`): hazard orange,
/// fully saturated, well redder than a yard lamp's pale warm bulb (a
/// warning, not a shelter, in a still frame before any pulse is seen) and
/// well yellower than the landmarks' red (linear; the tonemapper's via
/// `ToTheEye`).
pub const MARKER_AMBER: LinearRgba = LinearRgba::rgb(1.0, 0.18, 0.0);
/// Its slow swell (s): a single cosine, under half a hertz, never a
/// blink (VR comfort), every marker in step.
pub const MARKER_PERIOD: f32 = 2.4;
/// The swell's floor: never out, so it reads at any moment you look.
pub const MARKER_LOW: f32 = 0.35;

/// The marker halo's colour at full swell, its size, and how far off it
/// shows (m): a lamp to find a crossing by from 25 m and some way past,
/// well short of the landmarks' red, which carry across a valley.
const MARKER_HALO: LinearRgba = LinearRgba::rgb(1.5, 0.27, 0.0);
const MARKER_HALO_R: f32 = 0.3;
const MARKER_HALO_REACH: f32 = 90.0;
/// The least its halo spans (half-angle, rad): held, its bright core is
/// a disc of some 0.6 degrees out to 25 m and past, not a speck to hunt
/// for (`halos::HaloAngle`). Up close its own radius rules.
const MARKER_HALO_ANGLE: f32 = 0.06;

/// The one glow every marker's lens shares (all in step).
#[derive(Component)]
struct MarkerGlow;

/// A marker's halo, swelling with its lens.
#[derive(Component)]
struct MarkerHalo;

/// A machine marker's halo at its lamp. No light: a lamp to find, not to
/// light the wreck or sign it stands by.
fn halo_machine_markers(
    mut commands: Commands,
    markers: Query<(Entity, &Building), Added<Building>>,
) {
    for (e, b) in &markers {
        if b.prefab != "machine_marker" {
            continue;
        }
        commands.entity(e).with_child((
            Name::new("Machine marker halo"),
            MarkerHalo,
            Transform::from_translation(crate::world::way_lamps::MARKER_LAMP_AT),
            Visibility::default(),
            NotShadowCaster,
            Halo {
                color: MARKER_HALO,
                radius: MARKER_HALO_R,
            },
            HaloReach(MARKER_HALO_REACH),
            HaloAngle(MARKER_HALO_ANGLE),
        ));
    }
}

/// The markers' shared lens, once the model's material is in: lit (it came
/// unlit, which shows no emissive), a dim frosted ground, its glow set
/// against the eyes (`ToTheEye`) and swelled with the rest.
fn light_marker_lens(
    mut commands: Commands,
    model_mats: Option<Res<ModelMaterials>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    glow: Query<(), With<MarkerGlow>>,
) {
    if !glow.is_empty() {
        return;
    }
    let Some(lens) = model_mats.and_then(|m| m.cached("MarkerLens").cloned()) else {
        return;
    };
    if let Some(mut m) = materials.get_mut(&lens) {
        light_lens(&mut m);
    }
    commands.spawn((
        Name::new("Machine marker glow"),
        MarkerGlow,
        crate::render::grade::ToTheEye {
            material: lens,
            emissive: MARKER_AMBER,
        },
    ));
}

/// A marker lens made to glow: lit (unlit shows no emissive), on a dim
/// frosted ground, its glow first at full swell.
fn light_lens(m: &mut StandardMaterial) {
    m.unlit = false;
    m.base_color = Color::srgb(0.3, 0.27, 0.2);
    m.emissive = MARKER_AMBER;
}

/// Every marker's lens and halo, swelling together (`marker_pulse`).
fn swell_machine_markers(
    time: Res<Time>,
    mut glow: Query<&mut crate::render::grade::ToTheEye, With<MarkerGlow>>,
    mut halos: Query<&mut Halo, With<MarkerHalo>>,
) {
    let k = marker_pulse(time.elapsed_secs());
    for mut g in &mut glow {
        let want = MARKER_AMBER * k;
        if (g.emissive.red - want.red).abs() > 0.005 {
            g.emissive = want;
        }
    }
    let want = MARKER_HALO * k;
    for mut h in &mut halos {
        // (A rewrite re-prepares the halo's material: a visible step only.)
        if (h.color.red - want.red).abs() > 0.01 * MARKER_HALO.red {
            h.color = want;
        }
    }
}

/// The marker lamp's strength at `t` seconds, `MARKER_LOW..=1`.
pub fn marker_pulse(t: f32) -> f32 {
    let u = 0.5 - 0.5 * (std::f32::consts::TAU * t / MARKER_PERIOD).cos();
    MARKER_LOW + (1.0 - MARKER_LOW) * u
}

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

#[cfg(test)]
mod marker_tests {
    use super::*;

    /// Hue (degrees) and saturation of a linear colour, in sRGB.
    fn hue_sat(c: LinearRgba) -> (f32, f32) {
        let s = Color::from(c).to_srgba();
        let (r, g, b) = (s.red, s.green, s.blue);
        let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
        let d = mx - mn;
        let h = if d < 1e-6 {
            0.0
        } else if mx == r {
            60.0 * ((g - b) / d).rem_euclid(6.0)
        } else if mx == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        (h, if mx > 0.0 { d / mx } else { 0.0 })
    }

    #[test]
    fn a_machine_marker_gets_its_lamp_at_night() {
        // Its halo at the model's lamp, swelling with the rest (what Map
        // design's night gauge counts on: `map_gradient::SELF_LIT`).
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, halo_machine_markers);
        let m = app
            .world_mut()
            .spawn((
                Building {
                    prefab: "machine_marker".into(),
                },
                Transform::default(),
            ))
            .id();
        let lamp = app
            .world_mut()
            .spawn(Building {
                prefab: "way_lamp".into(),
            })
            .id();
        app.update();
        let w = app.world_mut();
        let halos: Vec<(Entity, Vec3, f32)> = w
            .query_filtered::<(&ChildOf, &Transform, &Halo), With<MarkerHalo>>()
            .iter(w)
            .map(|(c, t, h)| (c.parent(), t.translation, h.radius))
            .collect();
        assert_eq!(halos.len(), 1, "one halo, the marker's: {halos:?}");
        assert_eq!(halos[0].0, m);
        assert!(halos[0].0 != lamp);
        assert_eq!(halos[0].1, crate::world::way_lamps::MARKER_LAMP_AT);
        // And its lens, which came unlit, glows.
        let mut lens = StandardMaterial {
            unlit: true,
            ..default()
        };
        light_lens(&mut lens);
        assert!(!lens.unlit && lens.emissive == MARKER_AMBER);
        // Short of the landmarks' red, which carry across a valley.
        assert!(MARKER_HALO_REACH < HALO_REACH);
    }

    #[test]
    fn the_marker_breathes_slowly_and_never_goes_out() {
        assert!(1.0 / MARKER_PERIOD <= 0.5, "no faster than half a hertz");
        let frame = 1.0 / 90.0;
        let (mut lo, mut hi, mut step) = (1.0f32, 0.0f32, 0.0f32);
        let mut last = marker_pulse(0.0);
        for i in 1..(90.0 * MARKER_PERIOD * 2.0) as usize {
            let v = marker_pulse(i as f32 * frame);
            lo = lo.min(v);
            hi = hi.max(v);
            step = step.max((v - last).abs());
            last = v;
        }
        assert!((lo - MARKER_LOW).abs() < 0.01 && hi > 0.99, "{lo}..{hi}");
        // A headset frame never steps it more than 1.5%: a swell, no blink.
        assert!(step < 0.015, "{step}");
    }

    #[test]
    fn the_marker_is_amber_not_a_yard_lamp_nor_a_landmark() {
        let (mh, ms) = hue_sat(MARKER_AMBER);
        let (yh, ys) = hue_sat(BULB);
        // Hazard orange: between red (0) and yellow (60), well off the
        // landmarks' red (under 10) and the yard lamp's 37.
        assert!((22.0..32.0).contains(&mh), "marker hue {mh}");
        let (lh, _) = hue_sat(LinearRgba::rgb(4.0, 0.35, 0.18));
        assert!(mh > lh + 12.0, "well off the landmarks' red ({mh} vs {lh})");
        // A yard lamp is a pale warm white: the marker deeply saturated,
        // and redder, so a still frame tells them apart.
        assert!(ms > 0.95 && ys < 0.6, "saturation {ms} vs yard {ys}");
        assert!(mh + 6.0 < yh, "redder than the yard lamp ({mh} vs {yh})");
        let (hh, hs) = hue_sat(MARKER_HALO);
        assert!((hh - mh).abs() < 4.0 && hs > 0.95, "halo {hh} {hs}");
    }
}
