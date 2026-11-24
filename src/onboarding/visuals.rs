//! What the training yard's stations look like: a timber post with a
//! yellow band, a plank board behind the words, a hooded lantern on top
//! (lit on the station you're on, green once done, dark ahead), and the
//! props the stations need (the far-grab stand, the range table, crates).
//!
//! The lantern hangs at head height under a hood on a painted post: it
//! must read as "put up by people", not as a glow in the grass, which is
//! what an anomaly looks like.

use bevy::prelude::*;

use super::{BOARD_Y, LAMP_Y, Lamp, Prop, SIGN, StationPost};
use crate::render::materials::{Mat, MaterialLibrary, metric_box};

pub struct OnboardingVisualsPlugin;

impl Plugin for OnboardingVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                dress_posts,
                dress_props,
                light_lamps,
                flare_lamps,
                ring_marked,
                lay_dots,
                pulse_dots,
            )
                .chain(),
        );
    }
}

/// A lamp just lit: it flares three times as it comes on, so the eye
/// finds the next station from the last one (the payoff of finishing a
/// station, and the way on, in one).
#[derive(Component)]
struct Flare(f32);

/// How long a lamp flares, s, and how bright at the top of a flare.
const FLARE: f32 = 1.5;
const LAMP_LIGHT: f32 = 30_000.0;

fn flare_lamps(
    mut commands: Commands,
    mut lights: Query<(Entity, &mut PointLight, &mut Flare)>,
    time: Res<Time>,
) {
    for (e, mut light, mut flare) in &mut lights {
        flare.0 += time.delta_secs();
        let t = flare.0;
        if t >= FLARE {
            light.intensity = LAMP_LIGHT;
            light.range = 8.0;
            commands.entity(e).remove::<Flare>();
            continue;
        }
        // Three bright beats, fading.
        let beat = 0.5 - 0.5 * (t / FLARE * 3.0 * std::f32::consts::TAU).cos();
        let k = 1.0 + 6.0 * beat * (1.0 - t / FLARE);
        light.intensity = LAMP_LIGHT * k;
        light.range = 8.0 * k.sqrt();
    }
}

/// A station lantern's glass, and the light it gives while lit.
#[derive(Component)]
struct LampGlass;

#[derive(Component)]
struct LampLight;

#[derive(Resource)]
struct LampMats {
    dark: Handle<StandardMaterial>,
    lit: Handle<StandardMaterial>,
    done: Handle<StandardMaterial>,
}

fn lamp_mats(materials: &mut Assets<StandardMaterial>) -> LampMats {
    LampMats {
        dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.28, 0.27, 0.24),
            perceptual_roughness: 0.3,
            ..default()
        }),
        lit: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.4),
            emissive: LinearRgba::rgb(14.0, 9.0, 2.2),
            ..default()
        }),
        done: materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.9, 0.45),
            emissive: LinearRgba::rgb(1.2, 4.0, 1.4),
            ..default()
        }),
    }
}

