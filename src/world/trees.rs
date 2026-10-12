//! Procedural trees and their scattering through the map's forests.
//!
//! Trees are generated meshes (a few variants per species), instanced many
//! times with random yaw, scale and lean. Each gets a static trunk collider.

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use extraction_core::geom;
use extraction_core::map::TreeSpecies;
use extraction_core::noise::Rng;

use super::{Layer, Zone};

const VARIANTS: usize = 3;

pub struct TreesPlugin;

impl Plugin for TreesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, spawn_forests);
    }
}

/// Logic-side tree record: where trunks are, for colliders, AI line of
/// sight and the sims. Built once from the map.
#[derive(Clone, Debug)]
pub struct TreeInstance {
    pub species: TreeSpecies,
    pub variant: usize,
    pub pos: Vec3,
    pub yaw: f32,
    pub scale: f32,
}

/// Scatters trees through every forest polygon. Deterministic per map.
pub fn scatter_trees(zone: &Zone) -> Vec<TreeInstance> {
    let mut out = Vec::new();
    let map = &zone.map;
    // Keep clear of structures and markers.
    let clear: Vec<([f32; 2], f32)> = map
        .placements
        .iter()
        .map(|p| (p.at, super::structures::clearance(&p.prefab)))
        .chain(map.markers.iter().map(|m| (m.at, 8.0)))
        .collect();
    for forest in &map.forests {
        let mut rng = Rng::new(0x7EE5 ^ u64::from(forest.seed));
        let spacing = (100.0 / forest.density.max(0.01)).sqrt();
        let (lo, hi) = geom::bounds(&forest.polygon);
        let weights: Vec<f32> = forest.mix.iter().map(|(_, w)| *w).collect();
        let mut z = lo[1];
        while z < hi[1] {
            let mut x = lo[0];
            while x < hi[0] {
                let p = [
                    x + rng.range(-0.45, 0.45) * spacing,
                    z + rng.range(-0.45, 0.45) * spacing,
                ];
                x += spacing;
                if !geom::point_in_polygon(&forest.polygon, p) {
                    continue;
                }
                // Thin out toward the forest edge for a ragged treeline.
                let edge = geom::polygon_edge_distance(&forest.polygon, p);
                if edge < 12.0 && rng.f32() > edge / 12.0 {
                    continue;
                }
                if zone.hm.road_at(p).weight > 0 || zone.hm.water_level(p).is_some() {
                    continue;
                }
                if zone.hm.wetness(p) > 0.7 || zone.hm.slope_deg(p) > 38.0 {
                    continue;
                }
                if zone
                    .hm
                    .road_lines
                    .iter()
                    .any(|rl| geom::polyline_closest(&rl.points, p).0 < rl.width * 0.5 + 2.5)
                {
                    continue;
                }
                if clear.iter().any(|(c, r)| geom::dist(*c, p) < *r) {
                    continue;
                }
                let Some(k) = rng.weighted(&weights) else {
                    continue;
                };
                let species = forest.mix[k].0;
                let y = zone.ground(p[0], p[1]);
                out.push(TreeInstance {
                    species,
                    variant: rng.below(VARIANTS),
                    pos: Vec3::new(p[0], y - 0.15, p[1]),
                    yaw: rng.range(0.0, std::f32::consts::TAU),
                    scale: rng.range(0.8, 1.2),
                });
            }
            z += spacing;
        }
    }
    out
}

/// A tree in the world: logic side (collider). Meshes attach client-side.
#[derive(Component, Clone, Copy, Debug)]
pub struct Tree {
    pub species: TreeSpecies,
    pub variant: usize,
}

