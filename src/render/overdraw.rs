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

/// The overdraw of what `cam` sees.
pub fn measure(w: &mut World, cam: Entity) -> Overdraw {
    let (view, f, eye) = {
        let g = w.get::<GlobalTransform>(cam).unwrap();
        let Projection::Perspective(p) = w.get::<Projection>(cam).unwrap() else {
            return Overdraw::default();
        };
        let eye = g.translation();
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
    let mut by: HashMap<String, u64> = HashMap::new();
    for e in seen {
        let Some(m) = w.get::<Mesh3d>(e) else {
            continue;
        };
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
        let mut covered = 0u64;
        for t in idx.chunks_exact(3) {
            if let Some((eye, near, bases)) = &cut {
                if eye.distance(Vec2::from(bases[t[0]])) < *near {
                    continue;
                }
            }
            let tri = [vs[t[0]], vs[t[1]], vs[t[2]]];
            covered += raster(&tri, near, f, two, &mut layers);
        }
        if covered > 0 {
            *by.entry(class(w, e)).or_default() += covered;
        }
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
    use super::*;

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
}
