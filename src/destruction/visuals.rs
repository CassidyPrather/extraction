//! Client: the glass and the pieces.
//!
//! A building's panes are one mesh (a draw for all its windows): each
//! whole pane a thin slab, each broken one the teeth its break left in
//! the frame (`fracture::shatter`, the same seed as when it broke, so a
//! restored save shows the same teeth). It's rebuilt when a pane breaks
//! or the Surge mends them. Pieces in flight are their own small slabs,
//! shrinking away at the end of their life.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::math::Affine3A;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use extraction_core::fracture::{self, P2};
use std::collections::HashMap;

use super::debris::{Debris, Kind};
use super::glass::{Pane, THICK, cells_for};
use crate::render::materials::{Mat, MaterialLibrary};

pub struct DestructionVisualsPlugin;

impl Plugin for DestructionVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShardMaterial>().add_systems(
            Update,
            (
                glaze_buildings,
                dress_debris,
                fade_debris,
                shoot_for_screenshot,
                dress_crates,
            ),
        );
    }
}

/// A convex outline extruded `thick` along z, centred on z 0: faces,
/// flat normals, UVs in metres.
pub fn slab(poly: &[P2], thick: f32) -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    append_slab(
        &mut pos,
        &mut nrm,
        &mut uv,
        &mut idx,
        poly,
        thick,
        Affine3A::IDENTITY,
    );
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    m.insert_indices(Indices::U32(idx));
    m
}

fn append_slab(
    pos: &mut Vec<[f32; 3]>,
    nrm: &mut Vec<[f32; 3]>,
    uv: &mut Vec<[f32; 2]>,
    idx: &mut Vec<u32>,
    poly: &[P2],
    thick: f32,
    xf: Affine3A,
) {
    let n = poly.len();
    if n < 3 {
        return;
    }
    let h = thick * 0.5;
    let mut push = |p: Vec3, normal: Vec3, t: [f32; 2]| {
        pos.push(xf.transform_point3(p).to_array());
        nrm.push(xf.transform_vector3(normal).normalize_or_zero().to_array());
        uv.push(t);
        (pos.len() - 1) as u32
    };
    // Front (+z) and back (-z), fans (the outline is convex, CCW).
    for (z, normal) in [(h, Vec3::Z), (-h, Vec3::NEG_Z)] {
        let first = pos_len_after(&mut push, poly, z, normal);
        for i in 1..n as u32 - 1 {
            if z > 0.0 {
                idx.extend([first, first + i, first + i + 1]);
            } else {
                idx.extend([first, first + i + 1, first + i]);
            }
        }
    }
    // The edges.
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let e = Vec3::new(b[0] - a[0], b[1] - a[1], 0.0);
        let normal = Vec3::new(e.y, -e.x, 0.0).normalize_or_zero();
        let l = e.length();
        let v0 = push(Vec3::new(a[0], a[1], -h), normal, [0.0, 0.0]);
        let v1 = push(Vec3::new(b[0], b[1], -h), normal, [l, 0.0]);
        let v2 = push(Vec3::new(b[0], b[1], h), normal, [l, thick]);
        let v3 = push(Vec3::new(a[0], a[1], h), normal, [0.0, thick]);
        idx.extend([v0, v1, v2, v0, v2, v3]);
    }
}

fn pos_len_after(
    push: &mut impl FnMut(Vec3, Vec3, [f32; 2]) -> u32,
    poly: &[P2],
    z: f32,
    normal: Vec3,
) -> u32 {
    let mut first = None;
    for p in poly {
        let i = push(Vec3::new(p[0], p[1], z), normal, *p);
        first.get_or_insert(i);
    }
    first.unwrap_or(0)
}

/// The glass of a building's windows, drawn as one.
#[derive(Component)]
pub struct WindowGlass;

/// Rebuild the glass of each building whose panes changed.
fn glaze_buildings(
    mut commands: Commands,
    changed: Query<&Pane, Changed<Pane>>,
    panes: Query<(&Pane, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    mut shards: ResMut<ShardMaterial>,
    mut drawn: Local<HashMap<(Entity, usize), Entity>>,
) {
    let mut dirty: Vec<Entity> = changed.iter().map(|p| p.building).collect();
    if dirty.is_empty() {
        return;
    }
    dirty.sort();
    dirty.dedup();
    // Per building, two meshes: the whole panes, and the broken ones'
    // teeth (paler and glinting, so a broken window reads as broken).
    type Buf = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>);
    let mut by: HashMap<Entity, [Buf; 2]> =
        dirty.iter().map(|b| (*b, Default::default())).collect();
    for (p, local) in &panes {
        let Some(bufs) = by.get_mut(&p.building) else {
            continue;
        };
        let xf = local.compute_affine();
        match p.broken {
            None => {
                let (pos, nrm, uv, idx) = &mut bufs[0];
                let h = p.half;
                let rect = [[-h.x, -h.y], [h.x, -h.y], [h.x, h.y], [-h.x, h.y]];
                append_slab(pos, nrm, uv, idx, &rect, THICK, xf);
            }
            // The years' breakage is in the sash's own model.
            Some(b) if b.weathered => {}
            Some(b) => {
                let (pos, nrm, uv, idx) = &mut bufs[1];
                let s = fracture::shatter(
                    p.half.to_array(),
                    b.impact.to_array(),
                    b.seed,
                    cells_for(p.area()),
                );
                for k in &s.kept {
                    let at = xf * Affine3A::from_translation(Vec3::new(k.at[0], k.at[1], 0.0));
                    append_slab(pos, nrm, uv, idx, &k.poly, THICK, at);
                }
            }
        }
    }
    let glass = [
        library.get(Mat::Glass, &server, &mut materials),
        shards.get(&mut materials),
    ];
    for (b, bufs) in by {
        if commands.get_entity(b).is_err() {
            continue;
        }
        for (layer, (pos, nrm, uv, idx)) in bufs.into_iter().enumerate() {
            let old = drawn.get(&(b, layer)).copied();
            if idx.is_empty() {
                if let Some(e) = old {
                    commands.entity(e).try_despawn();
                    drawn.remove(&(b, layer));
                }
                continue;
            }
            let mut m = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            );
            m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
            m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
            m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
            m.insert_indices(Indices::U32(idx));
            let handle = meshes.add(m);
            match old {
                Some(e) if commands.get_entity(e).is_ok() => {
                    commands.entity(e).insert(Mesh3d(handle));
                }
                _ => {
                    let e = commands
                        .spawn((
                            Name::new("Window glass"),
                            WindowGlass,
                            Mesh3d(handle),
                            MeshMaterial3d(glass[layer].clone()),
                            Transform::IDENTITY,
                            NotShadowCaster,
                            crate::render::lod::near_range(),
                            ChildOf(b),
                        ))
                        .id();
                    drawn.insert((b, layer), e);
                }
            }
        }
    }
}

