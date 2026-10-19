//! What the training yard's stations look like: a timber post with a
//! yellow band, a plank board behind the words, a hooded lantern on top
//! (lit on the station you're on, green once done, dark ahead), and the
//! props the stations need (the far-grab stand, the range table, crates).
//!
//! The lantern hangs at head height under a hood on a painted post: it
//! must read as "put up by people", not as a glow in the grass, which is
//! what an anomaly looks like.

use bevy::prelude::*;

use super::{BOARD_Y, CHECKLIST, LAMP_Y, Lamp, Prop, SIGN, StationPost, TRUCK_SIGN};
use crate::render::materials::{Mat, MaterialLibrary, metric_box};

pub struct OnboardingVisualsPlugin;

impl Plugin for OnboardingVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (dress_posts, dress_props, light_lamps, chime).chain(),
        );
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
    for (e, post) in &posts {
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
        let (board, lamp_at) = if post.index == 0 {
            // In the truck: a board hung on two cords from the canopy, the
            // lantern hanging beside it.
            part(
                Some(Mat::Planks),
                Vec3::new(TRUCK_SIGN.x + 0.05, TRUCK_SIGN.y + 0.05, 0.025),
                Transform::default(),
            );
            for x in [-0.3, 0.3] {
                part(
                    Some(Mat::DarkMetal),
                    Vec3::new(0.008, 0.36, 0.008),
                    Transform::from_xyz(x, TRUCK_SIGN.y * 0.5 + 0.18, 0.0),
                );
            }
            (Vec3::ZERO, Vec3::new(TRUCK_SIGN.x * 0.5 + 0.12, 0.05, 0.05))
        } else {
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
            (Vec3::Y * BOARD_Y, Vec3::new(0.0, LAMP_Y - 0.14, 0.24))
        };
        let _ = board;
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
                PointLight {
                    color: Color::srgb(1.0, 0.82, 0.5),
                    intensity: 30_000.0,
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
            Prop::BoardPost => {
                part(
                    Mat::Timber,
                    Vec3::new(0.1, 2.0, 0.1),
                    Transform::from_xyz(0.0, 1.0, -0.05),
                );
                part(
                    Mat::Planks,
                    Vec3::new(CHECKLIST.x + 0.06, CHECKLIST.y + 0.06, 0.03),
                    Transform::from_xyz(0.0, 1.45, 0.01),
                );
                part(
                    Mat::Planks,
                    Vec3::new(0.82, 0.4, 0.03),
                    Transform::from_xyz(0.0, 0.72, 0.01),
                );
            }
        }
    }
}

/// A bell for each step ticked, two for a station done (made by
/// `scripts/make_chimes.py`). Not where the station is: in your ears, like
/// the radio, since you may be anywhere when it counts.
fn chime(
    mut commands: Commands,
    mut done: MessageReader<super::GoalDone>,
    server: Res<AssetServer>,
    samples: Option<Res<Assets<bevy_seedling::prelude::AudioSample>>>,
) {
    use bevy_seedling::prelude::*;
    if samples.is_none() {
        done.clear();
        return;
    }
    for d in done.read() {
        let file = if d.station_done {
            "audio/ui/chime_station.ogg"
        } else {
            "audio/ui/chime_step.ogg"
        };
        commands.spawn(
            SamplePlayer::new(server.load::<AudioSample>(file))
                .with_volume(Volume::Decibels(-12.0)),
        );
    }
}
