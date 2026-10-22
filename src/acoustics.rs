//! What the space around the listener sounds like, from its geometry
//! (`docs/AUDIO.md`, "Acoustics").
//!
//! A handful of rays from the head ([`probe`]) say how enclosed the place
//! is, how big, what its surfaces are made of and what roof is overhead.
//! From that comes a [`Reverb`]: how much of each sound returns as a tail
//! ([`Reverb::wet_db`]), how long the tail lasts ([`Reverb::rt60`]) and
//! how bright it stays ([`Reverb::damping`]). A forest is dry, a timber
//! room short and soft, a corrugated hall rings, a tunnel underground
//! echoes on and on.
//!
//! Also here: [`occlusion`], how a sound reaches the listener round or
//! through what's in the way, by the same rule robots hear by
//! (`robots::senses`): a wall or a hill between them cuts a sound's reach
//! to [`OCCLUDED`]; an open doorway on the way lets most of it through.
//!
//! Nothing here plays sound: `audio` does that. This module runs headless,
//! so its judgements can be tested on the real maps.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::render::materials::Mat;
use crate::robots::senses::{HEARD_OVER, Lighting, OCCLUDED};
use crate::world::Layer;
use crate::world::surfaces::Surfaces;

/// How far the probe's rays look (m): past this, it's open.
pub const PROBE_RANGE: f32 = 40.0;

/// What the walls and roof are mostly made of, as sound sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    /// Sheet metal, steel: hard and ringing.
    Metal,
    /// Concrete, rock: hard and bare.
    Hard,
    /// Plaster, brick, glass: a lived-in room's walls.
    Masonry,
    /// Planks, logs, shingles: warm and short.
    Wood,
    /// Canvas, bark and needles, earth: soaks it up.
    Soft,
}

/// What rain falls on overhead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Roof {
    /// Corrugated or sheet metal: the drumming tin roof.
    Tin,
    /// Shingles and planks.
    Wood,
    /// Canvas: a tent, a tarp.
    Canvas,
    /// Concrete, rock: underground, or a slab. Rain isn't heard.
    Solid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpaceKind {
    /// Sky overhead and little around: a field, a forest, a hillside.
    Open,
    /// Sky overhead but walls close round: an alley, a yard, a quarry.
    Sheltered,
    /// A roof and walls, small.
    Room,
    /// A roof and walls, big.
    Hall,
    /// Underground.
    Underground,
}

/// The space round a listener.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Space {
    pub kind: SpaceKind,
    /// Share of the probe's rays that met something within range.
    pub enclosed: f32,
    /// Mean distance to what they met, m (the range if they met nothing):
    /// the room's size. Rays that met nothing are its openings, counted in
    /// `enclosed`.
    pub size: f32,
    /// The longest way round: the mean of the three longest hits, m (a
    /// corridor's length, a hall's far end).
    pub length: f32,
    pub surface: Surface,
    pub roof: Option<Roof>,
}

impl Default for Space {
    fn default() -> Self {
        Self {
            kind: SpaceKind::Open,
            enclosed: 0.0,
            size: PROBE_RANGE,
            length: PROBE_RANGE,
            surface: Surface::Soft,
            roof: None,
        }
    }
}

/// A reverb setting: the reverb bus plays at `wet_db`, decaying over
/// `rt60` seconds, high frequencies damped by `damping` (0 bright, 1 dull).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reverb {
    pub wet_db: f32,
    pub rt60: f32,
    pub damping: f32,
}

/// Wet level that counts as off.
pub const DRY: f32 = -60.0;

