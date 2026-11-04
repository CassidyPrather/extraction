//! Overdraw: how many surfaces each pixel of an eye is shaded for.
//!
//! The fill cost of a view (and what MSAA multiplies) scales with the
//! fragments the GPU shades, not the pixels on screen. This rasterises
//! everything the camera draws, on the CPU, at a coarse resolution, depth
//! test off, back faces culled where the material culls them, and counts
//! layers per pixel: the mean is the frame's fragment load in screens,
//! the p95 and max where it piles up. Alpha-tested cards count their whole
//! quad: a discarded fragment has still been shaded.
//!
//! Positions are the meshes' own, so shader-moved geometry (rain streaks,
//! grass widening, swaying crowns) counts where it sits at rest.

use bevy::camera::visibility::VisibleEntities;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use std::collections::HashMap;

/// Pixels a side: coarse, but the counts are areas, which converge fast.
pub const RES: usize = 192;

#[derive(Clone, Debug, Default)]
pub struct Overdraw {
    pub mean: f32,
    pub p95: u16,
    pub max: u16,
    /// Layers a pixel (mean over the screen) by class, biggest first.
    pub by: Vec<(String, f32)>,
}

impl Overdraw {
    pub fn summary(&self) -> String {
        format!(
            "overdraw mean {:.2}, p95 {}, max {} layers a pixel",
            self.mean, self.p95, self.max
        )
    }
}

/// What a mesh is, for the tally: the material kind where it says (cards,
/// grass, impostors), else what it's part of.
fn class(w: &World, e: Entity) -> String {
    let r = w.entity(e);
    if r.contains::<MeshMaterial3d<crate::world::trees::TreeSwayMaterial>>() {
        return "tree cards".into();
    }
    if r.contains::<MeshMaterial3d<crate::world::trees::FarCrownMaterial>>() {
        return "near crowns".into();
    }
    if r.contains::<MeshMaterial3d<crate::world::cover::SwayMaterial>>() {
        return "ground cover".into();
    }
    if r.contains::<MeshMaterial3d<crate::world::impostors::ImpostorMaterial>>() {
        return "tree impostors".into();
    }
    if r.contains::<MeshMaterial3d<crate::render::terrain_material::TerrainMaterial>>() {
        return "terrain".into();
    }
    if r.contains::<MeshMaterial3d<crate::world::sky::SkyMaterial>>() {
        return "sky".into();
    }
    // The nearest named ancestor's first word.
    let mut at = e;
    for _ in 0..32 {
        if let Some(n) = w.get::<Name>(at) {
            return n.as_str().split(' ').next().unwrap_or("").to_string();
        }
        match w.get::<ChildOf>(at) {
            Some(p) => at = p.parent(),
            None => break,
        }
    }
    "(unnamed)".into()
}

/// Whether the mesh's material draws both sides.
fn both_sides(w: &World, e: Entity) -> bool {
    fn cull<M: Asset>(w: &World, e: Entity, base: impl Fn(&M) -> &StandardMaterial) -> Option<bool>
    where
        MeshMaterial3d<M>: Component,
        M: Material,
    {
        let h = w.get::<MeshMaterial3d<M>>(e)?;
        let m = w.resource::<Assets<M>>().get(&h.0)?;
        Some(base(m).cull_mode.is_none())
    }
    use crate::world::{cover, impostors, trees};
    cull::<StandardMaterial>(w, e, |m| m)
        .or_else(|| cull::<trees::TreeSwayMaterial>(w, e, |m| &m.base))
        .or_else(|| cull::<trees::FarCrownMaterial>(w, e, |m| &m.base))
        .or_else(|| cull::<cover::SwayMaterial>(w, e, |m| &m.base))
        .or_else(|| cull::<impostors::ImpostorMaterial>(w, e, |m| &m.base))
        .unwrap_or(false)
}

