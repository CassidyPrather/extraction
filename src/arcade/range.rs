//! The range lane, east of the bench: steel plates hung at 10, 25 and
//! 50 m from a firing line, to judge a gun's recoil, sights and sound
//! with nothing shooting back. A plate rings as steel does when struck
//! (the game's own impact) and swings back on its chains as hard as the
//! round hit it. `record` marks the shots and hits as the range's.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::stats::PLATES;
use super::{Arcade, ArcadeSystems};
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::render::legibility::TextSource;
use crate::render::materials::Mat;
use crate::weapons::ballistics::Hit;
use crate::world::surfaces::Surfaces;
use crate::world::{Layer, Zone};

/// The firing line's middle (map x, z); the plates are downrange of it,
/// east.
pub const LINE: [f32; 2] = [5.0, 33.0];

/// Downrange.
pub const DOWNRANGE: Vec3 = Vec3::X;

/// A plate's face (width, height) and thickness, m.
pub const PLATE: Vec2 = Vec2::new(0.45, 0.6);
pub const THICK: f32 = 0.02;

/// The crossbar the plates hang from, over the ground, m, and the chains
/// down to a plate's top.
pub const BAR: f32 = 1.9;
pub const CHAIN: f32 = 0.15;

/// A shot whose line passes this near a plate's middle was aimed at it, m.
pub const AIMED_NEAR: f32 = 1.5;

/// How hard a round swings a plate: radians a second per unit of the
/// round's momentum (kg m/s).
const KICK: f32 = 0.45;
/// How quickly a swing dies away (per second).
const DAMPING: f32 = 1.5;
const GRAVITY: f32 = 9.81;

/// A hung plate: which (0, 1, 2 for 10, 25, 50 m), where it hangs from,
/// and its swing.
#[derive(Component, Clone, Copy, Debug)]
pub struct RangePlate {
    pub index: usize,
    pub pivot: Vec3,
    pub angle: f32,
    pub spin: f32,
}

/// A plate's frame (`visuals` puts up its posts and bar).
#[derive(Component, Clone, Copy, Debug)]
pub struct RangeFrame;

/// The firing line (`visuals` lays a beam on the ground at it).
#[derive(Component, Clone, Copy, Debug)]
pub struct RangeLine;

/// The plate hung from `pivot` swung back `angle`.
pub fn plate_pose(pivot: Vec3, angle: f32) -> Transform {
    // Face toward the line: local +Z points back up the range.
    let face = Transform::default().looking_to(DOWNRANGE, Vec3::Y).rotation;
    let rot = face * Quat::from_rotation_x(angle);
    Transform::from_translation(pivot + rot * Vec3::new(0.0, -(CHAIN + PLATE.y * 0.5), 0.0))
        .with_rotation(rot)
}

/// Where the firing line is, on the ground.
pub fn line_at(zone: &Zone) -> Vec3 {
    zone.on_ground(LINE[0], LINE[1])
}

pub struct RangePlugin;

impl Plugin for RangePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, set_up.in_set(ArcadeSystems::Setup))
            .add_systems(Update, swing.in_set(ArcadeSystems::Play));
    }
}

fn set_up(mut commands: Commands, zone: Res<Zone>, arcade: Res<Arcade>, mut done: Local<bool>) {
    if *done || arcade.bench.is_none() {
        return;
    }
    *done = true;
    let line = line_at(&zone);
    let at_line = commands
        .spawn((
            Name::new("Range line"),
            RangeLine,
            Transform::from_translation(line).looking_to(DOWNRANGE, Vec3::Y),
            Visibility::default(),
        ))
        .id();
    // Its sign, on a post at the line's end, read from the bench.
    commands.spawn((
        Name::new("Range sign"),
        ScreenText {
            text: "RANGE\nplates at 10, 25, 50 m".into(),
            size: Vec2::new(0.5, 0.2),
            style: ScreenStyle::Card,
        },
        TextSource::Card,
        Transform::from_xyz(1.5, 1.4, 0.05),
        Visibility::default(),
        ChildOf(at_line),
    ));
    for (i, d) in PLATES.iter().enumerate() {
        let x = LINE[0] + d;
        let foot = zone.on_ground(x, LINE[1]);
        commands.spawn((
            Name::new(format!("Range frame, {d:.0} m")),
            RangeFrame,
            Transform::from_translation(foot).looking_to(DOWNRANGE, Vec3::Y),
            Visibility::default(),
        ));
        let pivot = foot + Vec3::Y * BAR;
        let half = Vec3::new(PLATE.x * 0.5, PLATE.y * 0.5, THICK * 0.5);
        commands.spawn((
            Name::new(format!("Range plate, {d:.0} m")),
            RangePlate {
                index: i,
                pivot,
                angle: 0.0,
                spin: 0.0,
            },
            plate_pose(pivot, 0.0),
            Surfaces(vec![(half, Transform::IDENTITY, Mat::DarkMetal)]),
            RigidBody::Static,
            Collider::cuboid(PLATE.x, PLATE.y, THICK),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
            Visibility::default(),
        ));
    }
    info!("arcade: range at {:.1}", line);
}

/// Struck plates swing back and settle, as a pendulum from the bar.
fn swing(
    time: Res<Time>,
    mut hits: MessageReader<Hit>,
    mut plates: Query<(&mut RangePlate, &mut Transform)>,
) {
    for h in hits.read() {
        if let Ok((mut p, _)) = plates.get_mut(h.entity) {
            let momentum = h.mass_g.max(0.0) * 1e-3 * h.velocity.length();
            let away = h.velocity.normalize_or_zero().dot(DOWNRANGE).signum();
            p.spin += away * momentum * KICK;
        }
    }
    let dt = time.delta_secs().min(0.05);
    let arm = CHAIN + PLATE.y * 0.5;
    for (mut p, mut t) in &mut plates {
        if p.angle.abs() < 1e-4 && p.spin.abs() < 1e-4 {
            continue;
        }
        let acc = -(GRAVITY / arm) * p.angle.sin() - DAMPING * p.spin;
        p.spin += acc * dt;
        p.angle = (p.angle + p.spin * dt).clamp(-1.3, 1.3);
        if p.angle.abs() < 1e-4 && p.spin.abs() < 1e-3 {
            p.angle = 0.0;
            p.spin = 0.0;
        }
        *t = plate_pose(p.pivot, p.angle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plate_hangs_square_to_the_line_and_swings_away() {
        let pivot = Vec3::new(20.0, 2.0, 0.0);
        let rest = plate_pose(pivot, 0.0);
        assert!((rest.translation - (pivot - Vec3::Y * (CHAIN + 0.3))).length() < 1e-4);
        // Its face looks back up the range.
        assert!((rest.rotation * Vec3::Z).dot(-DOWNRANGE) > 0.999);
        // Struck, it swings downrange.
        let back = plate_pose(pivot, 0.4);
        assert!(back.translation.x > rest.translation.x + 0.1);
    }

    #[test]
    fn the_lane_is_inside_the_fence_and_clear_of_the_track() {
        let map = extraction_core::map::MapDef::from_ron(extraction_core::map::ARCADE).unwrap();
        let inner = map.size * 0.5 - map.fence_inset;
        let far = LINE[0] + PLATES[PLATES.len() - 1];
        assert!(far < inner - 1.0, "the far plate stands inside the fence");
        assert!(
            LINE[0] - 2.0 > 2.0,
            "the line stands clear of the 4 m track at x 0"
        );
    }
}