impl Space {
    pub fn reverb(&self) -> Reverb {
        // Sabine: RT60 = 0.161 V / (S a), and for a box the mean free
        // path is 4V/S, so RT60 is about 0.04 x path / a. Absorption `a`
        // by surface, counting what's in a room (crates, beds, rubble)
        // and not just the bare walls: bare concrete is 0.02.
        let (absorption, damping) = match self.surface {
            Surface::Metal => (0.05, 0.15),
            Surface::Hard => (0.07, 0.35),
            Surface::Masonry => (0.15, 0.45),
            Surface::Wood => (0.2, 0.55),
            Surface::Soft => (0.45, 0.8),
        };
        let per_m = 0.04 / absorption;
        // Underground, a sound goes on down the corridor and back: the
        // longest way round counts as well as the mean. (Above ground the
        // long rays are ones that left through a window and met the next
        // building.)
        let path = match self.kind {
            SpaceKind::Underground => self.size.max(0.5 * self.length),
            _ => self.size,
        };
        let rt60 = (path * per_m * (0.4 + 0.6 * self.enclosed)).clamp(0.3, 3.5);
        let wet_db = match self.kind {
            SpaceKind::Open => DRY,
            SpaceKind::Sheltered => -24.0 + 6.0 * self.enclosed,
            SpaceKind::Room => -16.0 + 4.0 * self.enclosed,
            SpaceKind::Hall => -13.0 + 4.0 * self.enclosed,
            SpaceKind::Underground => -10.5 + 2.0 * self.enclosed,
        };
        Reverb {
            wet_db,
            rt60,
            damping,
        }
    }
}

/// Freeverb's comb filters average this long (1378 samples at 44.1 kHz).
pub const FREEVERB_COMB: f32 = 0.0312;
/// Measured by the audit (`scripts/audio_acoustics.py`).
pub const FREEVERB_DAMPED: f32 = 1.16;

/// The `room_size` that gives Freeverb a decay of `rt60` seconds: its
/// feedback is `0.7 + 0.28 * room_size`, and a comb of delay `d` with
/// feedback `g` falls 60 dB in `3 d / -log10 g`. The shortest it does
/// is about 0.6 s (room_size 0). Its damping shortens the tail too: the
/// audit renders it and measures 0.86 of the comb's decay at every size,
/// so the comb is asked for [`FREEVERB_DAMPED`] more.
pub fn freeverb_room(rt60: f32) -> f32 {
    let feedback = 10f32.powf(-3.0 * FREEVERB_COMB / (rt60 * FREEVERB_DAMPED).max(0.05));
    ((feedback - 0.7) / 0.28).clamp(0.0, 1.0)
}

pub fn surface_of(m: Mat) -> Surface {
    match m {
        Mat::RustyMetal
        | Mat::PaintedMetal
        | Mat::Corrugated
        | Mat::DarkMetal
        | Mat::RobotShell
        | Mat::GunMetal => Surface::Metal,
        Mat::Planks
        | Mat::LogWall
        | Mat::WoodFloor
        | Mat::Timber
        | Mat::Shingles
        | Mat::GunWood => Surface::Wood,
        Mat::Canvas | Mat::BarkSpruce | Mat::BarkBirch | Mat::BarkPine | Mat::Polymer => {
            Surface::Soft
        }
        Mat::Brick | Mat::Plaster | Mat::Glass => Surface::Masonry,
        _ => Surface::Hard,
    }
}

pub fn roof_of(m: Mat) -> Roof {
    match surface_of(m) {
        Surface::Metal => Roof::Tin,
        Surface::Wood => Roof::Wood,
        Surface::Soft => Roof::Canvas,
        Surface::Hard | Surface::Masonry => Roof::Solid,
    }
}

/// The probe's directions: twelve round the horizon, four raised 40°.
fn rays() -> impl Iterator<Item = Dir3> {
    (0..16).filter_map(|i| {
        let (k, up) = if i < 12 { (i, 0.0) } else { (i - 12, 40f32) };
        let n = if i < 12 { 12.0 } else { 4.0 };
        let a = k as f32 / n * std::f32::consts::TAU + if i < 12 { 0.0 } else { 0.4 };
        let up = up.to_radians();
        Dir3::new(Vec3::new(a.cos() * up.cos(), up.sin(), a.sin() * up.cos())).ok()
    })
}

/// The material a ray met, if it was something built.
fn material(built: &Query<(&GlobalTransform, &Surfaces)>, hit: Entity, at: Vec3) -> Option<Mat> {
    built.get(hit).ok().and_then(|(gt, s)| s.at(gt, at))
}