/// Whether the mesh's material is opaque (writes depth wherever it
/// covers, so an opaque thing behind it is rejected before shading).
/// Alpha-tested and blended materials aren't: the sky shows, and is
/// shaded, through a card's holes.
fn opaque(w: &World, e: Entity) -> bool {
    fn mode<M: Asset>(w: &World, e: Entity, base: impl Fn(&M) -> &StandardMaterial) -> Option<bool>
    where
        MeshMaterial3d<M>: Component,
        M: Material,
    {
        let h = w.get::<MeshMaterial3d<M>>(e)?;
        let m = w.resource::<Assets<M>>().get(&h.0)?;
        Some(matches!(base(m).alpha_mode, AlphaMode::Opaque))
    }
    use crate::world::{cover, impostors, trees};
    mode::<StandardMaterial>(w, e, |m| m)
        .or_else(|| mode::<crate::render::terrain_material::TerrainMaterial>(w, e, |m| &m.base))
        .or_else(|| mode::<trees::TreeSwayMaterial>(w, e, |m| &m.base))
        .or_else(|| mode::<trees::FarCrownMaterial>(w, e, |m| &m.base))
        .or_else(|| mode::<cover::SwayMaterial>(w, e, |m| &m.base))
        .or_else(|| mode::<impostors::ImpostorMaterial>(w, e, |m| &m.base))
        .unwrap_or(false)
}

/// The sky, counted as the GPU shades it: one layer on the pixels it
/// covers that nothing opaque does. It's an opaque dome drawn behind
/// everything, so early-z rejects it under anything opaque; through
/// alpha-tested cards, blended things or nothing, it's shaded. Counted
/// as a full layer everywhere (no depth test), a dense spruce view paid
/// for a sky its trees hide.
fn add_sky(layers: &mut [u16], sky: &[u16], cover: &[u16]) -> u64 {
    let mut n = 0;
    for ((l, s), c) in layers.iter_mut().zip(sky).zip(cover) {
        if *s > 0 && *c == 0 {
            *l = l.saturating_add(1);
            n += 1;
        }
    }
    n
}

/// For an impostor mesh: the eye (x, z), the near cut, and each vertex's
/// tree base.
fn impostor_cut(
    w: &World,
    e: Entity,
    mesh: &Mesh,
    eye: Vec2,
) -> Option<(Vec2, f32, Vec<[f32; 2]>)> {
    let h = w.get::<MeshMaterial3d<crate::world::impostors::ImpostorMaterial>>(e)?;
    let m = w
        .resource::<Assets<crate::world::impostors::ImpostorMaterial>>()
        .get(&h.0)?;
    let Some(VertexAttributeValues::Float32x2(b)) = mesh.attribute(Mesh::ATTRIBUTE_UV_1) else {
        return None;
    };
    Some((eye, m.extension.cut.x, b.clone()))
}

/// Err when this checkout can't be measured honestly: the fetched
/// textures are missing (`capture::assets::fetched`), so the sky and the
/// cards aren't drawn as the game draws them. Checked once a run.
/// `EXTRACTION_ALLOW_FALLBACK=1` measures anyway.
pub fn measurable(root: &std::path::Path) -> Result<(), String> {
    if std::env::var_os("EXTRACTION_ALLOW_FALLBACK").is_some() {
        return Ok(());
    }
    crate::capture::assets::fetched(
        root,
        "the frame budgets would read low and pass where they should fail",
    )
}

