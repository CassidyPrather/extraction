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
                redress_breached,
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

/// Rebuild the glass of each building whose panes changed. A window's
/// leaf has its own (hung from it, so it swings with it); the litter on
/// the floor is the building's.
fn glaze_buildings(
    mut commands: Commands,
    changed: Query<&Pane, Changed<Pane>>,
    panes: Query<(&Pane, &Transform)>,
    leaves: Query<&crate::world::hinge::Moving>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    mut shards: ResMut<ShardMaterial>,
    spatial: avian3d::prelude::SpatialQuery,
    undressed: Query<(), With<crate::world::buildings::PropsPending>>,
    mut waiting: Local<Vec<(Entity, Entity)>>,
    mut drawn: Local<HashMap<(Entity, usize), Entity>>,
) {
    // A building gone (the zone left), or its glass: forget them.
    drawn.retain(|(b, _), e| commands.get_entity(*b).is_ok() && commands.get_entity(*e).is_ok());
    // (what it hangs from, its building)
    let mut dirty: Vec<(Entity, Entity)> = changed
        .iter()
        .flat_map(|p| {
            [(p.building, p.building)]
                .into_iter()
                .chain(p.leaf.map(|l| (l, p.building)))
        })
        .collect();
    dirty.append(&mut waiting);
    if dirty.is_empty() {
        return;
    }
    dirty.sort();
    dirty.dedup();
    // Not before the building's own walls are drawn (its shading bakes in
    // the background): glass hanging in the air on a valley's arrival.
    let (now, later): (Vec<(Entity, Entity)>, Vec<(Entity, Entity)>) = dirty
        .into_iter()
        .partition(|(_, b)| !undressed.contains(*b));
    *waiting = later;
    let dirty = now;
    if dirty.is_empty() {
        return;
    }
    // Per building, two meshes: the whole panes, and the broken ones'
    // teeth (paler and glinting, so a broken window reads as broken).
    type Buf = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>);
    let mut by: HashMap<Entity, [Buf; 2]> = dirty
        .iter()
        .map(|(g, _)| (*g, Default::default()))
        .collect();
    for (p, local) in &panes {
        // In its leaf's space, or its building's.
        let (group, xf) = match p.leaf.and_then(|l| Some((l, leaves.get(l).ok()?))) {
            Some((l, m)) => (
                l,
                m.rest.compute_affine().inverse() * p.rest.compute_affine(),
            ),
            None => (p.building, local.compute_affine()),
        };
        // The building's own space, for what lies on its floor.
        let to_building = (p.rest.compute_affine() * local.compute_affine().inverse()).inverse();
        if let (Some(b), true) = (p.broken.filter(|b| !b.weathered), group != p.building)
            && let Some(bufs) = by.get_mut(&p.building)
        {
            let (pos, nrm, uv, idx) = &mut bufs[1];
            for (at, poly) in litter(&spatial, p, b) {
                let flat = Affine3A::from_rotation_translation(
                    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                    at,
                );
                append_slab(pos, nrm, uv, idx, &poly, 0.002, to_building * flat);
            }
        }
        let Some(bufs) = by.get_mut(&group) else {
            continue;
        };
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
                // And glitter on the floor where it fell, the room's side
                // or the street's: what tells a window was broken after
                // the pieces are swept away. (A leaf's: above.)
                if group != p.building {
                    continue;
                }
                for (at, poly) in litter(&spatial, p, b) {
                    let flat = Affine3A::from_rotation_translation(
                        Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                        at,
                    );
                    append_slab(pos, nrm, uv, idx, &poly, 0.002, to_building * flat);
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

/// Bits of glass on the floor under a broken pane, on the side it fell:
/// where each lies (world, on the floor) and its outline. Deterministic
/// from the break.
fn litter(
    spatial: &avian3d::prelude::SpatialQuery,
    p: &Pane,
    b: super::glass::Break,
) -> Vec<(Vec3, Vec<P2>)> {
    use avian3d::prelude::SpatialQueryFilter;
    let mut rng = extraction_core::noise::Rng::new(b.seed ^ 0x1177);
    let n = (5.0 + p.area() * 25.0).min(14.0) as usize;
    // Under the window shut, wherever its leaf is now.
    let normal = p.rest.rotation * Vec3::Z * b.fell;
    let filter = SpatialQueryFilter::from_mask(crate::world::Layer::World);
    (0..n)
        .filter_map(|_| {
            let across = rng.range(-p.half.x, p.half.x) * 0.9;
            let out = rng.range(0.06, 0.7);
            let start = p.rest.transform_point(Vec3::new(across, -p.half.y, 0.0))
                + normal * out
                + Vec3::Y * 0.05;
            let hit = spatial.cast_ray(start, Dir3::NEG_Y, 3.0, true, &filter)?;
            let r = rng.range(0.012, 0.035);
            let a0 = rng.range(0.0, std::f32::consts::TAU);
            let poly: Vec<P2> = (0..3)
                .map(|i| {
                    let a = a0 + i as f32 * 2.1 + rng.range(-0.3, 0.3);
                    let rr = r * rng.range(0.6, 1.4);
                    [a.cos() * rr, a.sin() * rr]
                })
                .collect();
            // Wound counter-clockwise for the slab's faces.
            let poly = if fracture::area(&poly) < 0.0 {
                poly.into_iter().rev().collect()
            } else {
                poly
            };
            // A few millimetres proud of the floor (the house rule).
            Some((start - Vec3::Y * (hit.distance - 0.005), poly))
        })
        .collect()
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
    mut shapes: Local<Vec<Handle<Mesh>>>,
    server: Res<AssetServer>,
) {
    // A handful of shard shapes, each a unit slab, stretched to every
    // piece's size: pieces of a kind share mesh and material, so a
    // window's worth (or a cap's worth) batches into a few draws.
    if shapes.is_empty() {
        let cut = fracture::shatter([1.0, 1.0], [0.3, -0.2], 0x5_4A9E, 16);
        *shapes = cut
            .falling
            .iter()
            .chain(&cut.kept)
            .take(SHAPES)
            .map(|sh| {
                let h = sh.half_extents();
                let unit: Vec<P2> = sh
                    .poly
                    .iter()
                    .map(|p| [p[0] / h[0].max(1e-3), p[1] / h[1].max(1e-3)])
                    .collect();
                meshes.add(slab(&unit, 1.0))
            })
            .collect();
    }
    for (e, d) in &added {
        let material = match d.kind {
            Kind::Glass => shards.get(&mut materials),
            Kind::Wood(m) => library.get(m, &server, &mut materials),
        };
        let mut half = [0.0f32; 2];
        for p in &d.poly {
            half[0] = half[0].max(p[0].abs());
            half[1] = half[1].max(p[1].abs());
        }
        let k = (half[0] * 1000.0 + half[1] * 7000.0) as usize % shapes.len().max(1);
        let size = Vec3::new(half[0], half[1], d.thick);
        // On a child, so shrinking it never touches the body.
        commands.spawn((
            DebrisMesh(size),
            Mesh3d(shapes[k].clone()),
            MeshMaterial3d(material),
            NotShadowCaster,
            Transform::from_scale(size),
            ChildOf(e),
        ));
    }
}

/// How many shard shapes the pieces share.
const SHAPES: usize = 8;

/// A piece's drawn slab, and its size (the unit shape's stretch).
#[derive(Component)]
pub struct DebrisMesh(pub Vec3);

/// The last half second of a piece's life, it shrinks away.
fn fade_debris(pieces: Query<&Debris>, mut drawn: Query<(&ChildOf, &DebrisMesh, &mut Transform)>) {
    for (parent, size, mut t) in &mut drawn {
        let Ok(d) = pieces.get(parent.parent()) else {
            continue;
        };
        let left = d.life - d.age;
        if left < 0.5 {
            t.scale = size.0 * (left / 0.5).clamp(0.05, 1.0);
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
    spatial: avian3d::prelude::SpatialQuery,
    mut breaches: MessageWriter<super::hooks::Breach>,
    parts: Query<(), With<super::wood::Timber>>,
    names: Query<NameOrEntity>,
    mut frame: Local<u32>,
) {
    let (Some(_), Some((cam, yaw, pitch)), Some(zone)) = (&opts.screenshot, opts.camera, zone)
    else {
        return;
    };
    *frame += 1;
    // `EXTRACTION_BREACH=1`: twenty frames before the capture, a breach
    // where the view meets a wall (time for the walls to be built again).
    if std::env::var("EXTRACTION_BREACH").is_ok() && *frame + 20 == opts.frames {
        let ground = zone.ground(cam.x, cam.z);
        let eye = Vec3::new(
            cam.x,
            if cam.y == 0.0 { ground + 1.7 } else { cam.y },
            cam.z,
        );
        let rot = Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
        let dir = rot * Vec3::NEG_Z;
        if let Some(hit) = spatial.cast_ray(
            eye,
            Dir3::new(dir).unwrap_or(Dir3::NEG_Z),
            30.0,
            true,
            &avian3d::prelude::SpatialQueryFilter::from_mask(crate::world::Layer::World),
        ) {
            breaches.write(super::hooks::Breach {
                at: eye + dir * (hit.distance + 0.05),
                out: -dir,
            });
        }
    }
    // `EXTRACTION_STRIP=1`: twenty frames (or `EXTRACTION_STRIP_AT`)
    // before the capture, the breakable the view meets (a shutter, a bus
    // panel, a crate) goes, as to a crowbar or a boot.
    let strip_at = std::env::var("EXTRACTION_STRIP_AT")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(20);
    if std::env::var("EXTRACTION_STRIP").is_ok() && *frame + strip_at == opts.frames {
        let ground = zone.ground(cam.x, cam.z);
        let eye = Vec3::new(
            cam.x,
            if cam.y == 0.0 { ground + 1.7 } else { cam.y },
            cam.z,
        );
        let rot = Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
        let mut hits = spatial.ray_hits(
            eye,
            Dir3::new(rot * Vec3::NEG_Z).unwrap_or(Dir3::NEG_Z),
            30.0,
            8,
            true,
            &avian3d::prelude::SpatialQueryFilter::from_mask(crate::world::Layer::World),
        );
        hits.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        for h in &hits {
            let name = names
                .get(h.entity)
                .map(|n| n.to_string())
                .unwrap_or_default();
            info!("EXTRACTION_STRIP: {name} at {:.2} m", h.distance);
        }
        match hits.iter().find(|h| parts.contains(h.entity)) {
            Some(h) => commands.entity(h.entity).despawn(),
            None => warn!("EXTRACTION_STRIP: the view meets nothing breakable"),
        }
    }
    let Some(n) = std::env::var("EXTRACTION_SHOOT")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
    else {
        return;
    };
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

/// A breakable thing drawn on its own (`wood::Drawn`: a building's crate,
/// a board over a window): its box until its model is in. Each mesh
/// carries its range (a range doesn't pass to children).
#[allow(clippy::type_complexity)]
fn dress_crates(
    mut commands: Commands,
    things: Query<
        (Entity, &super::wood::Drawn, &Transform, Option<&Children>),
        Without<CrateDressed>,
    >,
    baking: Query<
        (&crate::world::buildings::Building, &Transform),
        With<crate::world::buildings::PropsPending>,
    >,
    boxes: Query<(), With<CrateBox>>,
    mut props: crate::render::world_props::PropModels,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use crate::render::world_props::Resolved;
    // Nothing of a building's (its shutters, boards, crates) shows before
    // the building does: it'd hang in the air while the walls bake.
    let baking: Vec<(Transform, Vec3)> = baking
        .iter()
        .map(|(b, t)| {
            let f = extraction_core::map::prefab_footprint(&b.prefab);
            (*t, Vec3::from(f) * 0.5 + Vec3::splat(1.0))
        })
        .collect();
    for (e, c, at, children) in &things {
        if baking.iter().any(|(t, half)| {
            let local = t.rotation.inverse() * (at.translation - t.translation);
            local.x.abs() < half.x && local.z.abs() < half.z && local.y.abs() < half.y * 2.0
        }) {
            continue;
        }
        let range = || bevy::camera::visibility::VisibilityRange {
            start_margin: 0.0..0.0,
            end_margin: c.range.clone(),
            use_aabb: true,
        };
        let has_box = children.into_iter().flatten().any(|k| boxes.contains(*k));
        let mut boxed = |props: &mut crate::render::world_props::PropModels| {
            if !has_box {
                let material = props.lib.get(c.mat, &props.server, &mut materials);
                let mut b = commands.spawn((
                    CrateBox,
                    Mesh3d(meshes.add(crate::render::materials::metric_box(c.size))),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    range(),
                    ChildOf(e),
                ));
                if !c.shadow {
                    b.insert(bevy::light::NotShadowCaster);
                }
            }
        };
        let Some(model) = c.model else {
            boxed(&mut props);
            commands.entity(e).insert(CrateDressed);
            continue;
        };
        // `file#node`: one node of a model (a car's door), in the model's
        // space.
        let (model, node) = super::wood::model_node(model);
        match props.resolve_all([model].into_iter()) {
            Resolved::Loading => boxed(&mut props),
            // Missing: the box stays.
            Resolved::Missing => {
                commands.entity(e).insert(CrateDressed);
            }
            Resolved::Ready => {
                let prims = match node {
                    None => props.primitives(model),
                    Some(node) => {
                        let found = props
                            .parts(model)
                            .unwrap_or_default()
                            .into_iter()
                            .find(|p| p.name == node);
                        let Some(part) = found else {
                            // No such node: the box stays, and says so.
                            warn!("{model} has no node {node}");
                            commands.entity(e).insert(CrateDressed);
                            continue;
                        };
                        part.prims
                            .into_iter()
                            .map(|(m, t, mat)| (m, part.at * t, mat))
                            .collect()
                    }
                };
                for k in children.into_iter().flatten() {
                    if boxes.contains(*k) {
                        commands.entity(*k).despawn();
                    }
                }
                for (mesh, t, mat) in prims {
                    // A sash's leaf: its glass is a pane's to draw.
                    if super::glass::drawn_by_panes(model, &mat) {
                        continue;
                    }
                    let material = props.material(&mat, &mut materials);
                    let mut m = commands.spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        c.at * t,
                        range(),
                        ChildOf(e),
                    ));
                    if !c.shadow {
                        m.insert(bevy::light::NotShadowCaster);
                    }
                }
                commands.entity(e).insert(CrateDressed);
            }
        }
    }
}

#[derive(Component)]
pub struct CrateBox;

#[derive(Component)]
pub struct CrateDressed;

/// A building whose walls changed (`breach`): its merged meshes go and are
/// built again round the holes. The glass and doors are their own.
fn redress_breached(
    mut commands: Commands,
    changed: Query<(Entity, &Children), Changed<super::breach::Breached>>,
    meshes: Query<(), (With<Mesh3d>, Without<WindowGlass>)>,
) {
    for (e, children) in &changed {
        for c in children {
            if meshes.contains(*c) {
                commands.entity(*c).despawn();
            }
        }
        crate::world::buildings::redress(&mut commands, e);
    }
}