/// Looks round from `head`.
pub fn probe(
    spatial: &SpatialQuery,
    built: &Query<(&GlobalTransform, &Surfaces)>,
    lighting: Option<&Lighting>,
    head: Vec3,
) -> Space {
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    let mut n = 0;
    let mut votes = [0u32; 5];
    let mut hits = Vec::with_capacity(16);
    for dir in rays() {
        n += 1;
        if let Some(h) = spatial.cast_ray(head, dir, PROBE_RANGE, true, &world) {
            hits.push(h.distance);
            let s = material(built, h.entity, head + *dir * h.distance)
                .map_or(Surface::Soft, surface_of);
            votes[s as usize] += 1;
        }
    }
    let size = if hits.is_empty() {
        PROBE_RANGE
    } else {
        hits.iter().sum::<f32>() / hits.len() as f32
    };
    hits.sort_by(|a, b| b.total_cmp(a));
    let length = if hits.is_empty() {
        PROBE_RANGE
    } else {
        let k = hits.len().min(3);
        hits.iter().take(k).sum::<f32>() / k as f32
    };
    let up = spatial.cast_ray(head, Dir3::Y, PROBE_RANGE, true, &world);
    let underground = lighting.is_some_and(|l| l.in_dark(head));
    let roof = up.map(|h| {
        material(built, h.entity, head + Vec3::Y * h.distance).map_or(Roof::Solid, roof_of)
    });
    let enclosed = hits.len() as f32 / n as f32;
    let surface = [
        Surface::Metal,
        Surface::Hard,
        Surface::Masonry,
        Surface::Wood,
        Surface::Soft,
    ]
    .into_iter()
    .max_by_key(|s| votes[*s as usize])
    .unwrap_or(Surface::Soft);
    // Underground the plaster is on bare concrete, and nothing's left in
    // the rooms to soak sound up.
    let surface = if underground && matches!(surface, Surface::Masonry | Surface::Soft) {
        Surface::Hard
    } else {
        surface
    };
    let kind = if underground {
        SpaceKind::Underground
    } else if roof.is_some() && enclosed >= 0.5 {
        if size > 8.0 {
            SpaceKind::Hall
        } else {
            SpaceKind::Room
        }
    } else if enclosed >= 0.5 && surface != Surface::Soft {
        // Walls close round with sky overhead. Trunks and earth don't
        // count: a forest, a gully, are dry.
        SpaceKind::Sheltered
    } else {
        SpaceKind::Open
    };
    Space {
        kind,
        enclosed,
        size,
        length,
        surface,
        roof: if underground { Some(Roof::Solid) } else { roof },
    }
}

/// Rain as heard from where you are: the open-air rain bed's gain (dB) and
/// low-pass (Hz), and the roof's own drumming bed if there's one for it
/// (it plays at the rain's level). Under a porch or an overhang (a roof
/// but open sides) a little of both.
pub fn rain_under(space: &Space) -> (f32, f32, Option<&'static str>) {
    if space.kind == SpaceKind::Underground {
        return (-80.0, 400.0, None);
    }
    let Some(roof) = space.roof else {
        return (0.0, 20_480.0, None);
    };
    let (db, cutoff, drum) = match roof {
        Roof::Tin => (-12.0, 1_200.0, Some("ambience/rain_tin")),
        Roof::Wood => (-14.0, 900.0, Some("ambience/rain_wood")),
        Roof::Canvas => (-6.0, 3_000.0, Some("ambience/rain_canvas")),
        Roof::Solid => (-18.0, 600.0, None),
    };
    match space.kind {
        SpaceKind::Room | SpaceKind::Hall => (db, cutoff, drum),
        // A porch, an overhang: the open rain all round, a little duller.
        _ => (-3.0, 8_000.0, drum),
    }
}

/// How a sound gets from `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Occlusion {
    /// Gain to apply, dB (0 in the clear).
    pub db: f32,
    /// Low-pass cutoff, Hz (20 480 in the clear).
    pub cutoff: f32,
}

pub const CLEAR: Occlusion = Occlusion {
    db: 0.0,
    cutoff: 20_480.0,
};

