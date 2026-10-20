//! Hierarchical LOD: far stand-ins that draw a whole object in one mesh.
//!
//! A building is half a dozen meshes (one a material, windows apart);
//! past [`FAR_BUILDING`] its textures are a few pixels a metre and the fog
//! has most of its contrast, so one vertex-coloured mesh of its shell
//! stands in, coloured with each material's average albedo. Villages seen
//! from a rise were 150-250 draws of buildings. The forest's far LOD
//! (trunks per bark, crowns) is one too, a chunk.

use bevy::mesh::{Mesh, VertexAttributeValues};
use bevy::prelude::*;

/// Buildings swap to their stand-in round this distance (to the bounds'
/// centre), cross-faded over 20 m.
pub const FAR_BUILDING: f32 = 120.0;

/// The near meshes' range when a stand-in takes over from them.
pub fn near_range() -> bevy::camera::visibility::VisibilityRange {
    bevy::camera::visibility::VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: (FAR_BUILDING - 10.0)..(FAR_BUILDING + 10.0),
        use_aabb: true,
    }
}

/// The stand-in's range.
pub fn far_range() -> bevy::camera::visibility::VisibilityRange {
    bevy::camera::visibility::VisibilityRange {
        start_margin: (FAR_BUILDING - 10.0)..(FAR_BUILDING + 10.0),
        end_margin: 1.0e4..1.0e4,
        use_aabb: true,
    }
}

/// Mean linear albedo of each texture set in `assets/textures/materials/`
/// (as `scripts/fetch_assets.py` reports it when it builds them).
const ALBEDO: &[(&str, [f32; 3])] = &[
    ("bark_spruce", [0.201, 0.141, 0.087]),
    ("bark_birch", [0.558, 0.537, 0.493]),
    ("bark_pine", [0.139, 0.089, 0.054]),
    ("weathered_planks", [0.221, 0.195, 0.140]),
    ("log_wall", [0.272, 0.238, 0.160]),
    ("red_brick", [0.255, 0.170, 0.107]),
    ("concrete_wet", [0.219, 0.191, 0.133]),
    ("rusty_metal", [0.106, 0.074, 0.057]),
    ("painted_metal_peeling", [0.075, 0.154, 0.102]),
    ("corrugated_iron", [0.137, 0.040, 0.022]),
    ("roof_tiles_or_shingles", [0.127, 0.114, 0.080]),
    ("wood_floor_old", [0.096, 0.067, 0.049]),
    ("plaster_wall_dirty", [0.288, 0.278, 0.247]),
    ("gun_metal", [0.042, 0.045, 0.051]),
    ("gun_wood", [0.064, 0.026, 0.011]),
    ("polymer_plum", [0.117, 0.023, 0.027]),
    ("robot_shell", [0.699, 0.686, 0.656]),
    ("fabric_canvas", [0.093, 0.089, 0.040]),
    ("asphalt", [0.108, 0.108, 0.099]),
    ("timber", [0.143, 0.062, 0.032]),
    ("boulder", [0.271, 0.258, 0.195]),
];

/// What a material looks like from far off: its tint times its albedo
/// texture's mean (a middling grey for a texture not in the table).
pub fn far_colour(m: &StandardMaterial) -> LinearRgba {
    let tint = m.base_color.to_linear();
    let mean = m.base_color_texture.as_ref().map(|t| {
        let path = t.path().map(|p| p.path().to_string_lossy().into_owned());
        path.and_then(|p| {
            ALBEDO
                .iter()
                .find(|(dir, _)| p.contains(&format!("/{dir}/")))
                .map(|(_, c)| *c)
        })
        .unwrap_or([0.18, 0.18, 0.18])
    });
    match mean {
        Some([r, g, b]) => LinearRgba::rgb(tint.red * r, tint.green * g, tint.blue * b),
        None => LinearRgba::rgb(tint.red, tint.green, tint.blue),
    }
}

/// Builds a stand-in: the pieces' positions and normals, each piece
/// painted its material's far colour. `None` if there's nothing to draw.
#[derive(Default)]
pub struct StandIn {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl StandIn {
    pub fn add(&mut self, mesh: &Mesh, colour: LinearRgba) {
        let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return;
        };
        let n = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(VertexAttributeValues::Float32x3(n)) => n.clone(),
            _ => vec![[0.0, 1.0, 0.0]; p.len()],
        };
        let base = self.pos.len() as u32;
        self.pos.extend_from_slice(p);
        self.nrm.extend(n);
        self.col
            .extend(std::iter::repeat_n(colour.to_f32_array(), p.len()));
        match mesh.indices() {
            Some(i) => self.idx.extend(i.iter().map(|i| base + i as u32)),
            None => self.idx.extend(base..base + p.len() as u32),
        }
    }

    /// A mesh in its own vertex colours (white where it has none).
    pub fn add_coloured(&mut self, mesh: &Mesh) {
        let start = self.col.len();
        self.add(mesh, LinearRgba::WHITE);
        if let Some(VertexAttributeValues::Float32x4(c)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
            if c.len() == self.col.len() - start {
                self.col[start..].copy_from_slice(c);
            }
        }
    }

    pub fn build(self) -> Option<Mesh> {
        if self.idx.is_empty() {
            return None;
        }
        Some(
            Mesh::new(
                bevy::mesh::PrimitiveTopology::TriangleList,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_indices(bevy::mesh::Indices::U32(self.idx)),
        )
    }
}

/// The stand-ins' one material: white, so the vertex colours are the
/// colour; as rough as the walls it stands in for; both sides lit, as
/// the far crowns it also stands in for are.
#[derive(Resource)]
pub struct StandInMaterial(pub Handle<StandardMaterial>);

impl FromWorld for StandInMaterial {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            reflectance: 0.3,
            double_sided: true,
            cull_mode: None,
            ..default()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every texture set the material library uses has a far colour.
    #[test]
    fn every_texture_set_has_a_far_colour() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/render/materials.rs"),
        )
        .unwrap();
        let mut missing = Vec::new();
        for line in src.lines() {
            if let Some(i) = line.find("Some(\"") {
                let rest = &line[i + 6..];
                let dir = &rest[..rest.find('"').unwrap()];
                if !ALBEDO.iter().any(|(d, _)| *d == dir) {
                    missing.push(dir.to_string());
                }
            }
        }
        assert!(missing.is_empty(), "no far colour for {missing:?}");
    }
}
