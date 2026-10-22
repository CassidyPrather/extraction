//! Every mesh the GPU is handed has something in it. Bevy skips the room
//! for a mesh with no vertex data but still copies it in, and logs "Use-
//! after-free: attempted to copy element data for an unallocated key" each
//! time it's (re)built; a mesh made from a list (footprints, a lightning
//! bolt) is empty whenever its list is. `never_empty` gives such a mesh one
//! zero-area triangle, which draws nothing. In tests, any empty mesh added
//! or changed fails the run. See docs/PERFORMANCE.md, "Empty meshes".

use bevy::mesh::{Indices, VertexAttributeValues as V};
use bevy::prelude::*;

/// Whether the GPU would be handed nothing for this mesh.
pub fn is_empty(mesh: &Mesh) -> bool {
    mesh.count_vertices() == 0 || mesh.indices().is_some_and(|i| i.is_empty())
}

/// `mesh`, or, if it has nothing in it, one zero-area triangle with the
/// same attributes (so the same pipeline): it draws nothing.
pub fn never_empty(mut mesh: Mesh) -> Mesh {
    if !is_empty(&mesh) {
        return mesh;
    }
    if mesh.count_vertices() == 0 {
        let ids: Vec<_> = mesh.attributes().map(|(a, _)| a.clone()).collect();
        for attr in ids {
            let Some(values) = mesh.attribute(attr.id) else {
                continue;
            };
            macro_rules! thrice {
                ($($v:ident),*) => {
                    match values {
                        $(V::$v(_) => V::$v(vec![Default::default(); 3]),)*
                    }
                };
            }
            let filled = thrice!(
                Uint8,
                Uint8x2,
                Uint8x4,
                Sint8,
                Sint8x2,
                Sint8x4,
                Unorm8,
                Unorm8x2,
                Unorm8x4,
                Snorm8,
                Snorm8x2,
                Snorm8x4,
                Uint16,
                Uint16x2,
                Uint16x4,
                Sint16,
                Sint16x2,
                Sint16x4,
                Unorm16,
                Unorm16x2,
                Unorm16x4,
                Snorm16,
                Snorm16x2,
                Snorm16x4,
                Float16,
                Float16x2,
                Float16x4,
                Float32,
                Float32x2,
                Float32x3,
                Float32x4,
                Uint32,
                Uint32x2,
                Uint32x3,
                Uint32x4,
                Sint32,
                Sint32x2,
                Sint32x3,
                Sint32x4,
                Float64,
                Float64x2,
                Float64x3,
                Float64x4,
                Unorm8x4Bgra,
                Unorm10_10_10_2
            );
            mesh.insert_attribute(attr, filled);
        }
    }
    match mesh.indices() {
        Some(Indices::U16(_)) => mesh.insert_indices(Indices::U16(vec![0, 0, 0])),
        Some(Indices::U32(_)) => mesh.insert_indices(Indices::U32(vec![0, 0, 0])),
        None => {}
    }
    mesh
}

pub struct SoundMeshesPlugin;

impl Plugin for SoundMeshesPlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(test)]
        _app.add_systems(Last, no_empty_meshes);
    }
}

/// In tests: an empty mesh added or changed fails the run, naming what
/// draws it.
#[cfg(test)]
fn no_empty_meshes(
    mut events: MessageReader<AssetEvent<Mesh>>,
    meshes: Res<Assets<Mesh>>,
    drawn: Query<(&Mesh3d, Option<&Name>)>,
) {
    for ev in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = ev else {
            continue;
        };
        if meshes.get(*id).is_some_and(is_empty) {
            let by: Vec<String> = drawn
                .iter()
                .filter(|(m, _)| m.id() == *id)
                .map(|(_, n)| n.map_or("(unnamed)".into(), |n| n.to_string()))
                .collect();
            panic!(
                "an empty mesh went to the GPU (drawn by {by:?}): build it with \
                 render::sound_meshes::never_empty"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_mesh_is_given_a_triangle_that_draws_nothing() {
        let m = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new())
        .with_inserted_indices(Indices::U32(vec![]));
        assert!(is_empty(&m));
        let m = never_empty(m);
        assert!(!is_empty(&m));
        assert_eq!(m.count_vertices(), 3);
        assert_eq!(m.attributes().count(), 2);
        let Some(V::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("positions")
        };
        assert!(p.iter().all(|q| *q == p[0]), "zero area");
    }

    /// A raid's first minute in Marrow Creek, snow and storm and all,
    /// hands the GPU no empty mesh (`no_empty_meshes` fails it if so).
    #[test]
    fn no_empty_mesh_reaches_the_gpu() {
        let _one_at_a_time = crate::render::budget::tests::heavy();
        let mut app = crate::render::budget::tests::client_threads("marrow_creek", "storm", None);
        crate::render::budget::tests::settle(&mut app);
        let meshes = app.world().resource::<Assets<Mesh>>();
        let empty = meshes.iter().filter(|(_, m)| is_empty(m)).count();
        assert_eq!(empty, 0, "{empty} empty meshes of {}", meshes.len());
    }
}