/// Through a wall or over a hill: as far as robots hear it, a sound
/// carries [`OCCLUDED`] of its reach, which under inverse-distance falloff
/// is this much quieter; and walls pass the lows.
pub fn wall_db() -> f32 {
    20.0 * OCCLUDED.log10()
}
pub const WALL_CUTOFF: f32 = 700.0;
/// Round a doorway's edge: a little quieter, a little duller.
pub const DOORWAY_DB: f32 = -3.0;
pub const DOORWAY_CUTOFF: f32 = 3_000.0;

/// Is the straight path from `from` to `to` blocked? As robots hear:
/// the sound bends over what stands less than `HEARD_OVER` above its
/// source (a hump in a field), so the path counts as blocked only if the
/// line from just above the source and the line from `HEARD_OVER` above
/// it are both cut. (Robots try only the raised line, which through a
/// doorway runs into the lintel.)
pub fn blocked(spatial: &SpatialQuery, from: Vec3, to: Vec3) -> bool {
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    [0.3, HEARD_OVER].into_iter().all(|up| {
        let src = from + Vec3::Y * up;
        let d = src.distance(to);
        Dir3::new(to - src).ok().is_some_and(|dir| {
            spatial
                .cast_ray(src, dir, (d - 0.5).max(0.0), true, &world)
                .is_some()
        })
    })
}

/// How a sound at `from` reaches a listener at `to`: in the clear, round
/// the edge of one of `openings` (open doorways: both legs clear), or
/// through the wall.
pub fn occlusion(spatial: &SpatialQuery, openings: &[Vec3], from: Vec3, to: Vec3) -> Occlusion {
    if !blocked(spatial, from, to) {
        return CLEAR;
    }
    let d = from.distance(to);
    let via = openings
        .iter()
        .filter(|o| o.distance(from) + o.distance(to) < d * 1.6 + 4.0)
        .any(|o| !blocked(spatial, from, *o) && !blocked(spatial, *o, to));
    if via {
        Occlusion {
            db: DOORWAY_DB,
            cutoff: DOORWAY_CUTOFF,
        }
    } else {
        Occlusion {
            db: wall_db(),
            cutoff: WALL_CUTOFF,
        }
    }
}

/// A big flat face a loud sound slaps back off (Sallow's dam): its plane
/// through `at` facing `normal`, `half` along `along` either way, from
/// `base` up `height`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reflector {
    pub at: Vec3,
    pub normal: Vec3,
    pub along: Vec3,
    pub half: f32,
    pub base: f32,
    pub height: f32,
}

/// The faces in a zone's placements that slap back: the dam's downstream
/// face (towards +z in its own frame) and its upstream wall (towards
/// -z), each its length long (`world::dam`).
pub fn reflectors(placements: &[(String, Transform)]) -> Vec<Reflector> {
    placements
        .iter()
        .filter_map(|(prefab, xf)| {
            let (len, height, facing, plane) = match prefab.as_str() {
                "dam_face" => (265.0, 22.0, 1.0, 1.5),
                "dam_face_end" => (25.0, 22.0, 1.0, 1.5),
                "dam_back" => (245.0, 21.0, -1.0, -1.5),
                "dam_back_end" => (35.0, 21.0, -1.0, -1.5),
                // Rime Fell's mill (`world::fell`): its front faces down
                // the slope (local -Z), the lowest bay's wall with the next
                // one's front over its roof, 20 m wide, about 9 m up.
                "ore_mill" => (20.0, 9.0, -1.0, -20.0),
                _ => return None,
            };
            let normal = xf.rotation * Vec3::Z * facing;
            Some(Reflector {
                at: xf.translation + xf.rotation * Vec3::Z * plane,
                normal,
                along: xf.rotation * Vec3::X,
                half: len * 0.5,
                base: xf.translation.y,
                height,
            })
        })
        .collect()
}

/// Speed of sound, m/s.
pub const SOUND: f32 = 343.0;
/// A slapback is heard as one when it comes this long after the sound
/// itself (s); sooner, it fuses with it.
pub const SLAP_AFTER: f32 = 0.06;