fn dress_posts(
    mut commands: Commands,
    posts: Query<(Entity, &StationPost), Added<StationPost>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    mats: Option<Res<LampMats>>,
) {
    if posts.is_empty() {
        return;
    }
    let dark = match &mats {
        Some(m) => m.dark.clone(),
        None => {
            let m = lamp_mats(&mut materials);
            let d = m.dark.clone();
            commands.insert_resource(m);
            d
        }
    };
    let yellow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.66, 0.12),
        perceptual_roughness: 0.7,
        ..default()
    });
    for (e, _) in &posts {
        let mut part = |m: Option<Mat>, size: Vec3, at: Transform| {
            let material = match m {
                Some(m) => lib.get(m, &server, &mut materials),
                None => yellow.clone(),
            };
            commands.spawn((
                Mesh3d(meshes.add(metric_box(size))),
                MeshMaterial3d(material),
                at,
                ChildOf(e),
            ));
        };
        // The lantern's height above the board's middle.
        let lamp_at = {
            let h = LAMP_Y + 0.1;
            part(
                Some(Mat::Timber),
                Vec3::new(0.1, h, 0.1),
                Transform::from_xyz(0.0, h * 0.5, -0.05),
            );
            // A yellow band low on the post and the arm the lantern hangs
            // from: painted, so it reads as a sign, not as nature.
            part(
                None,
                Vec3::new(0.11, 0.3, 0.11),
                Transform::from_xyz(0.0, 0.5, -0.05),
            );
            part(
                None,
                Vec3::new(0.06, 0.06, 0.32),
                Transform::from_xyz(0.0, LAMP_Y + 0.02, 0.1),
            );
            part(
                Some(Mat::Planks),
                Vec3::new(SIGN.x + 0.06, SIGN.y + 0.06, 0.03),
                Transform::from_xyz(0.0, BOARD_Y, 0.01),
            );
            Vec3::new(0.0, LAMP_Y - 0.14, 0.24)
        };
        let hood = lib.get(Mat::DarkMetal, &server, &mut materials);
        commands.spawn((
            Mesh3d(meshes.add(Cone {
                radius: 0.11,
                height: 0.08,
            })),
            MeshMaterial3d(hood.clone()),
            Transform::from_translation(lamp_at + Vec3::Y * 0.12),
            ChildOf(e),
        ));
        commands.spawn((
            LampGlass,
            Mesh3d(meshes.add(Cylinder::new(0.06, 0.16))),
            MeshMaterial3d(dark.clone()),
            Transform::from_translation(lamp_at),
            ChildOf(e),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.07, 0.02))),
            MeshMaterial3d(hood),
            Transform::from_translation(lamp_at - Vec3::Y * 0.09),
            ChildOf(e),
        ));
    }
}