/// Logic: every tree gets a trunk collider (bullets stop, robots can't see
/// through, nobody walks through).
pub fn spawn_tree_colliders(mut commands: Commands, zone: Res<Zone>) {
    let started = std::time::Instant::now();
    let trees = scatter_trees(&zone);
    for t in &trees {
        let radius = trunk_radius(t.species);
        commands.spawn((
            Tree {
                species: t.species,
                variant: t.variant,
            },
            Transform::from_translation(t.pos)
                .with_rotation(Quat::from_rotation_y(t.yaw))
                .with_scale(Vec3::splat(t.scale)),
            Visibility::default(),
            RigidBody::Static,
            Collider::capsule_endpoints(radius, Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 4.0, 0.0)),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
    info!(
        "{} trees scattered in {:.0} ms",
        trees.len(),
        started.elapsed().as_secs_f32() * 1000.0
    );
}

/// Client: tree meshes onto the logic trees.
fn spawn_forests(
    mut commands: Commands,
    trees: Query<(Entity, &Tree), Added<Tree>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<TreeAssets>>,
) {
    if trees.is_empty() {
        return;
    }
    let assets = cache.get_or_insert_with(|| {
        let bark = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            reflectance: 0.2,
            ..default()
        });
        let foliage = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.25,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let mut handles = std::collections::HashMap::new();
        for s in [
            TreeSpecies::Spruce,
            TreeSpecies::Pine,
            TreeSpecies::Birch,
            TreeSpecies::Snag,
        ] {
            for v in 0..VARIANTS {
                let (trunk, crown) = build_tree(s, 1000 + v as u64 * 17 + s as u64 * 101);
                handles.insert((s, v), (meshes.add(trunk), crown.map(|c| meshes.add(c))));
            }
        }
        TreeAssets {
            bark,
            foliage,
            handles,
        }
    });
    for (e, tree) in &trees {
        let (trunk, crown) = &assets.handles[&(tree.species, tree.variant)];
        commands
            .entity(e)
            .insert((Mesh3d(trunk.clone()), MeshMaterial3d(assets.bark.clone())));
        if let Some(crown) = crown {
            commands.entity(e).with_child((
                Mesh3d(crown.clone()),
                MeshMaterial3d(assets.foliage.clone()),
            ));
        }
    }
}

struct TreeAssets {
    bark: Handle<StandardMaterial>,
    foliage: Handle<StandardMaterial>,
    handles: std::collections::HashMap<(TreeSpecies, usize), (Handle<Mesh>, Option<Handle<Mesh>>)>,
}

fn trunk_radius(s: TreeSpecies) -> f32 {
    match s {
        TreeSpecies::Spruce => 0.22,
        TreeSpecies::Pine => 0.2,
        TreeSpecies::Birch => 0.14,
        TreeSpecies::Snag => 0.18,
    }
}

/// Accumulates triangles with per-vertex colour.
#[derive(Default)]
struct MeshBuilder {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl MeshBuilder {
    fn vertex(&mut self, p: Vec3, n: Vec3, c: [f32; 4]) -> u32 {
        self.pos.push(p.to_array());
        self.nrm.push(n.normalize_or_zero().to_array());
        self.col.push(c);
        (self.pos.len() - 1) as u32
    }

