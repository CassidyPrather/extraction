//! Reading a `.glb`'s JSON chunk without loading it: each mesh node's
//! bounds in the model's space (through the node hierarchy), its material
//! names and its images. For checks that want a model's size and shape
//! before anything renders (`capture::checks`, the sweep's report).

use bevy::math::{Mat4, Quat, Vec3};
use std::path::Path;

/// One mesh node: its name and bounds in the model's space.
#[derive(Clone, Debug)]
pub struct NodeBounds {
    pub name: String,
    pub lo: Vec3,
    pub hi: Vec3,
    pub materials: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct GlbInfo {
    pub nodes: Vec<NodeBounds>,
    /// Image URIs the file points outside itself for (embedded ones are
    /// listed as `""`).
    pub images: Vec<String>,
}

impl GlbInfo {
    /// The whole model's bounds, `None` if it has no meshes.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.bounds_of(|_| true)
    }

    /// Bounds of the nodes `keep` keeps (`_lod` stand-ins left out, say).
    pub fn bounds_of(&self, keep: impl Fn(&NodeBounds) -> bool) -> Option<(Vec3, Vec3)> {
        self.nodes
            .iter()
            .filter(|n| keep(n))
            .map(|n| (n.lo, n.hi))
            .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
    }
}

pub fn json(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return Err(format!("{} is not a glb", path.display()));
    }
    let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let end = (20 + len).min(bytes.len());
    serde_json::from_slice(&bytes[20..end]).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read(path: &Path) -> Result<GlbInfo, String> {
    let j = json(path)?;
    let f = |v: &serde_json::Value, i: usize, d: f32| {
        v.get(i).and_then(|x| x.as_f64()).map_or(d, |x| x as f32)
    };
    let local = |n: &serde_json::Value| -> Mat4 {
        if let Some(m) = n["matrix"].as_array() {
            let a: Vec<f32> = m.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
            if a.len() == 16 {
                return Mat4::from_cols_slice(&a);
            }
        }
        let (t, r, s) = (&n["translation"], &n["rotation"], &n["scale"]);
        Mat4::from_scale_rotation_translation(
            Vec3::new(f(s, 0, 1.0), f(s, 1, 1.0), f(s, 2, 1.0)),
            Quat::from_xyzw(f(r, 0, 0.0), f(r, 1, 0.0), f(r, 2, 0.0), f(r, 3, 1.0)),
            Vec3::new(f(t, 0, 0.0), f(t, 1, 0.0), f(t, 2, 0.0)),
        )
    };
    let nodes = j["nodes"].as_array().cloned().unwrap_or_default();
    let materials: Vec<String> = j["materials"]
        .as_array()
        .map(|ms| {
            ms.iter()
                .map(|m| m["name"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    // Roots: the scene's, or every node nobody lists as a child.
    let mut roots: Vec<usize> = j["scenes"][j["scene"].as_u64().unwrap_or(0) as usize]["nodes"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as usize)
                .collect()
        })
        .unwrap_or_default();
    if roots.is_empty() {
        let children: std::collections::HashSet<u64> = nodes
            .iter()
            .flat_map(|n| n["children"].as_array().cloned().unwrap_or_default())
            .filter_map(|c| c.as_u64())
            .collect();
        roots = (0..nodes.len())
            .filter(|i| !children.contains(&(*i as u64)))
            .collect();
    }
    let mut out = GlbInfo::default();
    let mut stack: Vec<(usize, Mat4, u32)> =
        roots.into_iter().map(|r| (r, Mat4::IDENTITY, 0)).collect();
    while let Some((i, parent, depth)) = stack.pop() {
        let Some(n) = nodes.get(i) else { continue };
        if depth > 64 {
            continue;
        }
        let world = parent * local(n);
        if let Some(children) = n["children"].as_array() {
            for c in children.iter().filter_map(|c| c.as_u64()) {
                stack.push((c as usize, world, depth + 1));
            }
        }
        let Some(mesh) = n["mesh"].as_u64() else {
            continue;
        };
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let mut mats = Vec::new();
        for prim in j["meshes"][mesh as usize]["primitives"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            if let Some(m) = prim["material"].as_u64() {
                mats.push(materials.get(m as usize).cloned().unwrap_or_default());
            }
            let Some(acc) = prim["attributes"]["POSITION"].as_u64() else {
                continue;
            };
            let acc = &j["accessors"][acc as usize];
            let (a, b) = (&acc["min"], &acc["max"]);
            for k in 0..8 {
                let c = Vec3::new(
                    if k & 1 == 0 {
                        f(a, 0, 0.0)
                    } else {
                        f(b, 0, 0.0)
                    },
                    if k & 2 == 0 {
                        f(a, 1, 0.0)
                    } else {
                        f(b, 1, 0.0)
                    },
                    if k & 4 == 0 {
                        f(a, 2, 0.0)
                    } else {
                        f(b, 2, 0.0)
                    },
                );
                let w = world.transform_point3(c);
                lo = lo.min(w);
                hi = hi.max(w);
            }
        }
        if lo.x <= hi.x {
            out.nodes.push(NodeBounds {
                name: n["name"].as_str().unwrap_or("").to_string(),
                lo,
                hi,
                materials: mats,
            });
        }
    }
    out.images = j["images"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|i| i["uri"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    Ok(out)
}

/// Every `.glb` under `assets/models/`, as (folder, name, path), sorted.
pub fn all_models() -> Vec<(String, String, std::path::PathBuf)> {
    let root = crate::asset_root_path().join("models");
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(&root) else {
        return out;
    };
    for d in dirs.flatten() {
        let dir = d.path();
        if !dir.is_dir() {
            continue;
        }
        let folder = d.file_name().to_string_lossy().into_owned();
        for f in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = f.path();
            if p.extension().is_some_and(|e| e == "glb") {
                let name = p.file_stem().unwrap().to_string_lossy().into_owned();
                out.push((folder.clone(), name, p));
            }
        }
    }
    out.sort();
    out
}

/// One triangle of a model, in the model's space.
#[derive(Clone, Debug)]
pub struct Tri {
    /// The mesh node it belongs to.
    pub node: String,
    pub material: String,
    pub p: [Vec3; 3],
}

/// Every triangle of a model's meshes (triangle-list primitives), through
/// the node hierarchy.
pub fn triangles(path: &Path) -> Result<Vec<Tri>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let j = json(path)?;
    let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    // The BIN chunk follows the JSON one (padded to four bytes).
    let at = 20 + len.div_ceil(4) * 4;
    let bin: &[u8] = if bytes.len() >= at + 8 && &bytes[at + 4..at + 8] == b"BIN\0" {
        let n = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        &bytes[at + 8..(at + 8 + n).min(bytes.len())]
    } else {
        &[]
    };
    let num = |v: &serde_json::Value| v.as_u64().unwrap_or(0) as usize;
    // An accessor's elements as f32 components (`comps` a row).
    let read = |acc: usize| -> Option<(Vec<f32>, usize)> {
        let a = &j["accessors"][acc];
        let view = &j["bufferViews"][a["bufferView"].as_u64()? as usize];
        let comps = match a["type"].as_str()? {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            _ => return None,
        };
        let ctype = a["componentType"].as_u64()?;
        let size = match ctype {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => return None,
        };
        let count = num(&a["count"]);
        let start = num(&view["byteOffset"]) + num(&a["byteOffset"]);
        let stride = view["byteStride"]
            .as_u64()
            .map_or(comps * size, |s| s as usize);
        let mut out = Vec::with_capacity(count * comps);
        for i in 0..count {
            for c in 0..comps {
                let o = start + i * stride + c * size;
                let b = bin.get(o..o + size)?;
                out.push(match ctype {
                    5126 => f32::from_le_bytes(b.try_into().ok()?),
                    5125 => u32::from_le_bytes(b.try_into().ok()?) as f32,
                    5123 => u16::from_le_bytes(b.try_into().ok()?) as f32,
                    5122 => i16::from_le_bytes(b.try_into().ok()?) as f32,
                    5121 => b[0] as f32,
                    _ => b[0] as i8 as f32,
                });
            }
        }
        Some((out, comps))
    };
    let f = |v: &serde_json::Value, i: usize, d: f32| {
        v.get(i).and_then(|x| x.as_f64()).map_or(d, |x| x as f32)
    };
    let nodes = j["nodes"].as_array().cloned().unwrap_or_default();
    let materials: Vec<String> = j["materials"]
        .as_array()
        .map(|ms| {
            ms.iter()
                .map(|m| m["name"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    let roots: Vec<usize> = j["scenes"][j["scene"].as_u64().unwrap_or(0) as usize]["nodes"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_u64())
                .map(|v| v as usize)
                .collect()
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    let mut stack: Vec<(usize, Mat4)> = roots.into_iter().map(|r| (r, Mat4::IDENTITY)).collect();
    while let Some((i, parent)) = stack.pop() {
        let Some(n) = nodes.get(i) else { continue };
        let (t, r, s) = (&n["translation"], &n["rotation"], &n["scale"]);
        let world = parent
            * Mat4::from_scale_rotation_translation(
                Vec3::new(f(s, 0, 1.0), f(s, 1, 1.0), f(s, 2, 1.0)),
                Quat::from_xyzw(f(r, 0, 0.0), f(r, 1, 0.0), f(r, 2, 0.0), f(r, 3, 1.0)),
                Vec3::new(f(t, 0, 0.0), f(t, 1, 0.0), f(t, 2, 0.0)),
            );
        for c in n["children"].as_array().into_iter().flatten() {
            if let Some(c) = c.as_u64() {
                stack.push((c as usize, world));
            }
        }
        let Some(mesh) = n["mesh"].as_u64() else {
            continue;
        };
        let name = n["name"].as_str().unwrap_or("").to_string();
        for prim in j["meshes"][mesh as usize]["primitives"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if prim["mode"].as_u64().unwrap_or(4) != 4 {
                continue;
            }
            let Some((pos, 3)) = prim["attributes"]["POSITION"]
                .as_u64()
                .and_then(|a| read(a as usize))
            else {
                continue;
            };
            let verts: Vec<Vec3> = pos
                .chunks(3)
                .map(|c| world.transform_point3(Vec3::new(c[0], c[1], c[2])))
                .collect();
            let idx: Vec<usize> = match prim["indices"].as_u64().and_then(|a| read(a as usize)) {
                Some((i, 1)) => i.into_iter().map(|v| v as usize).collect(),
                _ => (0..verts.len()).collect(),
            };
            let material = prim["material"]
                .as_u64()
                .and_then(|m| materials.get(m as usize).cloned())
                .unwrap_or_default();
            for t in idx.chunks(3) {
                if let (3, Some(a), Some(b), Some(c)) =
                    (t.len(), verts.get(t[0]), verts.get(t[1]), verts.get(t[2]))
                {
                    out.push(Tri {
                        node: name.clone(),
                        material: material.clone(),
                        p: [*a, *b, *c],
                    });
                }
            }
        }
    }
    Ok(out)
}