/// Lamps follow their station's state; the lit one gives light.
fn light_lamps(
    mut commands: Commands,
    posts: Query<(Entity, &StationPost, &Children), Changed<StationPost>>,
    mut glass: Query<(&mut MeshMaterial3d<StandardMaterial>, &Transform), With<LampGlass>>,
    lights: Query<(Entity, &ChildOf), With<LampLight>>,
    mats: Option<Res<LampMats>>,
) {
    let Some(mats) = mats else {
        return;
    };
    for (e, post, children) in &posts {
        let mut at = None;
        for c in children.iter() {
            if let Ok((mut m, t)) = glass.get_mut(c) {
                m.0 = match post.lamp {
                    Lamp::Dark => mats.dark.clone(),
                    Lamp::Lit => mats.lit.clone(),
                    Lamp::Done => mats.done.clone(),
                };
                at = Some(t.translation);
            }
        }
        for (l, parent) in &lights {
            if parent.parent() == e {
                commands.entity(l).despawn();
            }
        }
        if let (Lamp::Lit, Some(at)) = (post.lamp, at) {
            commands.spawn((
                LampLight,
                Flare(0.0),
                PointLight {
                    color: Color::srgb(1.0, 0.82, 0.5),
                    intensity: LAMP_LIGHT,
                    range: 8.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(at),
                ChildOf(e),
            ));
        }
    }
}

fn dress_props(
    mut commands: Commands,
    props: Query<(Entity, &Prop), Added<Prop>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for (e, p) in &props {
        let mut part = |m: Mat, size: Vec3, at: Transform| {
            let material = lib.get(m, &server, &mut materials);
            commands.spawn((
                Mesh3d(meshes.add(metric_box(size))),
                MeshMaterial3d(material),
                at,
                ChildOf(e),
            ));
        };
        match p {
            Prop::FarStand => {
                // A crate in a pen of posts and two rails a side.
                let c = super::FAR_CRATE;
                part(
                    Mat::Planks,
                    c,
                    Transform::from_xyz(0.0, c.y * 0.5, 0.0)
                        .with_rotation(Quat::from_rotation_y(0.15)),
                );
                let h = super::PEN_HALF;
                let f = super::PEN_FENCE;
                for (x, z) in [
                    (-h, -h),
                    (h, -h),
                    (-h, h),
                    (h, h),
                    (0.0, h),
                    (0.0, -h),
                    (h, 0.0),
                    (-h, 0.0),
                ] {
                    part(
                        Mat::Timber,
                        Vec3::new(0.09, f + 0.08, 0.09),
                        Transform::from_xyz(x, (f + 0.08) * 0.5, z),
                    );
                }
                for y in [f * 0.5, f - 0.04] {
                    for (along_x, at) in [(true, h), (true, -h), (false, h), (false, -h)] {
                        let (size, pos) = if along_x {
                            (Vec3::new(2.0 * h, 0.07, 0.04), Vec3::new(0.0, y, at))
                        } else {
                            (Vec3::new(0.04, 0.07, 2.0 * h), Vec3::new(at, y, 0.0))
                        };
                        part(Mat::Timber, size, Transform::from_translation(pos));
                    }
                }
            }
            Prop::RangeTable => {
                let t = super::TABLE;
                part(
                    Mat::Planks,
                    Vec3::new(t.x, 0.04, t.z),
                    Transform::from_xyz(0.0, t.y - 0.02, 0.0),
                );
                for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    part(
                        Mat::Timber,
                        Vec3::new(0.06, t.y - 0.04, 0.06),
                        Transform::from_xyz(
                            x * (t.x * 0.5 - 0.06),
                            (t.y - 0.04) * 0.5,
                            z * (t.z * 0.5 - 0.06),
                        ),
                    );
                }
            }
            Prop::Crate(size) => {
                part(
                    Mat::Planks,
                    *size,
                    Transform::from_xyz(0.0, size.y * 0.5, 0.0),
                );
            }
            Prop::LowBeam => {
                // Two stacks of crates, a beam laid across them at head
                // height, a rag tied on it.
                let gap = super::BEAM_GAP;
                let clear = super::BEAM_CLEAR;
                let off = (gap + 0.8) * 0.5;
                let h = clear * 0.5;
                for x in [-off, off] {
                    for (k, turn) in [0.0f32, 0.12].into_iter().enumerate() {
                        part(
                            Mat::Planks,
                            Vec3::new(0.78, h - 0.01, 0.78),
                            Transform::from_xyz(x, h * (k as f32 + 0.5), 0.0)
                                .with_rotation(Quat::from_rotation_y(turn)),
                        );
                    }
                }
                part(
                    Mat::Timber,
                    Vec3::new(gap + 1.5, 0.16, 0.16),
                    Transform::from_xyz(0.0, clear + 0.08, 0.0),
                );
                part(
                    Mat::Canvas,
                    Vec3::new(0.08, 0.3, 0.01),
                    Transform::from_xyz(0.3, clear - 0.15 + 0.08, 0.09),
                );
            }
            Prop::Stump => {
                let material = lib.get(Mat::BarkPine, &server, &mut materials);
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.17, super::STUMP_H))),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, super::STUMP_H * 0.5, 0.0),
                    ChildOf(e),
                ));
            }
            Prop::Flags(r, colour) => {
                // Six survey flags round it, knee high: red where to throw,
                // blue where to stand.
                let red = materials.add(StandardMaterial {
                    base_color: match colour {
                        super::Flag::Red => Color::srgb(0.85, 0.08, 0.06),
                        super::Flag::Blue => Color::srgb(0.08, 0.3, 0.85),
                    },
                    perceptual_roughness: 0.8,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                });
                let pole = lib.get(Mat::Timber, &server, &mut materials);
                for k in 0..6 {
                    let a = k as f32 * std::f32::consts::TAU / 6.0;
                    let at = Vec3::new(a.cos(), 0.0, a.sin()) * (r + 0.15);
                    commands.spawn((
                        Mesh3d(meshes.add(metric_box(Vec3::new(0.02, 0.9, 0.02)))),
                        MeshMaterial3d(pole.clone()),
                        Transform::from_translation(at + Vec3::Y * 0.45),
                        ChildOf(e),
                    ));
                    commands.spawn((
                        Mesh3d(meshes.add(metric_box(Vec3::new(0.24, 0.15, 0.004)))),
                        MeshMaterial3d(red.clone()),
                        Transform::from_translation(at + Vec3::new(0.12, 0.8, 0.0))
                            .with_rotation(Quat::from_rotation_y(-a)),
                        ChildOf(e),
                    ));
                }
            }
            Prop::SettingsBoard => {
                let w = super::SETTINGS_BOARD;
                for x in [-w.x * 0.5, w.x * 0.5] {
                    part(
                        Mat::Timber,
                        Vec3::new(0.1, 2.0, 0.1),
                        Transform::from_xyz(x, 1.0, -0.05),
                    );
                }
                part(
                    Mat::Planks,
                    Vec3::new(w.x, w.y, 0.03),
                    Transform::from_xyz(0.0, super::SETTINGS_BOARD_Y, -0.02),
                );
            }
        }
    }
}