/// The overdraw of what `cam` sees. Panics on a checkout without the
/// fetched textures (`measurable`): every fragment budget counts here,
/// and a bare checkout reads 12M a stereo frame low.
pub fn measure(w: &mut World, cam: Entity) -> Overdraw {
    static READY: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    if let Err(e) = READY.get_or_init(|| measurable(&crate::asset_root_path())) {
        panic!("{e}");
    }
    let eye_pos;
    let (view, f, eye) = {
        let g = w.get::<GlobalTransform>(cam).unwrap();
        let Projection::Perspective(p) = w.get::<Projection>(cam).unwrap() else {
            return Overdraw::default();
        };
        let eye = g.translation();
        eye_pos = eye;
        (
            g.affine().inverse(),
            1.0 / (p.fov * 0.5).tan(),
            Vec2::new(eye.x, eye.z),
        )
    };
    let near = crate::render::NEAR_CLIP;
    let seen: Vec<Entity> = w
        .get::<VisibleEntities>(cam)
        .unwrap()
        .get(std::any::TypeId::of::<Mesh3d>())
        .to_vec();
    let mut layers = vec![0u16; RES * RES];
    // The sky, and what opaque covers, apart: see `add_sky`.
    let mut sky = vec![0u16; RES * RES];
    let mut cover = vec![0u16; RES * RES];
    let mut by: HashMap<String, u64> = HashMap::new();
    for e in seen {
        let Some(m) = w.get::<Mesh3d>(e) else {
            continue;
        };
        if w.get::<crate::render::warmup::ShelfMark>(e).is_some() {
            continue;
        }
        // Streaks are counted below, as their shader places them.
        if w.get::<MeshMaterial3d<crate::world::rain::RainMaterial>>(e)
            .is_some()
        {
            continue;
        }
        let Some(mesh) = w.resource::<Assets<Mesh>>().get(&m.0) else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        let model = w.get::<GlobalTransform>(e).unwrap().affine();
        let mut world: Vec<Vec3> = pos
            .iter()
            .map(|p| model.transform_point3(Vec3::from(*p)))
            .collect();
        // Cards near the eye shrink to their centres in theirs.
        if w.get::<MeshMaterial3d<crate::world::trees::TreeSwayMaterial>>(e)
            .is_some()
        {
            if let Some(VertexAttributeValues::Float32x3(centres)) =
                mesh.attribute(crate::world::trees::ATTRIBUTE_CARD_CENTRE)
            {
                for (p, c) in world.iter_mut().zip(centres) {
                    let c = model.transform_point3(Vec3::from(*c));
                    let left = crate::world::trees::card_scale(c.distance(eye_pos));
                    *p = c + (*p - c) * left;
                }
            }
        }
        // Impostors turn to face the eye in their vertex shader.
        if w.get::<MeshMaterial3d<crate::world::impostors::ImpostorMaterial>>(e)
            .is_some()
        {
            if let (
                Some(VertexAttributeValues::Float32x2(uv)),
                Some(VertexAttributeValues::Float32x2(bases)),
            ) = (
                mesh.attribute(Mesh::ATTRIBUTE_UV_0),
                mesh.attribute(Mesh::ATTRIBUTE_UV_1),
            ) {
                for (i, p) in world.iter_mut().enumerate() {
                    *p = crate::world::impostors::face(*p, Vec2::from(bases[i]), uv[i][0], eye);
                }
            }
        }
        let vs: Vec<Vec3> = world.iter().map(|p| view.transform_point3(*p)).collect();
        let idx: Vec<usize> = match mesh.indices() {
            Some(i) => i.iter().collect(),
            None => (0..vs.len()).collect(),
        };
        let two = both_sides(w, e);
        // Impostor quads of trees within the near cut collapse in the
        // vertex shader (`impostor.wgsl`): they make no fragments.
        let cut = impostor_cut(w, e, mesh, eye);
        let is_sky = w
            .get::<MeshMaterial3d<crate::world::sky::SkyMaterial>>(e)
            .is_some();
        let occludes = !is_sky && opaque(w, e);
        let mut covered = 0u64;
        for t in idx.chunks_exact(3) {
            if let Some((eye, near, bases)) = &cut {
                if eye.distance(Vec2::from(bases[t[0]])) < *near {
                    continue;
                }
            }
            let tri = [vs[t[0]], vs[t[1]], vs[t[2]]];
            if is_sky {
                raster(&tri, near, f, two, &mut sky);
                continue;
            }
            covered += raster(&tri, near, f, two, &mut layers);
            if occludes {
                raster(&tri, near, f, two, &mut cover);
            }
        }
        if covered > 0 {
            *by.entry(class(w, e)).or_default() += covered;
        }
    }
    let shown = add_sky(&mut layers, &sky, &cover);
    if shown > 0 {
        *by.entry("sky".into()).or_default() += shown;
    }
    // The worst weather, wherever the gauge's own is: a downpour's
    // streaks, or (in a burnt valley, when it isn't raining) the ash.
    // They live in a box round the eye, placed by their vertex shader, so
    // they're replayed here rather than read from the scene.
    let ash = w
        .get_resource::<crate::world::Zone>()
        .map_or(0.0, |z| z.map.ash);
    let downpour = (
        Vec4::new(1.0, 9.0, 0.55, crate::world::rain::BOX),
        Vec4::new(3.2, 0.0, 1.6, 0.0),
    );
    let ashfall = (
        Vec4::new(ash, 0.55, 0.035, crate::world::rain::BOX),
        Vec4::new(0.35, 1.0, 0.15, 0.03),
    );
    let mut worst = (String::new(), vec![0u16; 0], 0u64);
    for (name, (params, wind)) in [("rain (downpour)", downpour), ("ash", ashfall)] {
        if params.x <= 0.0 {
            continue;
        }
        let mut l = vec![0u16; RES * RES];
        let n = streaks(params, wind, eye_pos, &view, f, &mut l);
        if n > worst.2 {
            worst = (name.to_string(), l, n);
        }
    }
    if worst.2 > 0 {
        for (a, b) in layers.iter_mut().zip(&worst.1) {
            *a = a.saturating_add(*b);
        }
        by.insert(worst.0, worst.2);
    }
    let px = (RES * RES) as f32;
    let mut sorted = layers.clone();
    sorted.sort_unstable();
    let mut by: Vec<(String, f32)> = by.into_iter().map(|(k, v)| (k, v as f32 / px)).collect();
    by.sort_by(|a, b| b.1.total_cmp(&a.1));
    Overdraw {
        mean: layers.iter().map(|&l| l as f32).sum::<f32>() / px,
        p95: sorted[(sorted.len() as f32 * 0.95) as usize],
        max: *sorted.last().unwrap_or(&0),
        by,
    }
}

