//! Ground cover (`extraction_core::cover`), streamed round the camera:
//! grass tufts as real blades (no texture, coloured base to tip), sedge on
//! wet ground, and fern clumps of arching fronds cut from the fern atlas.
//! One merged mesh per patch per material, dithered out with distance.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::VisibilityRange;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use extraction_core::cover::{CoverKind, CoverMap, CoverSpot};

use super::Zone;

/// Patch size, metres.
const PATCH: f32 = 16.0;
/// Patches within this of the camera are built...
const SPAWN: f32 = 52.0;
/// ...and dropped past this.
const DROP: f32 = 64.0;
/// Plants thin out (dithered) between these...
const FADE: std::ops::Range<f32> = 34.0..46.0;
/// ...and most of the grass already goes between these: only this share
/// of the tufts (by rank) is kept out to `FADE`.
const NEAR_FADE: std::ops::Range<f32> = 16.0..24.0;
const KEEP_FAR: f32 = 0.3;

/// The fern atlas's fronds, `[u0, v0, u1, v1]`, base at `v1`.
const FRONDS: [[f32; 4]; 5] = [
    [0.089, 0.111, 0.235, 0.948],
    [0.313, 0.024, 0.455, 0.764],
    [0.508, 0.026, 0.658, 0.5],
    [0.513, 0.511, 0.629, 0.965],
    [0.714, 0.109, 0.837, 0.896],
];

pub struct CoverPlugin;

impl Plugin for CoverPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SwayMaterial>::default())
            .add_systems(Update, (stream_cover, blow));
    }
}

/// Ground cover's material: standard PBR whose vertices sway in the wind
/// and lean away from the player's legs (`shaders/sway.wgsl`).
pub type SwayMaterial = bevy::pbr::ExtendedMaterial<StandardMaterial, Sway>;

#[derive(Asset, bevy::render::render_resource::AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct Sway {
    /// xy: the wind's direction (world x/z); z: strength 0..1; w: gusts.
    #[uniform(100)]
    pub wind: Vec4,
    /// xyz: the player's feet; w: how far round them plants lean away.
    #[uniform(100)]
    pub pusher: Vec4,
}

impl bevy::pbr::MaterialExtension for Sway {
    fn vertex_shader() -> bevy::shader::ShaderRef {
        "shaders/sway.wgsl".into()
    }
}

/// The wind as the weather has it (still in thick fog, hard in a
/// downpour), and where the player is standing.
fn blow(
    assets: Option<Res<CoverAssets>>,
    mut materials: ResMut<Assets<SwayMaterial>>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
) {
    let Some(assets) = assets else {
        return;
    };
    let strength = conditions.map_or(0.4, |c| c.wind());
    let feet = cams
        .iter()
        .next()
        .map_or(Vec3::splat(1e6), |g| g.translation() - Vec3::Y * 1.6);
    for h in [&assets.blades, &assets.fronds] {
        if let Some(mut m) = materials.get_mut(h) {
            m.extension.wind = Vec4::new(0.8, 0.6, strength, 0.8);
            m.extension.pusher = feet.extend(0.6);
        }
    }
}

/// A patch's meshes (centred) and which material and fade each takes.
type Built = Vec<(
    Mesh,
    bevy::camera::primitives::Aabb,
    Transform,
    bool,
    std::ops::Range<f32>,
)>;

#[derive(Resource)]
struct CoverAssets {
    cover: std::sync::Arc<CoverMap>,
    /// Patches being built on the task pool.
    building: std::collections::HashMap<(i32, i32), bevy::tasks::Task<Built>>,
    blades: Handle<SwayMaterial>,
    fronds: Handle<SwayMaterial>,
    /// Built patches and their entities.
    built: std::collections::HashMap<(i32, i32), Vec<Entity>>,
}

fn rng(seed: u32) -> extraction_core::noise::Rng {
    extraction_core::noise::Rng::new(u64::from(seed).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x5EED)
}

fn lin(c: Color) -> [f32; 4] {
    c.to_linear().to_f32_array()
}

#[derive(Default)]
struct Builder {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    /// Per vertex: how free it is in the wind (0 root, 1 tip) and the
    /// plant's phase (`sway.wgsl`).
    sway: Vec<[f32; 2]>,
    idx: Vec<u32>,
    /// The phase of the plant being built.
    phase: f32,
}