/// The ring round what the step names (`Marked`).
#[derive(Component)]
struct MarkRing;

#[derive(Resource)]
struct MarkMat(Handle<StandardMaterial>);

/// A thin steady amber ring round what the step names: what to reach for,
/// at a glance, without words. Steady, and nothing like the hands' green
/// reach glow or the far-grab ring, so it reads as "this one" and not as
/// "in reach".
#[allow(clippy::type_complexity)]
fn ring_marked(
    mut commands: Commands,
    added: Query<
        (
            Entity,
            Option<&avian3d::prelude::ColliderAabb>,
            Option<&crate::anomaly::Anomaly>,
            &GlobalTransform,
        ),
        Added<super::Marked>,
    >,
    mut removed: RemovedComponents<super::Marked>,
    rings: Query<(Entity, &ChildOf), With<MarkRing>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mat: Option<Res<MarkMat>>,
) {
    let handle = match mat {
        Some(m) => m.0.clone(),
        None => {
            let h = materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.8, 0.35, 0.8),
                emissive: LinearRgba::rgb(3.0, 2.0, 0.6),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
            commands.insert_resource(MarkMat(h.clone()));
            h
        }
    };
    for e in removed.read() {
        for (ring, parent) in &rings {
            if parent.parent() == e {
                commands.entity(ring).despawn();
            }
        }
    }
    for (e, aabb, anomaly, gt) in &added {
        // Round it, a little wider than it is across (an anomaly: its
        // edge, on the ground).
        let r = match (anomaly, aabb) {
            (Some(a), _) => a.radius,
            (_, Some(a)) => ((a.max - a.min).xz().length() * 0.5 + 0.04).clamp(0.08, 0.6),
            _ => 0.15,
        };
        let t = gt.compute_transform();
        let scale = t.scale.max_element().max(1e-3);
        commands.spawn((
            MarkRing,
            Mesh3d(meshes.add(Torus::new(r - 0.006, r + 0.006))),
            MeshMaterial3d(handle.clone()),
            // Flat on the ground whatever lies in it (a pistol on its side
            // turned the ring on edge with it).
            Transform::from_scale(Vec3::splat(1.0 / scale)).with_rotation(t.rotation.inverse()),
            ChildOf(e),
        ));
    }
}

/// One of the way-on trail's dots (`super::trail::TrailLeg`): how far
/// along its leg it is (m), and its own material (each glows in turn).
#[derive(Component)]
struct TrailDot {
    along: f32,
    mat: Handle<StandardMaterial>,
}

/// The trail's look (Legibility, 2026-11-23): the way-lamps' warmth
/// (`render::way_lamps::BULB`, never the machine markers' orange), read by
/// its pale paint in daylight and drizzle and by its glow only after dusk;
/// at night fainter than the lamp it leads from. Emissive is in scene
/// units and the grade adapts it: not scaled here as well.
///
/// Each a rounded dash along the leg, 12 cm by 45 (Legibility, measuring
/// the stills): a round dot on the ground foreshortens with the square of
/// its distance, and past 8 m it was 2-3 px tall, under the 0.35 degree
/// floor. At 38 cm the dash held it to 8.6 m and was a pixel short at 10
/// (its rounded ends lose about a fifth); 45 holds it to 10 m. And it
/// points the way. The paint warm enough to part from wet grey by its hue (the first,
/// paler, read whitish), at the bulb's hue and well off the markers' red.
const DASH_W: f32 = 0.12;
const DASH_L: f32 = 0.45;
const DOT_PAINT: Color = Color::srgb(0.80, 0.66, 0.42);
/// A dot's glow at rest and on the crest, as a share of the bulb's (every
/// channel stays under 0.5 linear: no bloom).
const DOT_REST: f32 = 0.15;
const DOT_CREST: f32 = 0.45;
/// The crest runs toward where you're going at a walk or slower (m/s: a
/// dot each 0.5 s), each dot easing up and down over `DOT_EASE` s, then a
/// pause before it starts again from the near end. No hard on and off.
const CREST_SPEED: f32 = 3.0;
const DOT_EASE: f32 = 1.0;
const CREST_PAUSE: f32 = 1.5;

