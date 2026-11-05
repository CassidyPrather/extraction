//! Signs that someone's home, seen from far off (`MapDef::cues`): a
//! window lit warm after dark, and a thin plume of smoke from a chimney,
//! bent by the wind. Breadcrumbs and the way home at night are found by
//! them (`docs/MAP_DESIGN.md`).
//!
//! Each cue dresses a building already placed: the one nearest its `at`.
//! A lit window fills that building's window nearest `at` with a glow
//! just inside the glass, facing out (from inside the room it isn't
//! there: only the lamp, if it has one, lights the room). Smoke rises from
//! the building's stack (`buildings::Geo::flues`), or else from a
//! stovepipe added up through the roof over its stove, or from a height
//! the map gives.
//!
//! The plume is a handful of soft puffs (`render::puff`) on a loop: each
//! rises, swells, leans downwind and thins away, and comes round again at
//! the top of the stack. Its alpha is one of a few shared materials, so a
//! plume writes no material while it plays.
//!
//! `EXTRACTION_CUES` adds cues for a look without editing a map:
//! `window:X,Z[,light][,always];smoke:X,Z[,Y]`.

use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use extraction_core::map::Cue;

use super::Zone;
use super::buildings::{Geo, Shape, Sill};
use super::weather::Conditions;
use crate::render::materials::Mat;
use crate::render::puff::{self, PuffAssets};

pub struct CuesPlugin;

impl Plugin for CuesPlugin {
    fn build(&self, app: &mut App) {
        puff::add_to(app);
        app.add_systems(Startup, place_cues)
            .add_systems(Update, (switch_windows, play_smoke));
    }
}

/// How far a cue's `at` may be from the middle of the building it
/// dresses (m): a farmhouse's far corner is 6.4 m from its middle.
pub const REACH: f32 = 20.0;

/// The glow's colour and strength (linear, before the eye's exposure):
/// a lamp behind a curtain, dimmer and warmer than the camp's lantern
/// (6, 3.2, 1).
const GLOW: LinearRgba = LinearRgba::rgb(2.6, 1.25, 0.42);
/// How far into the room from the wall's centre line the glow stands
/// (m): behind the glass and the sash's bars, which stand dark against it.
const GLOW_IN: f32 = 0.06;
/// A window's lamp: a small one, well into the room.
const LAMP_LUMENS: f32 = 5_000.0;
const LAMP_RANGE: f32 = 6.0;

/// A stovepipe's radius (the stove's own: `Builder::stove`), and how far
/// it stands above the roof.
const PIPE_R: f32 = 0.07;
const PIPE_PROUD: f32 = 0.7;

/// The plume: so many puffs, each living so long (s).
pub const PUFFS: usize = 24;
pub const LIFE: f32 = 18.0;
/// Its thickest (a puff's heart, just clear of the pipe).
const SMOKE_ALPHA: f32 = 0.6;
/// Wood smoke, lit by the sky: a pale blue-grey.
const SMOKE: Color = Color::srgb(0.8, 0.81, 0.84);
/// Shared alpha steps for the puffs (`SmokeLook`).
const STEPS: usize = 8;

/// The smoke's colour at alpha step `k` (1..=[`STEPS`]) in the hour's
/// `light`. Unlit, as every puff is (lit, a far thin one came out black):
/// pale by day, dimmed with the light so it doesn't glow after dark.
fn smoke_tint(k: usize, light: f32) -> Color {
    // (Dimmer than the light's own fall: the eyes' adaptation lifts it.)
    let c = SMOKE.to_linear() * light.clamp(0.0, 1.0).powf(1.7).max(0.004);
    Color::from(c).with_alpha(SMOKE_ALPHA * k as f32 / STEPS as f32)
}

/// Where a cue goes, worked out from the building it dresses.
#[derive(Clone, Debug, PartialEq)]
pub enum Resolved {
    /// A window: its sill in the world (the wall's +Z into the room), its
    /// width and height.
    Window {
        sill: Transform,
        width: f32,
        height: f32,
        light: bool,
        always: bool,
    },
    /// Smoke from `top`; `pipe`: a stovepipe to add, from its foot under
    /// the roof to `top`.
    Smoke { top: Vec3, pipe: Option<Vec3> },
}