impl Builder {
    fn vertex(&mut self, p: Vec3, n: Vec3, uv: [f32; 2], c: [f32; 4]) -> u32 {
        self.pos.push(p.to_array());
        self.nrm.push(n.to_array());
        self.uv.push(uv);
        self.col.push(c);
        self.sway.push([0.0, self.phase]);
        (self.pos.len() - 1) as u32
    }

    /// A strip along `spine` (centre points), `width` across at each, `side`
    /// the across direction at each; `v` runs 0 at the base to 1 at the tip.
    fn strip(
        &mut self,
        spine: &[(Vec3, Vec3, f32)],
        n: Vec3,
        uv: impl Fn(f32, f32) -> [f32; 2],
        col: impl Fn(f32) -> [f32; 4],
    ) {
        let last = spine.len() - 1;
        let mut prev: Option<(u32, u32)> = None;
        for (k, (p, side, w)) in spine.iter().enumerate() {
            let t = k as f32 / last as f32;
            let a = self.vertex(*p - *side * *w, n, uv(0.0, t), col(t));
            let b = self.vertex(*p + *side * *w, n, uv(1.0, t), col(t));
            for v in [a, b] {
                self.sway[v as usize][0] = t * t;
            }
            if let Some((pa, pb)) = prev {
                self.idx.extend([pa, pb, b, pa, b, a]);
            }
            prev = Some((a, b));
        }
    }

    fn build(self) -> Option<Mesh> {
        if self.idx.is_empty() {
            return None;
        }
        Some(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.sway)
            .with_inserted_indices(Indices::U32(self.idx)),
        )
    }
}

/// A tuft: a handful of blades leaning out from a point, dark at the root,
/// paler (or straw, now and then) at the tips.
fn tuft(b: &mut Builder, s: &CoverSpot, burnt: bool) {
    let mut r = rng(s.seed);
    b.phase = r.f32();
    let base = Vec3::from_array(s.pos);
    let sedge = s.kind == CoverKind::Sedge;
    let blades = if sedge { 10 } else { 6 + r.below(5) };
    let (tall, width) = if sedge { (0.55, 0.014) } else { (0.27, 0.011) };
    let dry = r.f32();
    let root = lin(Color::srgb(0.16, 0.17, 0.09));
    let tip = if burnt {
        // Scorched: grey-brown and straw, a little green coming back.
        if dry < 0.8 {
            lin(Color::srgb(0.44, 0.4, 0.33))
        } else {
            lin(Color::srgb(0.38, 0.4, 0.26))
        }
    } else if sedge {
        lin(Color::srgb(0.36, 0.39, 0.22))
    } else if dry < 0.3 {
        lin(Color::srgb(0.58, 0.53, 0.36))
    } else {
        lin(Color::srgb(0.4 + 0.12 * dry, 0.45, 0.25))
    };
    for k in 0..blades {
        let a = s.yaw + k as f32 * std::f32::consts::TAU / blades as f32 + r.range(-0.4, 0.4);
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let side = Vec3::new(-a.sin(), 0.0, a.cos());
        let h = tall * s.scale * r.range(0.6, 1.15);
        let lean = h * r.range(0.12, 0.5);
        let foot = base + out * r.range(0.0, 0.07);
        let spine: Vec<(Vec3, Vec3, f32)> = (0..3)
            .map(|i| {
                let t = i as f32 / 2.0;
                (
                    foot + Vec3::Y * h * t + out * lean * t * t,
                    side,
                    width * s.scale * (1.0 - t * 0.9),
                )
            })
            .collect();
        // Lit as if it faced the sky, so a tuft isn't dark on one side.
        let n = (Vec3::Y + out * 0.35).normalize();
        b.strip(
            &spine,
            n,
            |u, v| [u, v],
            |t| {
                let mut c = [1.0; 4];
                for i in 0..3 {
                    c[i] = root[i] + (tip[i] - root[i]) * t.powf(0.8);
                }
                c
            },
        );
    }
}

