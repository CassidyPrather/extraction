//! The Gate's lamp (`robots::heavy_mind::GateCount`): while a valley key is
//! inside its 60 m and it counts toward calling a heavy, a red band across
//! each face of its lintel wakes at full strength, the instant the key
//! crosses the line, and swells faster as the count runs out (`held`
//! 0 → 1). By day the band reads as red on the white lintel; after dark
//! it has a held halo at each end and the middle. Quiet, it fades out in
//! a breath. It never lights when no heavy could be called (the Gate
//! never wakes then), so it never lies.
//!
//! A swell, never a blink: a smooth cosine, never under `LOW`, at
//! `SLOW`..`FAST` Hz, all under the 3 Hz where flashing starts to matter
//! in a headset.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::robots::heavy_mind::GateCount;
use crate::world::buildings::Building;
use crate::world::halos::{Halo, HaloAngle, HaloReach};

pub struct GateLampPlugin;

impl Plugin for GateLampPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (band_robot_gates, wake_gate_lamp).chain());
    }
}

/// The band's colour at full swell (linear; the tonemapper's via
/// `ToTheEye`): a machine's warning red, a band where the landmarks'
/// red is a point and the machine markers' an orange point.
pub const GATE_RED: LinearRgba = LinearRgba::rgb(1.0, 0.05, 0.02);
/// Its halos' colour at full swell, size, held angle (half, rad) and
/// reach (m): a disc of some 1.4 degrees from the 60 m line and past.
const GATE_HALO: LinearRgba = LinearRgba::rgb(3.0, 0.15, 0.06);
const GATE_HALO_R: f32 = 0.6;
const GATE_HALO_ANGLE: f32 = 0.012;
const GATE_HALO_REACH: f32 = 150.0;
/// The band on each face of the lintel (the gate's own space, m): 14 m by
/// 0.5, over the lintel's dark seam (`buildings::robot_gate`: the lintel
/// 17.6 x 1.6 x 4.0 about y 12.8).
pub const BAND: Vec2 = Vec2::new(14.0, 0.5);
pub const BAND_Y: f32 = 12.5;
pub const BAND_Z: f32 = 2.012;
/// The swell: its rate as the count starts and as it runs out (Hz), its
/// floor, and how long it takes to go out once quiet (s).
pub const SLOW: f32 = 0.75;
pub const FAST: f32 = 2.0;
pub const LOW: f32 = 0.45;
const FADE: f32 = 0.4;

/// The bands' shared glow.
#[derive(Component)]
struct GateGlow;

/// A Gate's halo.
#[derive(Component)]
struct GateHalo;

/// The swell's rate (Hz) `held` of the way through the count.
pub fn gate_rate(held: f32) -> f32 {
    SLOW + (FAST - SLOW) * held.clamp(0.0, 1.0)
}

/// The swell's strength at `phase` (rad): 1 at 0, where it wakes.
pub fn gate_swell(phase: f32) -> f32 {
    LOW + (1.0 - LOW) * (0.5 + 0.5 * phase.cos())
}

fn band_robot_gates(
    mut commands: Commands,
    gates: Query<(Entity, &Building), Added<Building>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    glow: Query<&crate::render::grade::ToTheEye, With<GateGlow>>,
) {
    for (e, b) in &gates {
        if b.prefab != "robot_gate" {
            continue;
        }
        let material = match glow.iter().next() {
            Some(g) => g.material.clone(),
            None => {
                let m = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.08, 0.02, 0.02),
                    perceptual_roughness: 0.4,
                    emissive: LinearRgba::BLACK,
                    ..default()
                });
                commands.spawn((
                    Name::new("Gate lamp glow"),
                    GateGlow,
                    crate::render::grade::ToTheEye {
                        material: m.clone(),
                        emissive: LinearRgba::BLACK,
                    },
                ));
                m
            }
        };
        let mesh = meshes.add(Cuboid::new(BAND.x, BAND.y, 0.02));
        for side in [-1.0f32, 1.0] {
            commands.entity(e).with_child((
                Name::new("Gate lamp band"),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(0.0, BAND_Y, side * BAND_Z),
                NotShadowCaster,
                NotShadowReceiver,
            ));
            for x in [-0.45 * BAND.x, 0.0, 0.45 * BAND.x] {
                commands.entity(e).with_child((
                    Name::new("Gate lamp halo"),
                    GateHalo,
                    Transform::from_xyz(x, BAND_Y, side * (BAND_Z + 0.05)),
                    Visibility::default(),
                    Halo {
                        color: LinearRgba::BLACK,
                        radius: GATE_HALO_R,
                    },
                    HaloReach(GATE_HALO_REACH),
                    HaloAngle(GATE_HALO_ANGLE),
                ));
            }
        }
    }
}