/// Broken glass: paler than a whole pane and catching the light on its
/// edges, so teeth in a frame and shards on a floor read as broken.
#[derive(Resource, Default)]
pub struct ShardMaterial(Option<Handle<StandardMaterial>>);

impl ShardMaterial {
    fn get(&mut self, materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
        self.0
            .get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgba(0.45, 0.52, 0.52, 0.42),
                    perceptual_roughness: 0.04,
                    reflectance: 1.0,
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .clone()
    }
}

/// A piece's slab, as it's thrown.
fn dress_debris(
    mut commands: Commands,
    added: Query<(Entity, &Debris), Added<Debris>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    mut shards: ResMut<ShardMaterial>,
    server: Res<AssetServer>,
) {
    for (e, d) in &added {
        let material = match d.kind {
            Kind::Glass => shards.get(&mut materials),
            Kind::Wood(m) => library.get(m, &server, &mut materials),
        };
        // On a child, so shrinking it never touches the body.
        commands.spawn((
            DebrisMesh,
            Mesh3d(meshes.add(slab(&d.poly, d.thick))),
            MeshMaterial3d(material),
            NotShadowCaster,
            Transform::IDENTITY,
            ChildOf(e),
        ));
    }
}

#[derive(Component)]
pub struct DebrisMesh;

/// The last half second of a piece's life, it shrinks away.
fn fade_debris(
    pieces: Query<&Debris>,
    mut drawn: Query<(&ChildOf, &mut Transform), With<DebrisMesh>>,
) {
    for (parent, mut t) in &mut drawn {
        let Ok(d) = pieces.get(parent.parent()) else {
            continue;
        };
        let left = d.life - d.age;
        if left < 0.5 {
            t.scale = Vec3::splat((left / 0.5).clamp(0.05, 1.0));
        }
    }
}

/// For looking at breaks (`docs/shots/destruction`): with
/// `EXTRACTION_SHOOT=N` on a `--screenshot` run, a rifle round is fired
/// from the `--camera` along its view each of the last N frames before
/// the capture, so the capture catches the pieces in the air.
fn shoot_for_screenshot(
    mut commands: Commands,
    opts: Res<crate::LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    player: Query<Entity, With<crate::rig::LocalPlayer>>,
    mut frame: Local<u32>,
) {
    let (Some(_), Some((cam, yaw, pitch)), Some(zone)) = (&opts.screenshot, opts.camera, zone)
    else {
        return;
    };
    let Some(n) = std::env::var("EXTRACTION_SHOOT")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
    else {
        return;
    };
    *frame += 1;
    let last = opts.frames.saturating_sub(1);
    if *frame + n <= last || *frame > last {
        return;
    }
    let ground = zone.ground(cam.x, cam.z);
    let eye_y = if cam.y == 0.0 { ground + 1.7 } else { cam.y };
    let rot = Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
    let gun = commands.spawn_empty().id();
    commands.spawn((
        crate::weapons::ballistics::Projectile {
            velocity: rot * Vec3::NEG_Z * 800.0,
            damage: 90.0,
            // (So it starts outside the body that's standing there.)
            shooter: player.iter().next(),
            gun,
            life: 1.0,
            mods: default(),
            mass_g: 9.6,
            seed: *frame as u64,
            passes: 0,
            load: default(),
        },
        Transform::from_translation(Vec3::new(cam.x, eye_y, cam.z)),
    ));
}

/// A building's crate, drawn on its own: its box until its model is in.
fn dress_crates(
    mut commands: Commands,
    added: Query<(Entity, &super::wood::BuildingCrate), Added<super::wood::BuildingCrate>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    use crate::render::world_props::{Blockout, SwapIn};
    for (e, c) in &added {
        let planks = library.get(Mat::Planks, &server, &mut materials);
        commands.spawn((
            Mesh3d(meshes.add(crate::render::materials::metric_box(Vec3::splat(c.size)))),
            MeshMaterial3d(planks),
            Transform::IDENTITY,
            Blockout,
            ChildOf(e),
        ));
        commands.entity(e).insert((
            SwapIn(vec![(c.model, c.model_at)]),
            crate::render::lod::near_range(),
        ));
    }
}