/// A fern: fronds rising from the crown and arching over.
fn fern(b: &mut Builder, s: &CoverSpot) {
    let mut r = rng(s.seed);
    b.phase = r.f32();
    let base = Vec3::from_array(s.pos) + Vec3::Y * 0.02;
    let fronds = 5 + r.below(4);
    for k in 0..fronds {
        let a = s.yaw + k as f32 * std::f32::consts::TAU / fronds as f32 + r.range(-0.3, 0.3);
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let side = Vec3::new(-a.sin(), 0.0, a.cos());
        let rect = FRONDS[r.below(FRONDS.len())];
        let len = r.range(0.55, 0.85) * s.scale;
        // Its width, true to the atlas's proportions (and a touch fuller).
        let w = len * (rect[2] - rect[0]) / (rect[3] - rect[1]) * 0.65;
        let rise = r.range(0.9, 1.25);
        let mut p = base;
        let segs = 5;
        let mut spine = Vec::new();
        for i in 0..=segs {
            let t = i as f32 / segs as f32;
            spine.push((p, side, w * (0.6 + 0.4 * (1.0 - (2.0 * t - 1.0).powi(2)))));
            // Up at first, over and down by the tip.
            let elev = rise - 1.7 * t;
            p += (out * elev.cos() + Vec3::Y * elev.sin()) * (len / segs as f32);
        }
        let tint = 0.85 + 0.2 * r.f32();
        b.strip(
            &spine,
            (Vec3::Y + out * 0.2).normalize(),
            |u, v| {
                [
                    rect[0] + (rect[2] - rect[0]) * u,
                    rect[3] + (rect[1] - rect[3]) * v,
                ]
            },
            |_| [tint, tint, tint * 0.95, 1.0],
        );
    }
}

fn patch_of(p: Vec3) -> (i32, i32) {
    ((p.x / PATCH).floor() as i32, (p.z / PATCH).floor() as i32)
}