/// Rain or ash streaks round the eye at time zero, placed and faced as
/// `rain.wgsl` does (every streak the intensity keeps; shelters ignored:
/// the open is the worst case). A streak right at the eye fades to
/// nothing but is still rasterised, so it counts. Returns pixels covered.
fn streaks(
    params: Vec4,
    wind: Vec4,
    eye: Vec3,
    view: &bevy::math::Affine3A,
    f: f32,
    layers: &mut [u16],
) -> u64 {
    let mesh = crate::world::rain::streak_mesh();
    let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return 0;
    };
    let fall = Vec3::new(wind.x, -params.y, wind.z);
    let axis = (-fall).normalize();
    let width = if wind.w > 0.0 { wind.w } else { 0.006 };
    let box_size = params.w;
    let mut covered = 0;
    for corners in pos.chunks_exact(4) {
        let base = Vec3::from(corners[0]);
        let keep = ((base.x * 12.9898 + base.z * 78.233).sin() * 43758.547)
            .fract()
            .abs();
        if keep > params.x {
            continue;
        }
        let mut p = base;
        if wind.y > 0.5 {
            let ph = base.x * 3.1 + base.z * 1.7;
            p += Vec3::new(ph.sin(), 0.0, (ph * 1.3).cos()) * 0.35;
        }
        let rel = p - eye;
        let rel = ((rel / box_size + 0.5).fract_gl() - 0.5) * box_size;
        let centre = eye + rel;
        let to_cam = (eye - centre).normalize_or(Vec3::Z);
        let side = axis.cross(to_cam).normalize_or(Vec3::X);
        let at = |u: f32, v: f32| {
            view.transform_point3(centre + side * (u - 0.5) * width + axis * v * params.z)
        };
        let q = [at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)];
        covered += raster(
            &[q[0], q[1], q[2]],
            crate::render::NEAR_CLIP,
            f,
            true,
            layers,
        );
        covered += raster(
            &[q[0], q[2], q[3]],
            crate::render::NEAR_CLIP,
            f,
            true,
            layers,
        );
    }
    covered
}