/// `EXTRACTION_GATE_SHOW=peak|trough`: the lamp held counting at its
/// brightest or dimmest (the shots' A/B against a quiet Gate).
fn shown() -> Option<f32> {
    static SHOW: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    *SHOW.get_or_init(|| match std::env::var("EXTRACTION_GATE_SHOW").as_deref() {
        Ok("peak") => Some(1.0),
        Ok("trough") => Some(LOW),
        _ => None,
    })
}

/// The lamp's strength now, from the Gate's count: full the frame it
/// wakes, swelling faster as it runs out, fading once quiet.
#[derive(Default)]
struct Swell {
    on: bool,
    phase: f32,
    level: f32,
}

impl Swell {
    fn step(&mut self, counting: bool, held: f32, dt: f32) -> f32 {
        if counting {
            if !self.on {
                self.phase = 0.0;
            } else {
                self.phase = (self.phase + std::f32::consts::TAU * gate_rate(held) * dt)
                    % std::f32::consts::TAU;
            }
            self.on = true;
            self.level = gate_swell(self.phase);
        } else {
            self.on = false;
            self.level = (self.level - dt / FADE).max(0.0);
        }
        self.level
    }
}

fn wake_gate_lamp(
    time: Res<Time>,
    count: Option<Res<GateCount>>,
    mut swell: Local<Swell>,
    mut frame: Local<u32>,
    mut glow: Query<&mut crate::render::grade::ToTheEye, With<GateGlow>>,
    mut halos: Query<&mut Halo, With<GateHalo>>,
) {
    let was_on = swell.on;
    let k = match shown() {
        Some(k) => k,
        None => {
            let (counting, held) = count.map_or((false, 0.0), |c| (c.counting, c.held));
            swell.step(counting, held, time.delta_secs())
        }
    };
    // Its shared band is written a third of the frames (as the machine
    // markers' lens, `way_lamps::SWELL_EVERY`), but the frame it wakes
    // at once: it reads from the first moment.
    *frame = frame.wrapping_add(1);
    let woke = swell.on && !was_on;
    if !woke && *frame % crate::render::way_lamps::SWELL_EVERY != 0 {
        return;
    }
    let want = GATE_RED * k;
    for mut g in &mut glow {
        if (g.emissive.red - want.red).abs() > 0.005 || (k == 0.0 && g.emissive.red != 0.0) {
            g.emissive = want;
        }
    }
    let want = GATE_HALO * k;
    for mut h in &mut halos {
        if (h.color.red - want.red).abs() > 0.01 * GATE_HALO.red || (k == 0.0 && h.color.red != 0.0)
        {
            h.color = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_wakes_full_the_frame_a_key_crosses() {
        let mut s = Swell::default();
        assert_eq!(s.step(false, 0.0, 1.0 / 90.0), 0.0);
        assert_eq!(s.step(true, 0.0, 1.0 / 90.0), 1.0);
    }

    #[test]
    fn it_quickens_as_the_count_runs_out_and_never_blinks() {
        assert!(gate_rate(0.0) < gate_rate(0.5) && gate_rate(0.5) < gate_rate(1.0));
        assert!(FAST < 3.0, "under where flashing matters in a headset");
        let frame = 1.0 / 90.0;
        let mut s = Swell::default();
        let (mut lo, mut step) = (1.0f32, 0.0f32);
        let mut last = s.step(true, 0.0, frame);
        // The whole 6 s count, held 0 → 1.
        for i in 1..540 {
            let v = s.step(true, i as f32 / 540.0, frame);
            lo = lo.min(v);
            step = step.max((v - last).abs());
            last = v;
        }
        assert!(lo >= LOW - 1e-4, "never out while counting: {lo}");
        // A headset frame never moves it more than 4%: a swell, no blink.
        assert!(step < 0.04, "{step}");
    }

    #[test]
    fn quiet_it_goes_out_in_a_breath() {
        let frame = 1.0 / 90.0;
        let mut s = Swell::default();
        s.step(true, 0.3, frame);
        let mut t = 0.0;
        while s.step(false, 0.0, frame) > 0.0 {
            t += frame;
            assert!(t < 0.5, "out within half a second");
        }
    }

    #[test]
    fn its_red_is_a_warning_not_a_marker_or_a_yard_lamp() {
        // Redder than the machine markers' orange.
        let m = crate::render::way_lamps::MARKER_AMBER;
        assert!(GATE_RED.green / GATE_RED.red < 0.5 * m.green / m.red);
    }

    #[test]
    fn a_gate_gets_a_band_on_each_face_and_its_halos() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_systems(Update, band_robot_gates);
        app.world_mut().spawn((
            Building {
                prefab: "robot_gate".into(),
            },
            Transform::default(),
        ));
        app.update();
        let w = app.world_mut();
        let halos = w.query_filtered::<(), With<GateHalo>>().iter(w).count();
        assert_eq!(halos, 6);
        let glows = w.query_filtered::<(), With<GateGlow>>().iter(w).count();
        assert_eq!(glows, 1);
    }
}