/// A sound at `from` slapping back off `wall` to `ear`: where its image
/// is (the sound mirrored in the wall's plane, so it plays from behind
/// the wall at the length of the reflected path) and how long after the
/// sound itself it arrives (s). `None` if either is behind the wall, the
/// reflection misses it, or it would fuse with the sound.
pub fn slapback(wall: &Reflector, from: Vec3, ear: Vec3) -> Option<(Vec3, f32)> {
    let ds = (from - wall.at).dot(wall.normal);
    let dl = (ear - wall.at).dot(wall.normal);
    if ds <= 0.5 || dl <= 0.5 {
        return None;
    }
    let image = from - wall.normal * (2.0 * ds);
    // Where the reflected path meets the wall.
    let t = ds / (ds + dl);
    let hit = from + (ear - from) * t;
    let along = (hit - wall.at).dot(wall.along);
    let y = hit.y;
    if along.abs() > wall.half || y < wall.base || y > wall.base + wall.height {
        return None;
    }
    let delay = (image.distance(ear) - from.distance(ear)) / SOUND;
    (delay >= SLAP_AFTER).then_some((image, delay))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shot 100 m out from the dam's face, fired where you stand,
    /// comes back off it 0.58 s later from 100 m behind it; from behind
    /// the face, or past its end, it doesn't.
    #[test]
    fn the_dam_slaps_back() {
        let face = reflectors(&[("dam_face".into(), Transform::from_xyz(0.0, 30.0, 0.0))]);
        let wall = face[0];
        let here = Vec3::new(10.0, 32.0, 101.5);
        let (image, delay) = slapback(&wall, here, here).unwrap();
        assert!((delay - 200.0 / SOUND).abs() < 0.01, "{delay}");
        assert!((image.z - (1.5 - 100.0)).abs() < 0.01, "{image}");
        assert!(slapback(&wall, Vec3::new(0.0, 32.0, -20.0), here).is_none());
        assert!(
            slapback(
                &wall,
                Vec3::new(400.0, 32.0, 50.0),
                Vec3::new(400.0, 32.0, 60.0)
            )
            .is_none()
        );
        // Right at it: the echo fuses with the shot.
        let near = Vec3::new(0.0, 32.0, 6.0);
        assert!(slapback(&wall, near, near).is_none());
    }
    /// The mill as Rime Fell places it (turned to face south, its front
    /// at z = 40): a shot 50 m down the slope in front of it comes back
    /// 0.29 s later; from beside it, past its end, it doesn't.
    #[test]
    fn the_mill_slaps_back() {
        let at = Transform::from_xyz(140.0, 90.0, 20.0)
            .with_rotation(Quat::from_rotation_y(180f32.to_radians()));
        let wall = reflectors(&[("ore_mill".into(), at)])[0];
        assert!(
            (wall.at.z - 40.0).abs() < 0.01 && wall.normal.z > 0.99,
            "{wall:?}"
        );
        let here = Vec3::new(140.0, 92.0, 90.0);
        let (_, delay) = slapback(&wall, here, here).unwrap();
        assert!((delay - 100.0 / SOUND).abs() < 0.01, "{delay}");
        let beside = Vec3::new(190.0, 92.0, 60.0);
        assert!(slapback(&wall, beside, beside).is_none());
    }
    use bevy::ecs::system::RunSystemOnce;

    fn app(map: &str) -> App {
        let opts = crate::LaunchOptions {
            robots: false,
            map: map.into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        app
    }

    fn probe_at(app: &mut App, head: Vec3) -> Space {
        app.world_mut()
            .run_system_once(
                move |spatial: SpatialQuery,
                      built: Query<(&GlobalTransform, &Surfaces)>,
                      lighting: Option<Res<Lighting>>| {
                    probe(&spatial, &built, lighting.as_deref(), head)
                },
            )
            .unwrap()
    }

    /// Wherever the probe stands, the reverb is sane: a tail no longer
    /// than 3.5 s, wet never louder than -8 dB.
    #[test]
    fn reverb_stays_in_bounds() {
        for kind in [
            SpaceKind::Open,
            SpaceKind::Sheltered,
            SpaceKind::Room,
            SpaceKind::Hall,
            SpaceKind::Underground,
        ] {
            for surface in [
                Surface::Metal,
                Surface::Hard,
                Surface::Masonry,
                Surface::Wood,
                Surface::Soft,
            ] {
                for (enclosed, size) in [(0.0, 40.0), (1.0, 2.0), (1.0, 40.0), (0.6, 10.0)] {
                    let r = Space {
                        kind,
                        enclosed,
                        size,
                        length: size * 2.0,
                        surface,
                        roof: None,
                    }
                    .reverb();
                    assert!((0.3..=3.5).contains(&r.rt60), "{r:?}");
                    assert!(r.wet_db <= -8.0, "{r:?}");
                }
            }
        }
    }

    /// The places the brief named: the open valley is dry, a building's
    /// inside is a room or a hall with a roof, underground is underground.
    #[test]
    fn places_sound_like_their_size() {
        let mut app = app("marrow_creek");
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let field = zone
            .marker(&extraction_core::map::MarkerKind::Insertion)
            .map(|m| zone.on_ground(m.at[0], m.at[1]))
            .unwrap();
        let open = probe_at(&mut app, field + Vec3::Y * 1.6);
        assert_eq!(open.kind, SpaceKind::Open, "{open:?}");
        assert_eq!(open.reverb().wet_db, DRY);

        // Inside every building with doors: roofed, and some reverb.
        let shells: Vec<_> = app
            .world()
            .resource::<crate::robots::doors::Entrances>()
            .shells
            .clone();
        assert!(!shells.is_empty());
        let mut indoors = 0;
        let mut checked = 0;
        for s in shells.iter().take(12) {
            let floor = s.xf.translation;
            let head = floor + Vec3::Y * 1.6;
            let space = probe_at(&mut app, head);
            checked += 1;
            if matches!(
                space.kind,
                SpaceKind::Room | SpaceKind::Hall | SpaceKind::Underground
            ) {
                indoors += 1;
                assert!(space.roof.is_some(), "{space:?}");
                assert!(space.reverb().wet_db > DRY, "{space:?}");
            }
        }
        assert!(
            indoors * 3 >= checked * 2,
            "{indoors} of {checked} building centres sound indoors"
        );

        // Underground: every site's rooms and corridors echo.
        use extraction_core::underground as ug;
        let mut rts = Vec::new();
        for p in zone
            .map
            .placements
            .iter()
            .filter(|p| ug::is_site(&p.prefab))
        {
            let xf = crate::world::buildings::placement_transform(&zone, p);
            let (l, r) = (
                ug::layout(&p.prefab).unwrap(),
                ug::raster(&p.prefab).unwrap(),
            );
            let mut heads: Vec<Vec3> = l
                .rooms
                .iter()
                .map(|room| Vec3::new(room.at[0], r.floor + 1.6, room.at[1]))
                .collect();
            if let Some(k) = r.cells.iter().position(|c| matches!(c, ug::Cell::Hall(_))) {
                let c = r.centre(k % r.nx, k / r.nx);
                heads.push(Vec3::new(c[0], r.floor + 1.6, c[1]));
            }
            for h in heads {
                let space = probe_at(&mut app, xf.transform_point(h));
                if space.size < 0.5 {
                    // Inside a wall: a corridor cell at the site's edge.
                    continue;
                }
                assert_eq!(
                    space.kind,
                    SpaceKind::Underground,
                    "{} {h}: {space:?}",
                    p.prefab
                );
                let rv = space.reverb();
                assert!(
                    rv.rt60 >= 0.8 && rv.wet_db >= -11.0,
                    "{} {h}: {space:?} {rv:?}",
                    p.prefab
                );
                rts.push(rv.rt60);
            }
        }
        assert!(!rts.is_empty(), "no underground site on the map");
        rts.sort_by(f32::total_cmp);
        assert!(rts[rts.len() / 2] >= 1.2, "underground echoes: {rts:?}");
    }

    /// A sound inside a building, heard from outside its door: through
    /// the wall while the door is shut, round the doorway when it's open.
    #[test]
    fn a_shut_door_muffles_and_an_open_one_lets_it_out() {
        let mut app = app("marrow_creek");
        let entrances: Vec<_> = app
            .world()
            .resource::<crate::robots::doors::Entrances>()
            .doors
            .clone();
        let occ = |app: &mut App, openings: Vec<Vec3>, from: Vec3, to: Vec3| {
            app.world_mut()
                .run_system_once(move |spatial: SpatialQuery| {
                    occlusion(&spatial, &openings, from, to)
                })
                .unwrap()
        };
        let (mut passed, mut failed) = (0, Vec::new());
        // Underground, a door leads to the next room, not out (and the
        // sites' doors would fill the sample).
        let entrances: Vec<_> = entrances
            .into_iter()
            .filter(|e| {
                !app.world()
                    .get::<crate::world::buildings::Building>(e.building)
                    .is_some_and(|b| extraction_core::underground::is_site(&b.prefab))
            })
            .collect();
        for e in entrances.iter().take(30) {
            // Shut it, let the leaf swing to.
            if let Some(mut d) = app
                .world_mut()
                .get_mut::<crate::world::buildings::Door>(e.door)
            {
                d.open = false;
            } else {
                continue;
            }
            for _ in 0..90 {
                app.update();
            }
            let outward = e.out.normalize_or_zero();
            let inside = e.mid - outward * 1.5 + Vec3::Y * 0.2;
            let outside = e.mid + outward * 5.0 + Vec3::Y * 1.4;
            let shut = occ(&mut app, vec![], inside, outside);
            if shut == CLEAR {
                // A doorway with no leaf, or a window in line: no test.
                continue;
            }
            assert_eq!(shut.db, wall_db(), "{e:?}");
            if let Some(mut d) = app
                .world_mut()
                .get_mut::<crate::world::buildings::Door>(e.door)
            {
                d.open = true;
            }
            for _ in 0..90 {
                app.update();
            }
            let open = occ(&mut app, vec![e.mid], inside, outside);
            // (A door whose way out runs into something else, a porch post
            // or the next building, stays muffled.)
            if open.db > shut.db && open.cutoff > shut.cutoff {
                passed += 1;
            } else {
                failed.push(*e);
            }
        }
        assert!(passed >= 5, "only {passed} doors let sound out");
        assert!(failed.len() * 3 <= passed, "{passed} passed; {failed:?}");
    }

    /// Rain is loudest in the open, duller under a roof, gone underground.
    #[test]
    fn rain_is_heard_through_the_roof() {
        let open = Space::default();
        let shed = Space {
            kind: SpaceKind::Room,
            enclosed: 1.0,
            size: 3.0,
            length: 4.0,
            surface: Surface::Wood,
            roof: Some(Roof::Tin),
        };
        let tunnel = Space {
            kind: SpaceKind::Underground,
            roof: Some(Roof::Solid),
            ..shed
        };
        let (o, oc, _) = rain_under(&open);
        let (s, sc, drum) = rain_under(&shed);
        let (t, _, _) = rain_under(&tunnel);
        assert!(o > s && s > t && sc < oc);
        assert_eq!(drum, Some("ambience/rain_tin"));
    }

    #[test]
    fn freeverb_room_follows_the_decay() {
        assert_eq!(freeverb_room(0.3), 0.0);
        assert!(freeverb_room(1.0) < freeverb_room(2.0));
        assert!(freeverb_room(3.5) < 0.9 && freeverb_room(3.5) > 0.8);
    }

    /// Through a wall a sound loses what robots say it loses; round an
    /// open doorway, less.
    #[test]
    fn walls_cost_what_robots_hear() {
        assert!((wall_db() - 20.0 * OCCLUDED.log10()).abs() < 1e-4);
        assert!(wall_db() < DOORWAY_DB && DOORWAY_DB < 0.0);
        assert!(WALL_CUTOFF < DOORWAY_CUTOFF);
    }

    /// Every map's places, probed: the open field and the forest are dry,
    /// metal buildings ring longer than wooden ones, underground echoes. Writes `target/audio_audit/acoustics.json`
    /// for the audio audit, which renders each place's reverb and
    /// measures it (`scripts/audio_mix.py`).
    #[test]
    fn every_place_is_probed_for_the_audit() {
        use extraction_core::underground as ug;
        let mut rows = Vec::new();
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            let mut app = app(map);
            let zone = app.world().resource::<crate::world::Zone>().clone();
            let mut put = |app: &mut App, place: String, head: Vec3| {
                let space = probe_at(app, head);
                let r = space.reverb();
                rows.push((map, place, space, r));
            };
            let ins = zone
                .marker(&extraction_core::map::MarkerKind::Insertion)
                .unwrap()
                .at;
            put(
                &mut app,
                "field".into(),
                zone.on_ground(ins[0], ins[1]) + Vec3::Y * 1.6,
            );
            // The densest forest within 150 m of the insertion.
            let mut best = (0.0, Vec3::ZERO);
            for i in -15..=15 {
                for j in -15..=15 {
                    let (x, z) = (ins[0] + i as f32 * 10.0, ins[1] + j as f32 * 10.0);
                    let c = zone.forest_cover(x, z);
                    if c > best.0 {
                        best = (c, zone.on_ground(x, z) + Vec3::Y * 1.6);
                    }
                }
            }
            if best.0 > 0.6 {
                put(&mut app, "forest".into(), best.1);
            }
            let shells: Vec<_> = app
                .world()
                .resource::<crate::robots::doors::Entrances>()
                .shells
                .clone();
            for sh in shells {
                let name = app
                    .world()
                    .get::<crate::world::buildings::Building>(sh.building)
                    .map(|b| b.prefab.clone())
                    .unwrap_or_default();
                if ug::is_site(&name) {
                    continue;
                }
                put(&mut app, name, sh.xf.translation + Vec3::Y * 1.6);
            }
            // Sallow's draw-off tower: inside at the bridge from the crest
            // (it has no door shell); and the crest itself.
            for p in zone
                .map
                .placements
                .iter()
                .filter(|p| p.prefab == "draw_off_tower" || p.prefab == "dam_face")
            {
                let xf = crate::world::buildings::placement_transform(&zone, p);
                let up = if p.prefab == "draw_off_tower" {
                    crate::world::dam::TOWER_BRIDGE + 1.6
                } else {
                    22.0 + 1.6
                };
                let name = if p.prefab == "dam_face" {
                    "dam_crest"
                } else {
                    "draw_off_tower"
                };
                put(&mut app, name.into(), xf.translation + Vec3::Y * up);
            }
            for p in zone
                .map
                .placements
                .iter()
                .filter(|p| ug::is_site(&p.prefab))
            {
                let xf = crate::world::buildings::placement_transform(&zone, p);
                let (l, r) = (
                    ug::layout(&p.prefab).unwrap(),
                    ug::raster(&p.prefab).unwrap(),
                );
                for room in &l.rooms {
                    let h = Vec3::new(room.at[0], r.floor + 1.6, room.at[1]);
                    put(
                        &mut app,
                        format!("{}/{}", p.prefab, room.id),
                        xf.transform_point(h),
                    );
                }
            }
        }
        let json: Vec<serde_json::Value> = rows
            .iter()
            .map(|(map, place, s, r)| {
                serde_json::json!({
                    "map": map, "place": place, "kind": format!("{:?}", s.kind),
                    "surface": format!("{:?}", s.surface), "roof": s.roof.map(|r| format!("{r:?}")),
                    "enclosed": s.enclosed, "size": s.size, "length": s.length,
                    "wet_db": r.wet_db, "rt60": r.rt60, "damping": r.damping,
                })
            })
            .collect();
        let dir = crate::asset_root_path().join("../target/audio_audit");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("acoustics.json"),
            serde_json::to_string_pretty(&json).unwrap(),
        )
        .unwrap();
        let mut wood = Vec::new();
        let mut metal = Vec::new();
        for (map, place, space, r) in &rows {
            if place == "field" || place == "forest" {
                assert!(r.wet_db <= -20.0, "{map} {place}: {space:?} {r:?}");
            }
            if matches!(space.kind, SpaceKind::Room | SpaceKind::Hall) {
                match space.surface {
                    Surface::Wood => wood.push(r.rt60),
                    Surface::Metal => metal.push(r.rt60),
                    _ => {}
                }
            }
        }
        let median = |v: &mut Vec<f32>| {
            v.sort_by(f32::total_cmp);
            v.get(v.len() / 2).copied()
        };
        if let (Some(w), Some(m)) = (median(&mut wood), median(&mut metal)) {
            assert!(m > w * 1.5, "metal rooms {metal:?} vs wooden {wood:?}");
        }
    }
}