/// Rasterises one view-space triangle (the camera looks down -Z), clipped
/// at the near plane; returns the pixels it covered.
fn raster(tri: &[Vec3; 3], near: f32, f: f32, two: bool, layers: &mut [u16]) -> u64 {
    // Clip against z = -near (keep z <= -near).
    let mut poly: Vec<Vec3> = Vec::with_capacity(4);
    for i in 0..3 {
        let (a, b) = (tri[i], tri[(i + 1) % 3]);
        let (ina, inb) = (a.z <= -near, b.z <= -near);
        if ina {
            poly.push(a);
        }
        if ina != inb {
            let t = (-near - a.z) / (b.z - a.z);
            poly.push(a + (b - a) * t);
        }
    }
    if poly.len() < 3 {
        return 0;
    }
    // To pixels (y down).
    let half = RES as f32 * 0.5;
    let pts: Vec<Vec2> = poly
        .iter()
        .map(|p| Vec2::new(half + p.x * f / -p.z * half, half - p.y * f / -p.z * half))
        .collect();
    let mut covered = 0;
    for i in 1..pts.len() - 1 {
        let (a, b, c) = (pts[0], pts[i], pts[i + 1]);
        // Counter-clockwise in view space is clockwise with y down.
        let area = (b - a).perp_dot(c - a);
        if area == 0.0 || (!two && area > 0.0) {
            continue;
        }
        let lo = a.min(b).min(c).max(Vec2::ZERO);
        let hi = a.max(b).max(c).min(Vec2::splat(RES as f32 - 0.5));
        if lo.x > hi.x || lo.y > hi.y {
            continue;
        }
        let s = area.signum();
        for y in
            (lo.y - 0.5).ceil().max(0.0) as usize..=((hi.y - 0.5).floor() as usize).min(RES - 1)
        {
            for x in
                (lo.x - 0.5).ceil().max(0.0) as usize..=((hi.x - 0.5).floor() as usize).min(RES - 1)
            {
                // Just off the centre, so an edge through it counts once.
                let p = Vec2::new(x as f32 + 0.5001, y as f32 + 0.5002);
                let e0 = (b - a).perp_dot(p - a) * s;
                let e1 = (c - b).perp_dot(p - b) * s;
                let e2 = (a - c).perp_dot(p - c) * s;
                if e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0 {
                    let l = &mut layers[y * RES + x];
                    *l = l.saturating_add(1);
                    covered += 1;
                }
            }
        }
    }
    covered
}

#[cfg(test)]
mod tests {

    /// A checkout without the fetched textures isn't measured: the error
    /// names the command that makes them. One with them is.
    #[test]
    fn a_checkout_without_textures_is_not_measured() {
        if std::env::var_os("EXTRACTION_ALLOW_FALLBACK").is_some() {
            return;
        }
        let root = crate::asset_root_path();
        let bare = std::env::temp_dir().join(format!("overdraw_bare_{}", std::process::id()));
        let assets = bare.join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::create_dir_all(bare.join("scripts")).unwrap();
        std::fs::copy(
            root.join("../scripts/assets_manifest.json"),
            bare.join("scripts/assets_manifest.json"),
        )
        .unwrap();
        let err = super::measurable(&assets);
        let _ = std::fs::remove_dir_all(&bare);
        let err = err.unwrap_err();
        assert!(err.contains("fetch_assets.py"), "{err}");
        assert!(err.contains("sky"), "{err}");
        assert_eq!(super::measurable(&root), Ok(()));
    }
    use super::*;