/// The cues the map asks for, and any from `EXTRACTION_CUES`.
pub fn cues(zone: &Zone) -> Vec<Cue> {
    let mut out = zone.map.cues.clone();
    if let Ok(v) = std::env::var("EXTRACTION_CUES") {
        out.extend(parse(&v));
    }
    out
}

/// `window:X,Z[,light][,always];smoke:X,Z[,Y]`.
pub fn parse(s: &str) -> Vec<Cue> {
    s.split(';')
        .filter_map(|one| {
            let (kind, rest) = one.trim().split_once(':')?;
            let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
            let num = |i: usize| parts.get(i).and_then(|p| p.parse::<f32>().ok());
            let at = [num(0)?, num(1)?];
            match kind {
                "window" => Some(Cue::LitWindow {
                    at,
                    light: parts.contains(&"light"),
                    always: parts.contains(&"always"),
                }),
                "smoke" => Some(Cue::Smoke { at, y: num(2) }),
                _ => None,
            }
        })
        .collect()
}

/// The placed building nearest `at` (within [`REACH`]), its recipe and
/// its pose.
fn building_at(zone: &Zone, at: [f32; 2]) -> Option<(Geo, Transform)> {
    zone.map
        .placements
        .iter()
        .map(|p| (p, (p.at[0] - at[0]).hypot(p.at[1] - at[1])))
        .filter(|(_, d)| *d <= REACH)
        .filter_map(|(p, d)| super::buildings::prefab(&p.prefab).map(|g| (g, p, d)))
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(g, p, _)| (g, super::buildings::placement_transform(zone, p)))
}

/// Where `cue` goes, or why it can't.
pub fn resolve(zone: &Zone, cue: &Cue) -> Result<Resolved, String> {
    let at = cue.at();
    let flat = |v: Vec3| (v.x - at[0]).hypot(v.z - at[1]);
    if let Cue::Smoke { y: Some(y), .. } = cue {
        let top = Vec3::new(at[0], zone.ground(at[0], at[1]) + y, at[1]);
        return Ok(Resolved::Smoke { top, pipe: None });
    }
    let (geo, t) =
        building_at(zone, at).ok_or_else(|| format!("no building within {REACH} m of {at:?}"))?;
    match *cue {
        Cue::LitWindow { light, always, .. } => {
            let sill: &Sill = geo
                .sills
                .iter()
                .min_by(|a, b| flat(t * a.at).total_cmp(&flat(t * b.at)))
                .ok_or_else(|| format!("the building at {at:?} has no windows"))?;
            Ok(Resolved::Window {
                sill: t * Transform::from_translation(sill.at).with_rotation(sill.rot),
                width: sill.len,
                height: sill.height,
                light,
                always,
            })
        }
        Cue::Smoke { .. } => {
            if let Some(f) = geo
                .flues
                .iter()
                .min_by(|a, b| flat(t * **a).total_cmp(&flat(t * **b)))
            {
                return Ok(Resolved::Smoke {
                    top: t * *f,
                    pipe: None,
                });
            }
            let (foot, roof) = stovepipe(&geo)
                .ok_or_else(|| format!("the building at {at:?} has no stack and no stove"))?;
            let top = Vec3::new(foot.x, roof + PIPE_PROUD, foot.z);
            Ok(Resolved::Smoke {
                top: t * top,
                pipe: Some(t * foot),
            })
        }
    }
}

/// A building's stove's pipe (`Builder::stove`: a dark-metal cylinder
/// 7 cm round): its top, under the ceiling, and the height of the roof's
/// outer face straight over it, building-local.
pub fn stovepipe(geo: &Geo) -> Option<(Vec3, f32)> {
    let pipe = geo.pieces.iter().find(|p| {
        matches!(p.shape, Shape::Cylinder(r, h) if r == PIPE_R && h == 1.6)
            && p.mat == Mat::DarkMetal
    })?;
    let Shape::Cylinder(_, h) = pipe.shape else {
        return None;
    };
    let foot = pipe.at.translation + Vec3::Y * h * 0.5;
    let roof = top_over(geo, foot.x, foot.z)?;
    (roof > foot.y).then_some((foot, roof))
}