/// A new leg: its dots laid, the last leg's taken up.
fn lay_dots(
    mut commands: Commands,
    leg: Option<Res<super::trail::TrailLeg>>,
    old: Query<Entity, With<TrailDot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(leg) = leg.filter(|l| l.is_changed()) else {
        return;
    };
    for e in &old {
        commands.entity(e).despawn();
    }
    if leg.dots.is_empty() {
        return;
    }
    let mesh = meshes.add(Capsule2d::new(DASH_W * 0.5, DASH_L - DASH_W));
    // Lying flat, its length (the shape's +Y) along -Z; then turned about
    // the up axis to lie along the leg, the way on.
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let n = leg.dots.len();
    for (i, p) in leg.dots.iter().enumerate() {
        let along = leg.dots[(i + 1).min(n - 1)] - leg.dots[i.saturating_sub(1)];
        let yaw = if along.xz().length_squared() > 1e-6 {
            f32::atan2(-along.x, -along.z)
        } else {
            0.0
        };
        let mat = materials.add(StandardMaterial {
            base_color: DOT_PAINT,
            emissive: crate::render::way_lamps::BULB * DOT_REST,
            perceptual_roughness: 0.9,
            ..default()
        });
        commands.spawn((
            Name::new("Trail dot"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(*p).with_rotation(Quat::from_rotation_y(yaw) * flat),
            bevy::light::NotShadowCaster,
            TrailDot {
                along: i as f32 * super::trail::DOT_EVERY,
                mat,
            },
        ));
    }
}

/// How brightly a dot `along` m down its leg glows at `t` s, as a share of
/// the bulb's: rest, rising smoothly to the crest as it passes.
pub fn dot_glow(along: f32, length: f32, t: f32) -> f32 {
    let period = length / CREST_SPEED + CREST_PAUSE;
    let crest = (t % period) * CREST_SPEED;
    let half = CREST_SPEED * DOT_EASE * 0.5;
    let x = ((along - crest).abs() / half).min(1.0);
    let k = 0.5 + 0.5 * (std::f32::consts::PI * x).cos();
    DOT_REST + (DOT_CREST - DOT_REST) * k
}

fn pulse_dots(
    time: Res<Time>,
    dots: Query<&TrailDot>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let length = dots.iter().map(|d| d.along).fold(0.0f32, f32::max) + super::trail::DOT_EVERY;
    let t = time.elapsed_secs();
    for d in &dots {
        if let Some(mut m) = materials.get_mut(&d.mat) {
            m.emissive = crate::render::way_lamps::BULB * dot_glow(d.along, length, t);
        }
    }
}

#[cfg(test)]
mod trail_look_tests {
    use super::*;

    /// Legibility's floors for the trail: its glow never past half a
    /// linear unit in any channel (no bloom, under the lamp), never under
    /// rest, and its crest moves at a walk or slower.
    #[test]
    fn the_trail_glows_softly_and_leads_at_a_walk() {
        let bulb = crate::render::way_lamps::BULB;
        let mut peak = 0.0f32;
        for i in 0..200 {
            let t = i as f32 * 0.05;
            for k in 0..20 {
                let g = dot_glow(k as f32 * 1.5, 30.0, t);
                assert!(g >= DOT_REST - 1e-4 && g <= DOT_CREST + 1e-4, "{g}");
                peak = peak.max(g);
            }
        }
        assert!(bulb.red * peak < 0.5 && bulb.green * peak < 0.5 && bulb.blue * peak < 0.5);
        assert!(CREST_SPEED <= 4.0, "a walk or slower");
        assert!(
            super::super::trail::DOT_EVERY / CREST_SPEED >= 0.4,
            "a dot each 0.4 s or slower"
        );
        assert!(DASH_W >= 0.12, "12 cm across at least");
        // Its length on screen, about length x eye height / d^2, holds the
        // 0.35 degree floor out to 10 m from a 1.7 m eye.
        let at_10 = (DASH_L * 1.7 / 100.0).to_degrees();
        assert!(at_10 >= 0.35, "{at_10:.2} deg at 10 m");
    }
}