    /// The streak replay against the analytic mean (streak area times
    /// density over the frustum's directions and depth), for streaks wide
    /// enough to sample well, so a near-zero rain figure is real. The
    /// analytic sum treats streaks as points, which undercounts the few
    /// within a hand's width of the eye (they cover much of the view and
    /// are rasterised, faded or not): it's a floor, the replay about twice it.
    #[test]
    fn streaks_match_the_analytic_mean() {
        let (b, n) = (crate::world::rain::BOX, crate::world::rain::STREAKS as f32);
        let (width, len) = (0.3, 0.55);
        let params = Vec4::new(1.0, 9.0, len, b);
        let wind = Vec4::new(0.0, 0.0, 0.0, width);
        let fov = super::super::budget::EYE_FOV_DEG.to_radians();
        let f = 1.0 / (fov * 0.5).tan();
        let view = bevy::math::Affine3A::IDENTITY;
        let mut layers = vec![0u16; RES * RES];
        let px = streaks(params, wind, Vec3::ZERO, &view, f, &mut layers);
        let mean = px as f32 / (RES * RES) as f32;
        // Layers = density x area x (f^2 / 4) x the integral over the
        // frustum's directions of how deep the box goes that way.
        let steps = 200;
        let mut integral = 0.0;
        for i in 0..steps {
            for j in 0..steps {
                let x = (i as f32 + 0.5) / steps as f32 * 2.0 - 1.0;
                let y = (j as f32 + 0.5) / steps as f32 * 2.0 - 1.0;
                let d = Vec3::new(x / f, y / f, -1.0);
                let r = d.length();
                let dir = d / r;
                let deep = b * 0.5 / dir.abs().max_element();
                // Solid angle of this NDC cell.
                let cell = (2.0 / steps as f32).powi(2) / (f * f) / (r * r * r);
                integral += deep * cell;
            }
        }
        let expect = n / (b * b * b) * width * len * f * f / 4.0 * integral;
        assert!(
            mean > expect * 0.9 && mean < expect * 3.0,
            "streaks: {mean:.3} layers, analytic {expect:.3}"
        );
    }

    /// A quad filling the view counts one layer everywhere, seen from
    /// either side when two-sided, and none from behind when not.
    #[test]
    fn a_full_screen_quad_is_one_layer() {
        let f = 1.0;
        let z = -2.0;
        // Bigger than the view at that depth, counter-clockwise from the
        // camera.
        let q = [
            Vec3::new(-5.0, -5.0, z),
            Vec3::new(5.0, -5.0, z),
            Vec3::new(5.0, 5.0, z),
            Vec3::new(-5.0, 5.0, z),
        ];
        let mut layers = vec![0u16; RES * RES];
        raster(&[q[0], q[1], q[2]], 0.03, f, false, &mut layers);
        raster(&[q[0], q[2], q[3]], 0.03, f, false, &mut layers);
        assert!(layers.iter().all(|&l| l == 1), "front face: one layer");
        let mut back = vec![0u16; RES * RES];
        raster(&[q[0], q[2], q[1]], 0.03, f, false, &mut back);
        assert!(back.iter().all(|&l| l == 0), "back face culled");
        raster(&[q[0], q[2], q[1]], 0.03, f, true, &mut back);
        raster(&[q[0], q[3], q[2]], 0.03, f, true, &mut back);
        assert!(back.iter().all(|&l| l == 1), "two-sided from behind");
    }

    /// The sky counts a layer only where nothing opaque covers it: a
    /// wall over the left half leaves it the right half (it counted the
    /// whole screen, a full layer, before), and an alpha-tested card,
    /// which shades the sky through its holes, hides none of it.
    #[test]
    fn the_sky_counts_where_nothing_opaque_hides_it() {
        let n = RES * RES;
        let sky = vec![1u16; n];
        let mut wall = vec![0u16; n];
        for (i, c) in wall.iter_mut().enumerate() {
            if i % RES < RES / 2 {
                *c = 1;
            }
        }
        let mut layers = wall.clone();
        let shown = add_sky(&mut layers, &sky, &wall);
        assert_eq!(shown as usize, n / 2, "the sky past the wall");
        assert!(layers.iter().all(|&l| l == 1), "one layer everywhere");
        // A card over everything, not opaque: nothing in the cover.
        let mut layers = vec![1u16; n];
        let shown = add_sky(&mut layers, &sky, &vec![0u16; n]);
        assert_eq!(shown as usize, n, "through a card, the whole sky");
    }
}