/// The highest face of any of `geo`'s boxes straight over (x, z),
/// building-local: a ray down from above, through each box's own frame.
pub fn top_over(geo: &Geo, x: f32, z: f32) -> Option<f32> {
    const FROM: f32 = 200.0;
    let mut best: Option<f32> = None;
    for p in &geo.pieces {
        let Shape::Box(size) = p.shape else {
            continue;
        };
        let inv = p.at.compute_affine().inverse();
        let o = inv.transform_point3(Vec3::new(x, FROM, z));
        let d = inv.transform_vector3(Vec3::NEG_Y);
        let half = size * 0.5;
        let (mut t0, mut t1) = (f32::NEG_INFINITY, f32::INFINITY);
        let mut hit = true;
        for k in 0..3 {
            if d[k].abs() < 1e-6 {
                if o[k].abs() > half[k] {
                    hit = false;
                    break;
                }
                continue;
            }
            let (a, b) = ((-half[k] - o[k]) / d[k], (half[k] - o[k]) / d[k]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        if hit && t0 <= t1 && t0 >= 0.0 {
            let y = FROM - t0;
            best = Some(best.map_or(y, |b: f32| b.max(y)));
        }
    }
    best
}

/// A window's glow, and whether it's lit by day too.
#[derive(Component)]
pub struct LitWindow {
    pub always: bool,
}

/// One of a plume's puffs: its place in the loop.
#[derive(Component)]
pub struct SmokePuff {
    pub top: Vec3,
    pub phase: f32,
    pub seed: f32,
}

/// The plumes' shared materials, thinnest first.
#[derive(Resource)]
struct SmokeLook(Vec<Handle<StandardMaterial>>);

/// Whether a window is lit now: from dusk to dawn, or always.
pub fn lit(conditions: &Conditions, always: bool) -> bool {
    always || super::halos::halos_show(conditions)
}

/// The curtain: drawn back to either side, a little darker toward the
/// sill, so the glow is a room's light and not a lit slab. White, its
/// value the glow's strength across the pane.
fn curtain_image() -> Image {
    const W: u32 = 16;
    const H: u32 = 16;
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for j in 0..H {
        for i in 0..W {
            let u = (i as f32 + 0.5) / W as f32;
            let v = (j as f32 + 0.5) / H as f32; // 0 at the head
            let edge = (u.min(1.0 - u) / 0.2).clamp(0.0, 1.0);
            let k = (0.3 + 0.7 * edge * edge) * (1.0 - 0.3 * v);
            let c = (k.clamp(0.0, 1.0) * 255.0).round() as u8;
            data.extend_from_slice(&[c, c, c, 255]);
        }
    }
    Image::new(
        Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[allow(clippy::too_many_arguments)]
fn place_cues(
    mut commands: Commands,
    zone: Res<Zone>,
    conditions: Res<Conditions>,
    puffs: Res<PuffAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let wanted = cues(&zone);
    if wanted.is_empty() {
        return;
    }
    let glow = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: GLOW,
        emissive_texture: Some(images.add(curtain_image())),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let pipe_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.13, 0.12, 0.11),
        perceptual_roughness: 0.7,
        metallic: 0.6,
        ..default()
    });
    let look: Vec<Handle<StandardMaterial>> = (1..=STEPS)
        .map(|k| materials.add(puffs.material(smoke_tint(k, conditions.light), false)))
        .collect();
    for (n, cue) in wanted.iter().enumerate() {
        let r = match resolve(&zone, cue) {
            Ok(r) => r,
            Err(why) => {
                warn!("cue {cue:?}: {why}");
                continue;
            }
        };
        match r {
            Resolved::Window {
                sill,
                width,
                height,
                light,
                always,
            } => {
                let (w, h) = ((width - 0.04).max(0.1), (height - 0.06).max(0.1));
                // The rectangle faces +Z; turned to face out of the room.
                let at = sill.translation + sill.rotation * Vec3::new(0.0, height * 0.5, GLOW_IN);
                let on = lit(&conditions, always);
                let vis = if on {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                let mut e = commands.spawn((
                    Name::new("Lit window"),
                    LitWindow { always },
                    Mesh3d(meshes.add(Rectangle::new(w, h))),
                    MeshMaterial3d(glow.clone()),
                    crate::render::grade::ToTheEye {
                        material: glow.clone(),
                        emissive: GLOW,
                    },
                    Transform::from_translation(at)
                        .with_rotation(sill.rotation * Quat::from_rotation_y(std::f32::consts::PI)),
                    vis,
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
                if light {
                    // Well into the room and up by the ceiling, a lamp on
                    // a table or a hook: it lights the room, and its halo
                    // shows only through the window.
                    // (The glow is turned to face out: the room is its -Z.)
                    let lamp = Vec3::new(0.0, height * 0.2, -(1.4 - GLOW_IN));
                    e.with_child((
                        Name::new("Window lamp"),
                        PointLight {
                            color: Color::srgb(1.0, 0.72, 0.42),
                            intensity: LAMP_LUMENS,
                            range: LAMP_RANGE,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_translation(lamp),
                    ));
                }
            }
            Resolved::Smoke { top, pipe } => {
                if let Some(foot) = pipe {
                    let len = top.y - foot.y;
                    commands.spawn((
                        Name::new("Stovepipe"),
                        Mesh3d(meshes.add(Cylinder::new(PIPE_R, len).mesh().resolution(12))),
                        MeshMaterial3d(pipe_mat.clone()),
                        Transform::from_translation((foot + top) * 0.5),
                    ));
                }
                let seed = n as f32 * 1.7;
                for i in 0..PUFFS {
                    commands.spawn((
                        Name::new("Chimney smoke"),
                        SmokePuff {
                            top,
                            phase: i as f32 / PUFFS as f32 * LIFE,
                            seed: seed + i as f32 * 0.61,
                        },
                        puff::bundle(&puffs, look[0].clone()),
                        Transform::from_translation(top).with_scale(Vec3::splat(0.0)),
                        NotShadowCaster,
                        NotShadowReceiver,
                    ));
                }
            }
        }
    }
    commands.insert_resource(SmokeLook(look));
}

/// Windows on from dusk to dawn (the hour can change in a raid).
fn switch_windows(conditions: Res<Conditions>, mut windows: Query<(&LitWindow, &mut Visibility)>) {
    if !conditions.is_changed() {
        return;
    }
    for (w, mut vis) in &mut windows {
        let want = if lit(&conditions, w.always) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
}

/// A puff `age` s out of the pipe: its offset from the top, its scale
/// (the puff quad's, 1 m across its body at 1) and its alpha. `wind` is
/// the air's drift (m/s, level).
pub fn puff_at(age: f32, seed: f32, wind: Vec3) -> (Vec3, f32, f32) {
    let u = (age / LIFE).clamp(0.0, 1.0);
    // Hot at the pipe, it climbs and slows as it cools: about 9 m up.
    let rise = 9.0 * (1.0 - (-age / 8.0).exp());
    // It leans downwind more the higher it gets (still air drifts a little).
    let drift = (wind + Vec3::new(0.24, 0.0, 0.18)) * age * 0.4;
    let wobble = Vec3::new(
        (age * 0.6 + seed).sin(),
        0.0,
        (age * 0.5 + seed * 1.3).cos(),
    ) * (0.08 + 0.25 * u);
    let scale = 0.7 + 2.5 * u.powf(0.8);
    let alpha = (age / 1.0).min(1.0) * (1.0 - u).powf(1.3);
    (Vec3::Y * rise + drift + wobble, scale, alpha)
}

fn play_smoke(
    time: Res<Time>,
    conditions: Option<Res<Conditions>>,
    look: Option<Res<SmokeLook>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut puffs: Query<(
        &SmokePuff,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some(look) = look else {
        return;
    };
    if let Some(c) = conditions.as_ref().filter(|c| c.is_changed()) {
        for (k, h) in look.0.iter().enumerate() {
            if let Some(mut m) = materials.get_mut(h) {
                // (As `PuffAssets::soften` made it: its heart thicker.)
                let t = smoke_tint(k + 1, c.light);
                m.base_color = t.with_alpha((t.alpha() * puff::HEART).min(1.0));
            }
        }
    }
    let wind = crate::weapons::impacts::wind_velocity(conditions.as_deref());
    let t = time.elapsed_secs();
    for (p, mut tf, mut mat) in &mut puffs {
        let age = (t + p.phase) % LIFE;
        let (off, scale, alpha) = puff_at(age, p.seed, wind);
        tf.translation = p.top + off;
        tf.scale = Vec3::splat(scale);
        let step = ((alpha * STEPS as f32).round() as usize).clamp(1, STEPS) - 1;
        if mat.0 != look.0[step] {
            mat.0 = look.0[step].clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every cue in every valley finds what it dresses: a window near
    /// where the map put it, a stack or a stove under a roof.
    #[test]
    fn every_cue_finds_its_building() {
        let mut bad = Vec::new();
        for (id, _) in extraction_core::map::VALLEYS {
            let zone = Zone::load(id);
            for cue in &zone.map.cues {
                match resolve(&zone, cue) {
                    Ok(Resolved::Window { sill, .. }) => {
                        let at = cue.at();
                        let d = (sill.translation.x - at[0]).hypot(sill.translation.z - at[1]);
                        if d > 4.0 {
                            bad.push(format!("{id}: {cue:?}: its window is {d:.1} m off"));
                        }
                    }
                    Ok(_) => {}
                    Err(why) => bad.push(format!("{id}: {cue:?}: {why}")),
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// The houses the cues were asked for have windows, and a stack or a
    /// stove whose pipe can go up through the roof.
    #[test]
    fn the_homes_can_smoke_and_light_up() {
        for id in [
            "farmhouse",
            "house_plank",
            "house_plank_b",
            "house_plank_c",
            "house_brick",
            "cabin_log",
            "pump_station",
        ] {
            let geo = super::super::buildings::prefab(id).unwrap();
            assert!(!geo.sills.is_empty(), "{id}: no windows");
            assert!(
                geo.sills.iter().all(|s| s.height > 0.9 && s.len > 0.5),
                "{id}: a window's size is off"
            );
            if geo.flues.is_empty() {
                let (foot, roof) = stovepipe(&geo).unwrap_or_else(|| panic!("{id}: no stove"));
                // Up through a ceiling and a roof, not a mile of pipe.
                assert!(
                    (0.5..3.5).contains(&(roof - foot.y)),
                    "{id}: {:.2} m from the pipe to the roof",
                    roof - foot.y
                );
            }
        }
    }

    /// The plume starts thin at the pipe, rises and swells, leans
    /// downwind, and thins to nothing as it comes round again.
    #[test]
    fn the_plume_rises_leans_and_thins() {
        let wind = Vec3::new(1.0, 0.0, 0.0);
        let (o0, s0, a0) = puff_at(0.0, 0.0, wind);
        let (o1, s1, a1) = puff_at(LIFE * 0.5, 0.0, wind);
        let (_, _, a2) = puff_at(LIFE * 0.999, 0.0, wind);
        assert!(o0.length() < 0.2 && a0 < 0.01 && s0 < 0.8);
        assert!(o1.y > 4.0 && o1.x > 3.0 && s1 > 1.0 && a1 > 0.1);
        assert!(a2 < 0.01);
        // Its thickest puff is still a thin smoke, not a wall.
        let peak = (0..100)
            .map(|i| puff_at(i as f32 * LIFE / 100.0, 0.0, wind).2)
            .fold(0.0f32, f32::max);
        assert!(peak <= 1.0 && SMOKE_ALPHA * peak < 0.75);
    }

    #[test]
    fn cues_parse_from_the_env() {
        let c = parse("window:1,2,light;smoke:3.5,-4;smoke:5,6,2.5;bad:1");
        assert_eq!(
            c,
            vec![
                Cue::LitWindow {
                    at: [1.0, 2.0],
                    light: true,
                    always: false
                },
                Cue::Smoke {
                    at: [3.5, -4.0],
                    y: None
                },
                Cue::Smoke {
                    at: [5.0, 6.0],
                    y: Some(2.5)
                },
            ]
        );
    }
}