fn stream_cover(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    assets: Option<ResMut<CoverAssets>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SwayMaterial>>,
    server: Res<AssetServer>,
    time: Res<Time>,
    mut timer: Local<f32>,
    opts: Res<crate::launch::LaunchOptions>,
) {
    let Some(zone) = zone else {
        return;
    };
    let Some(mut assets) = assets else {
        let fern_tex: Handle<Image> = server
            .load_builder()
            .with_settings(|s: &mut bevy::image::ImageLoaderSettings| {
                s.is_srgb = true;
            })
            .load("textures/foliage/fern/albedo.ktx2");
        commands.insert_resource(CoverAssets {
            cover: std::sync::Arc::new(CoverMap::new(&zone.map, &zone.hm)),
            building: Default::default(),
            blades: materials.add(SwayMaterial {
                base: StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.85,
                    reflectance: 0.15,
                    double_sided: true,
                    cull_mode: None,
                    diffuse_transmission: 0.2,
                    ..default()
                },
                extension: Sway::default(),
            }),
            fronds: materials.add(SwayMaterial {
                base: StandardMaterial {
                    base_color: Color::srgb(0.85, 0.9, 0.78),
                    base_color_texture: Some(fern_tex),
                    alpha_mode: AlphaMode::Mask(0.45),
                    perceptual_roughness: 0.8,
                    reflectance: 0.2,
                    double_sided: true,
                    cull_mode: None,
                    diffuse_transmission: 0.3,
                    ..default()
                },
                extension: Sway::default(),
            }),
            built: Default::default(),
        });
        return;
    };
    // Finished patches go in every frame (the work was done elsewhere).
    let assets = &mut *assets;
    let done: Vec<(i32, i32)> = assets
        .building
        .iter()
        .filter(|(_, t)| t.is_finished())
        .map(|(k, _)| *k)
        .collect();
    for key in done {
        let task = assets.building.remove(&key).unwrap();
        let parts = bevy::tasks::block_on(task);
        let ents = parts
            .into_iter()
            .map(|(mesh, aabb, at, fronds, fade)| {
                commands
                    .spawn((
                        Name::new("Ground cover"),
                        Mesh3d(meshes.add(mesh)),
                        aabb,
                        at,
                        MeshMaterial3d(if fronds {
                            assets.fronds.clone()
                        } else {
                            assets.blades.clone()
                        }),
                        VisibilityRange {
                            start_margin: 0.0..0.0,
                            end_margin: fade,
                            use_aabb: true,
                        },
                        bevy::light::NotShadowCaster,
                    ))
                    .id()
            })
            .collect();
        assets.built.insert(key, ents);
    }
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.2;
    let eyes: Vec<Vec2> = cams.iter().map(|g| g.translation().xz()).collect();
    if eyes.is_empty() {
        return;
    }
    // Horizontal distance from the nearest eye to a patch's square.
    let reach = |(i, j): (i32, i32)| {
        let lo = Vec2::new(i as f32, j as f32) * PATCH;
        eyes.iter()
            .map(|e| e.clamp(lo, lo + Vec2::splat(PATCH)).distance(*e))
            .fold(f32::INFINITY, f32::min)
    };
    assets.building.retain(|key, _| reach(*key) < DROP);
    assets.built.retain(|key, ents| {
        let keep = reach(*key) < DROP;
        if !keep {
            for e in ents.iter() {
                commands.entity(*e).despawn();
            }
        }
        keep
    });
    // Build the nearest missing patch or two per tick, not all at once.
    let n = (SPAWN / PATCH).ceil() as i32 + 1;
    let mut wanted: Vec<((i32, i32), f32)> = Vec::new();
    for e in &eyes {
        let (ci, cj) = patch_of(Vec3::new(e.x, 0.0, e.y));
        for j in cj - n..=cj + n {
            for i in ci - n..=ci + n {
                let d = reach((i, j));
                if d < SPAWN
                    && !assets.built.contains_key(&(i, j))
                    && !assets.building.contains_key(&(i, j))
                    && !wanted.iter().any(|w| w.0 == (i, j))
                {
                    wanted.push(((i, j), d));
                }
            }
        }
    }
    wanted.sort_by(|a, b| a.1.total_cmp(&b.1));
    // Built off the main thread: a patch is a couple of milliseconds of
    // work, and several at once would drop frames in the headset.
    let pool = bevy::tasks::AsyncComputeTaskPool::get();
    let detail = opts.detail;
    let burnt = assets.cover.burnt;
    for (key, _) in wanted.into_iter().take(6) {
        let cover = assets.cover.clone();
        let hm = zone.hm.clone();
        let task = pool.spawn(async move {
            let lo = [key.0 as f32 * PATCH, key.1 as f32 * PATCH];
            let plants = cover.patch(&hm, lo, PATCH);
            let (mut near, mut far, mut fronds) =
                (Builder::default(), Builder::default(), Builder::default());
            for s in &plants {
                // Lower detail keeps fewer plants (by rank, so the same ones).
                if s.rank > detail {
                    continue;
                }
                match s.kind {
                    CoverKind::Fern => fern(&mut fronds, s),
                    _ if s.rank < KEEP_FAR => tuft(&mut far, s, burnt),
                    _ => tuft(&mut near, s, burnt),
                }
            }
            [
                (near, false, NEAR_FADE),
                (far, false, FADE),
                (fronds, true, FADE),
            ]
            .into_iter()
            .filter_map(|(b, is_fronds, fade)| {
                let (mesh, aabb, at) = crate::render::centre(b.build()?);
                Some((mesh, aabb, at, is_fronds, fade))
            })
            .collect()
        });
        assets.building.insert(key, task);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tufts_and_ferns_make_meshes() {
        for kind in [CoverKind::Grass, CoverKind::Sedge, CoverKind::Fern] {
            let s = CoverSpot {
                pos: [0.0, 0.0, 0.0],
                kind,
                scale: 1.0,
                yaw: 0.3,
                seed: 7,
                rank: 0.5,
            };
            let mut b = Builder::default();
            if kind == CoverKind::Fern {
                fern(&mut b, &s);
            } else {
                tuft(&mut b, &s, false);
            }
            assert!(b.idx.len() >= 5 * 12, "{kind:?}: {} indices", b.idx.len());
            let top = b.pos.iter().map(|p| p[1]).fold(0.0, f32::max);
            assert!((0.15..1.0).contains(&top), "{kind:?} stands {top} m");
            assert!(
                b.uv.iter()
                    .all(|u| (0.0..=1.0).contains(&u[0]) && (0.0..=1.0).contains(&u[1]))
            );
        }
    }
}