    /// A tapered tube along a polyline of (centre, radius) rings.
    fn tube(&mut self, rings: &[(Vec3, f32)], sides: usize, color: impl Fn(Vec3, f32) -> [f32; 4]) {
        let base = self.pos.len() as u32;
        for (k, (c, r)) in rings.iter().enumerate() {
            let dir = if k + 1 < rings.len() {
                (rings[k + 1].0 - *c).normalize_or_zero()
            } else {
                (*c - rings[k.saturating_sub(1)].0).normalize_or_zero()
            };
            let side = dir.any_orthonormal_vector();
            let up = dir.cross(side);
            for s in 0..=sides {
                let a = s as f32 / sides as f32 * std::f32::consts::TAU;
                let n = side * a.cos() + up * a.sin();
                let p = *c + n * *r;
                self.vertex(p, n, color(p, a));
            }
        }
        let ring = (sides + 1) as u32;
        for k in 0..rings.len().saturating_sub(1) as u32 {
            for s in 0..sides as u32 {
                let a = base + k * ring + s;
                let b = a + 1;
                let c = a + ring;
                let d = c + 1;
                self.idx.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }
    }

    /// A lumpy blob: an icosphere-ish UV sphere displaced by noise.
    fn blob(
        &mut self,
        centre: Vec3,
        radius: Vec3,
        seed: u32,
        color: impl Fn(Vec3, f32) -> [f32; 4],
    ) {
        let (rings, segs) = (7, 10);
        let base = self.pos.len() as u32;
        for i in 0..=rings {
            let v = i as f32 / rings as f32;
            let phi = v * std::f32::consts::PI;
            for j in 0..=segs {
                let u = j as f32 / segs as f32;
                let th = u * std::f32::consts::TAU;
                let dir = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
                let lump = 0.75
                    + 0.5
                        * extraction_core::noise::fbm(
                            dir.x * 1.8 + dir.y * 0.7,
                            dir.z * 1.8 - dir.y * 0.5,
                            seed,
                            3,
                            0.5,
                        )
                        .abs()
                        .min(0.6);
                let p = centre + dir * radius * lump;
                self.vertex(p, dir, color(p, lump));
            }
        }
        let row = (segs + 1) as u32;
        for i in 0..rings as u32 {
            for j in 0..segs as u32 {
                let a = base + i * row + j;
                let b = a + 1;
                let c = a + row;
                let d = c + 1;
                self.idx.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }

    fn build(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
        .with_inserted_indices(Indices::U32(self.idx))
    }
}

fn lin(r: f32, g: f32, b: f32) -> [f32; 4] {
    let c = Color::srgb(r, g, b).to_linear();
    [c.red, c.green, c.blue, 1.0]
}

/// Builds (trunk, crown) meshes for a species variant in local space, base
/// at the origin, before instance scaling.
fn build_tree(species: TreeSpecies, seed: u64) -> (Mesh, Option<Mesh>) {
    let mut rng = Rng::new(seed);
    let mut trunk = MeshBuilder::default();
    let mut crown = MeshBuilder::default();
    match species {
        TreeSpecies::Spruce => {
            let h = rng.range(15.0, 22.0);
            let r0 = 0.24;
            let rings: Vec<(Vec3, f32)> = (0..=6)
                .map(|k| {
                    let t = k as f32 / 6.0;
                    (Vec3::new(0.0, t * h, 0.0), r0 * (1.0 - t * 0.92))
                })
                .collect();
            trunk.tube(&rings, 7, |_, _| lin(0.22, 0.17, 0.13));
            // Whorls of drooping, jagged skirts, widest low down.
            let first = rng.range(1.2, 2.4);
            let whorls = 15;
            let rmax = rng.range(2.6, 3.4);
            for w in 0..whorls {
                let t = w as f32 / (whorls - 1) as f32;
                let y = first + t * (h - first - 0.4);
                let r = rmax * (1.0 - t).powf(0.85) + 0.25;
                let droop = r * 0.45;
                let sides = 9;
                let base = crown.pos.len() as u32;
                let top = crown.vertex(
                    Vec3::new(0.0, y + r * 0.55 + 0.3, 0.0),
                    Vec3::Y,
                    lin(0.10, 0.14, 0.10),
                );
                let jag = rng.range(0.0, 1.0);
                for s in 0..=sides * 2 {
                    let a = s as f32 / (sides * 2) as f32 * std::f32::consts::TAU + jag;
                    // Alternate long and short points for branch tips.
                    let rr = if s % 2 == 0 { r } else { r * 0.62 } * rng.range(0.85, 1.1);
                    let dir = Vec3::new(a.cos(), 0.0, a.sin());
                    let p = dir * rr + Vec3::Y * (y - droop * (rr / r));
                    let tip = rr / r;
                    let shade = 0.75 + 0.35 * tip + 0.15 * t;
                    let c = lin(0.11 * shade, 0.155 * shade, 0.115 * shade);
                    crown.vertex(p, (dir + Vec3::Y * 0.8).normalize(), c);
                }
                for s in 0..(sides * 2) as u32 {
                    crown
                        .idx
                        .extend_from_slice(&[top, base + 2 + s, base + 1 + s]);
                }
            }
        }
        TreeSpecies::Pine => {
            let h = rng.range(17.0, 24.0);
            let lean = Vec3::new(rng.range(-0.4, 0.4), 0.0, rng.range(-0.4, 0.4));
            let rings: Vec<(Vec3, f32)> = (0..=8)
                .map(|k| {
                    let t = k as f32 / 8.0;
                    (
                        Vec3::new(0.0, t * h, 0.0) + lean * t * t,
                        0.24 * (1.0 - t * 0.8),
                    )
                })
                .collect();
            trunk.tube(&rings, 7, |p, _| {
                // Grey-brown below, the orange "fox" bark of a Scots pine above.
                let t = (p.y / h).clamp(0.0, 1.0);
                let a = Vec3::new(0.24, 0.20, 0.17);
                let b = Vec3::new(0.55, 0.32, 0.18);
                let c = a.lerp(b, ((t - 0.35) * 2.5).clamp(0.0, 1.0));
                lin(c.x, c.y, c.z)
            });
            let crown_base = h * rng.range(0.62, 0.72);
            let blobs = 5 + rng.below(3);
            for b in 0..blobs {
                let t = b as f32 / blobs as f32;
                let a = rng.range(0.0, std::f32::consts::TAU);
                let y = crown_base + t * (h - crown_base);
                let off = Vec3::new(a.cos(), 0.0, a.sin()) * rng.range(0.6, 1.8) * (1.0 - t * 0.6);
                let c = Vec3::new(0.0, y, 0.0) + lean * (y / h).powi(2) + off;
                let rad = Vec3::new(
                    rng.range(1.5, 2.2),
                    rng.range(1.3, 1.9),
                    rng.range(1.5, 2.2),
                ) * (1.0 - t * 0.35);
                crown.blob(c, rad, rng.next_u64() as u32, |_, l| {
                    let s = 0.7 + 0.5 * l;
                    lin(0.12 * s, 0.16 * s, 0.09 * s)
                });
            }
        }
        TreeSpecies::Birch => {
            let h = rng.range(11.0, 16.0);
            let lean = Vec3::new(rng.range(-0.9, 0.9), 0.0, rng.range(-0.9, 0.9));
            let rings: Vec<(Vec3, f32)> = (0..=8)
                .map(|k| {
                    let t = k as f32 / 8.0;
                    (
                        Vec3::new(0.0, t * h, 0.0) + lean * t * t,
                        0.15 * (1.0 - t * 0.85),
                    )
                })
                .collect();
            let s = seed as u32;
            trunk.tube(&rings, 7, |p, a| {
                // White bark with dark horizontal lenticels, darker at the foot.
                let dash = extraction_core::noise::perlin(a * 2.0, p.y * 6.0, s).abs();
                let foot = (1.0 - p.y / 2.5).clamp(0.0, 1.0);
                let v = if dash > 0.55 {
                    0.18
                } else {
                    0.78 - 0.45 * foot
                };
                lin(v, v * 0.98, v * 0.94)
            });
            let crown_base = h * 0.45;
            let blobs = 6 + rng.below(4);
            // Autumn: most birches have turned, some are still hanging on.
            let turned = rng.range(0.3, 1.0);
            for b in 0..blobs {
                let t = b as f32 / blobs as f32;
                let a = rng.range(0.0, std::f32::consts::TAU);
                let y = crown_base + t * (h - crown_base) * 0.95;
                let off = Vec3::new(a.cos(), 0.0, a.sin()) * rng.range(0.5, 1.6);
                let c = Vec3::new(0.0, y, 0.0) + lean * (y / h).powi(2) + off;
                let rad = Vec3::splat(rng.range(1.0, 1.7)) * Vec3::new(1.0, 0.8, 1.0);
                let yellow = (turned + rng.range(-0.2, 0.2)).clamp(0.0, 1.0);
                crown.blob(c, rad, rng.next_u64() as u32, move |_, l| {
                    let s = 0.65 + 0.5 * l;
                    let g = Vec3::new(0.22, 0.28, 0.10);
                    let y = Vec3::new(0.62, 0.48, 0.12);
                    let c = g.lerp(y, yellow) * s;
                    lin(c.x, c.y, c.z)
                });
            }
        }
        TreeSpecies::Snag => {
            let h = rng.range(6.0, 12.0);
            let lean = Vec3::new(rng.range(-0.6, 0.6), 0.0, rng.range(-0.6, 0.6));
            let rings: Vec<(Vec3, f32)> = (0..=6)
                .map(|k| {
                    let t = k as f32 / 6.0;
                    (Vec3::new(0.0, t * h, 0.0) + lean * t, 0.2 * (1.0 - t * 0.6))
                })
                .collect();
            trunk.tube(&rings, 6, |p, _| {
                let v = 0.36 + 0.06 * (p.y * 3.1).sin();
                lin(v, v * 0.97, v * 0.93)
            });
            // Broken branch stubs.
            for _ in 0..(3 + rng.below(4)) {
                let y = rng.range(2.0, h * 0.9);
                let a = rng.range(0.0, std::f32::consts::TAU);
                let dir = Vec3::new(a.cos(), rng.range(-0.1, 0.5), a.sin()).normalize();
                let start = Vec3::new(0.0, y, 0.0) + lean * (y / h);
                let len = rng.range(0.6, 2.2);
                trunk.tube(&[(start, 0.06), (start + dir * len, 0.02)], 4, |_, _| {
                    lin(0.33, 0.32, 0.30)
                });
            }
        }
    }
    let crown = (!crown.pos.is_empty()).then(|| crown.build());
    (trunk.build(), crown)
}
