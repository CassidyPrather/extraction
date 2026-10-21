//! Terrain meshes: the heightmap cut into chunks, with per-vertex surface
//! weights the terrain shader blends between.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use extraction_core::map::RoadKind;

use super::Zone;
use crate::render::terrain_material::{TerrainMaterial, terrain_material};

/// Metres per chunk side.
const CHUNK: usize = 64;

/// Detail levels: vertex spacing in cells, and how far out (from the
/// chunk's centre) each takes over. Coarser chunks hang skirts round their
/// edges so the cracks between neighbours at different levels don't show.
const LODS: [(usize, f32); 3] = [(1, 0.0), (2, 140.0), (4, 260.0)];
/// Until the weather says otherwise, chunks vanish this far out.
const FOG_CULL_DEFAULT: f32 = 760.0;

pub struct TerrainVisualsPlugin;

impl Plugin for TerrainVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_terrain_meshes)
            .add_systems(PostUpdate, fog_cull)
            .add_systems(Update, wet_with_weather);
    }
}

#[derive(Component)]
pub struct TerrainChunk {
    lod: usize,
}

/// The distance band level `lod` is drawn in; the last level ends at the
/// fog.
fn lod_range(lod: usize, fog: f32) -> bevy::camera::visibility::VisibilityRange {
    let start = LODS[lod].1;
    let end = LODS.get(lod + 1).map_or(fog, |l| l.1);
    bevy::camera::visibility::VisibilityRange {
        start_margin: start..start,
        end_margin: end..end,
        use_aabb: true,
    }
}

/// Past what the weather lets you see (well past: the fog's last few
/// percent still show a hill's line), terrain isn't drawn.
fn fog_cull(
    weather: Res<super::sky::Weather>,
    mut chunks: Query<(
        &TerrainChunk,
        &mut bevy::camera::visibility::VisibilityRange,
    )>,
    added: Query<(), Added<TerrainChunk>>,
) {
    if !weather.is_changed() && added.is_empty() {
        return;
    }
    let fog = weather.visibility * 1.3 + CHUNK as f32 * 0.7;
    for (c, mut r) in &mut chunks {
        let want = lod_range(c.lod, fog);
        if r.end_margin != want.end_margin {
            *r = want;
        }
    }
}

/// The ground is as wet as the weather has made it (`weather::Wetness`):
/// soaking over minutes in rain, drying slowly after. (A uniform: no
/// recompile; written when it has moved a little.)
fn wet_with_weather(
    wet: Res<super::weather::Wetness>,
    conditions: Res<super::weather::Conditions>,
    chunks: Query<&MeshMaterial3d<TerrainMaterial>, With<TerrainChunk>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
    mut done: Local<Option<f32>>,
) {
    // A little damp even dry: the Zone's ground never quite dries out.
    let want = 0.12 + 0.75 * wet.level;
    if done.is_some_and(|d| (d - want).abs() < 0.01) && !conditions.is_changed() {
        return;
    }
    let Some(handle) = chunks.iter().next() else {
        return;
    };
    if let Some(mut m) = materials.get_mut(&handle.0) {
        m.extension.params.wetness = want;
        m.extension.params.puddles = super::puddles::fill(wet.level);
        m.extension.params.rain = conditions.rain;
    }
    *done = Some(want);
}

fn spawn_terrain_meshes(
    mut commands: Commands,
    zone: Res<Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let mut material = terrain_material(&asset_server);
    if zone.map.ash > 0.0 {
        material.extension.params.burnt = 1.0;
    }
    let material = materials.add(material);
    let canopy = Canopy::of(&zone);
    let hm = &zone.hm;
    let cells = hm.n - 1;
    let step = (CHUNK as f32 / hm.cell).round() as usize;
    let mut j0 = 0;
    while j0 < cells {
        let mut i0 = 0;
        while i0 < cells {
            let i1 = (i0 + step).min(cells);
            let j1 = (j0 + step).min(cells);
            for (lod, (spacing, _)) in LODS.iter().enumerate() {
                let mesh = chunk_mesh(&zone, &canopy, i0, j0, i1, j1, *spacing);
                commands.spawn((
                    Name::new("Terrain chunk"),
                    TerrainChunk { lod },
                    crate::render::centred(&mut meshes, mesh),
                    MeshMaterial3d(material.clone()),
                    lod_range(lod, FOG_CULL_DEFAULT),
                    // Under this sky the land's own shadows are faint, and
                    // the cascades are drawn per eye: it takes shadows
                    // (trees, buildings) but casts none.
                    bevy::light::NotShadowCaster,
                ));
            }
            i0 = i1;
        }
        j0 += step;
    }
}

/// A valley's old water line, where a reservoir was drained (m): below
/// it the ground is bare silt and mud, with a damp band at the tide mark.
pub fn drowned_line(map: &str) -> Option<f32> {
    match map {
        // The dam's tide mark is at 49.8 m.
        "Sallow Reservoir" => Some(50.5),
        _ => None,
    }
}

/// How drowned ground at height `y` is under the old water line `line`:
/// (silt 0..1, damp 0..1 for the tide mark's band just under it).
pub fn drowned(line: f32, y: f32) -> (f32, f32) {
    let silt = smooth(line + 0.3, line - 0.8, y);
    let damp = smooth(line - 3.0, line - 1.5, y) * smooth(line + 0.2, line - 0.3, y);
    (silt, damp)
}

/// Splat weights packed into vertex colour: r = bare dirt/mud, g = rock,
/// b = road gravel/asphalt, a = forest floor (needles, moss). Grass is
/// whatever is left over. Road kind goes in UV1.x so the shader can pick
/// asphalt vs gravel vs dirt.
/// Where the trees shade the ground: a coarse grid of how much canopy is
/// over each point, from the trees' own positions. The forest floor
/// (needles, litter) lies under the trees and fades out in the gaps
/// between them; a sparse copse has a floor under its trees, not across
/// its whole polygon. (Painted by polygon, small stands read as tan
/// rectangles from the air.)
pub struct Canopy {
    origin: [f32; 2],
    n: usize,
    shade: Vec<f32>,
}

/// The canopy grid's cell (m).
const CANOPY_CELL: f32 = 2.0;
/// How far a tree's litter reaches from its trunk (m).
const CANOPY_REACH: f32 = 7.0;

impl Canopy {
    pub fn of(zone: &Zone) -> Self {
        let trees = extraction_core::forest::scatter_trees(&zone.map, &zone.hm);
        let positions: Vec<[f32; 2]> = trees.iter().map(|t| [t.pos[0], t.pos[2]]).collect();
        let hm = &zone.hm;
        let lo = hm.pos(0, 0);
        let hi = hm.pos(hm.n - 1, hm.n - 1);
        Self::from_trees(lo, hi, &positions)
    }

    pub fn from_trees(lo: [f32; 2], hi: [f32; 2], trees: &[[f32; 2]]) -> Self {
        let n = (((hi[0] - lo[0]).max(hi[1] - lo[1])) / CANOPY_CELL).ceil() as usize + 1;
        let mut shade = vec![0.0f32; n * n];
        let r = (CANOPY_REACH / CANOPY_CELL).ceil() as i64;
        for t in trees {
            let ci = ((t[0] - lo[0]) / CANOPY_CELL).round() as i64;
            let cj = ((t[1] - lo[1]) / CANOPY_CELL).round() as i64;
            for j in (cj - r).max(0)..=(cj + r).min(n as i64 - 1) {
                for i in (ci - r).max(0)..=(ci + r).min(n as i64 - 1) {
                    let x = lo[0] + i as f32 * CANOPY_CELL;
                    let z = lo[1] + j as f32 * CANOPY_CELL;
                    let d = ((x - t[0]).powi(2) + (z - t[1]).powi(2)).sqrt();
                    let k = (1.0 - d / CANOPY_REACH).max(0.0);
                    shade[j as usize * n + i as usize] += k * k;
                }
            }
        }
        Self {
            origin: lo,
            n,
            shade,
        }
    }

    /// How much of a forest floor `p` has, 0..1: full under the trees and
    /// in a closed stand, none in a gap a few metres from any trunk.
    pub fn floor(&self, p: [f32; 2]) -> f32 {
        let i = ((p[0] - self.origin[0]) / CANOPY_CELL).round();
        let j = ((p[1] - self.origin[1]) / CANOPY_CELL).round();
        if i < 0.0 || j < 0.0 || i as usize >= self.n || j as usize >= self.n {
            return 0.0;
        }
        smooth(0.1, 0.7, self.shade[j as usize * self.n + i as usize])
    }
}

fn chunk_mesh(
    zone: &Zone,
    canopy: &Canopy,
    i0: usize,
    j0: usize,
    i1: usize,
    j1: usize,
    spacing: usize,
) -> Mesh {
    let hm = &zone.hm;
    // Sample rows and columns every `spacing` cells, always ending on the
    // chunk's edge.
    let ticks = |a: usize, b: usize| {
        let mut v: Vec<usize> = (a..b).step_by(spacing).collect();
        v.push(b);
        v
    };
    let (is, js) = (ticks(i0, i1), ticks(j0, j1));
    let (w, h) = (is.len(), js.len());
    let line = drowned_line(&zone.map.name);
    let mut positions = Vec::with_capacity(w * h);
    let mut normals = Vec::with_capacity(w * h);
    let mut uvs = Vec::with_capacity(w * h);
    let mut uv1 = Vec::with_capacity(w * h);
    let mut colors = Vec::with_capacity(w * h);
    for &j in &js {
        for &i in &is {
            let p = hm.pos(i, j);
            let y = hm.heights[hm.idx(i, j)];
            positions.push([p[0], y, p[1]]);
            normals.push(hm.normal(p));
            // World-space UVs in metres; the shader scales per layer.
            uvs.push([p[0], p[1]]);
            // Coarse vertices take the strongest road paint around them,
            // so a road doesn't break up into patches far off.
            let mut road = hm.roads[hm.idx(i, j)];
            if spacing > 1 {
                let r = spacing / 2;
                for jj in j.saturating_sub(r)..=(j + r).min(hm.n - 1) {
                    for ii in i.saturating_sub(r)..=(i + r).min(hm.n - 1) {
                        let q = hm.roads[hm.idx(ii, jj)];
                        if q.weight > road.weight {
                            road = q;
                        }
                    }
                }
            }
            let road_w = road.weight as f32 / 255.0;
            let kind = match road.kind {
                Some(RoadKind::Asphalt) => 1.0,
                Some(RoadKind::Gravel) => 2.0,
                Some(RoadKind::Rail) => 3.0,
                Some(RoadKind::Dirt) => 4.0,
                Some(RoadKind::Path) => 5.0,
                None => 0.0,
            };
            let (silt, damp) = line.map_or((0.0, 0.0), |l| drowned(l, y));
            uv1.push([kind, hm.wet[hm.idx(i, j)].max(damp)]);
            let slope = hm.slope_deg(p);
            let rock = smooth(28.0, 42.0, slope);
            let wet = hm.wet[hm.idx(i, j)];
            let forest = zone.forest_cover(p[0], p[1]) * canopy.floor(p);
            // Hollows are muddy, and where puddles stand (`puddles`).
            let hollow = super::puddles::hollow(hm, p);
            colors.push([
                wet.max(0.15 * rock).max(hollow).max(silt),
                rock,
                road_w,
                forest * (1.0 - silt),
            ]);
        }
    }
    let mut indices = Vec::with_capacity((w - 1) * (h - 1) * 6);
    for j in 0..h - 1 {
        for i in 0..w - 1 {
            let a = (j * w + i) as u32;
            let b = a + 1;
            let c = a + w as u32;
            let d = c + 1;
            // Cells opened over underground stairwells stay open.
            if holed(hm, is[i], is[i + 1], js[j], js[j + 1]) {
                continue;
            }
            // Alternate the diagonal to avoid a visible grain.
            if (i + j) % 2 == 0 {
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            } else {
                indices.extend_from_slice(&[a, c, d, a, d, b]);
            }
        }
    }
    // Skirts: the edge ring again, hung below, faced both ways. A coarse
    // chunk's edge can sit a little off its finer neighbour's; the skirt
    // fills the gap with ground.
    if spacing > 1 {
        let drop = 1.5 * spacing as f32;
        let ring: Vec<usize> = (0..w)
            .chain((1..h).map(|j| j * w + w - 1))
            .chain((0..w - 1).rev().map(|i| (h - 1) * w + i))
            .chain((1..h - 1).rev().map(|j| j * w))
            .collect();
        let first = positions.len() as u32;
        for &k in &ring {
            let [x, y, z] = positions[k];
            positions.push([x, y - drop, z]);
            normals.push(normals[k]);
            uvs.push(uvs[k]);
            uv1.push(uv1[k]);
            colors.push(colors[k]);
        }
        let n = ring.len();
        for s in 0..n {
            let (a, b) = (ring[s] as u32, ring[(s + 1) % n] as u32);
            let (c, d) = (first + s as u32, first + ((s + 1) % n) as u32);
            indices.extend_from_slice(&[a, c, b, b, c, d, a, b, c, b, d, c]);
        }
    }
    // The UVs are world x and z, so the tangent is +X laid onto the ground
    // everywhere. Generated per chunk instead, the tangents at a chunk's
    // edge only saw its own side, and every chunk border showed as a ruled
    // line in the ground's lighting.
    let tangents: Vec<[f32; 4]> = normals.iter().map(|n| uv_tangent(Vec3::from(*n))).collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uv1)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
    mesh
}

/// Whether any heightmap cell in `i0..i1` by `j0..j1` is a hole.
fn holed(
    hm: &extraction_core::terrain::Heightmap,
    i0: usize,
    i1: usize,
    j0: usize,
    j1: usize,
) -> bool {
    (j0..j1).any(|j| (i0..i1).any(|i| hm.holes[hm.idx(i, j)]))
}

/// The tangent for world-space UVs (u along +X, v along +Z) on ground with
/// normal `n`: the way the surface goes as u grows, `(1, dh/dx, 0)`. (Not
/// +X pushed flat onto the surface: on a slope facing between the axes
/// that leans toward Z, by 30 degrees on the steepest banks.) The
/// handedness is the same everywhere the ground faces up, and is what
/// mikktspace gives these UVs (see the test).
fn uv_tangent(n: Vec3) -> [f32; 4] {
    let t = if n.y > 1e-3 {
        Vec3::new(n.y, -n.x, 0.0).normalize()
    } else {
        (Vec3::X - n * n.x).normalize_or(Vec3::X)
    };
    [t.x, t.y, t.z, 1.0]
}

#[cfg(test)]
mod tangent_tests {
    use super::*;

    /// Away from its edges a chunk's generated tangents are right; the
    /// analytic ones match them there (and so are right at the edges too).
    #[test]
    fn analytic_tangents_match_generated_ones_inside_a_chunk() {
        let zone = Zone::load("marrow_creek");
        let mut mesh = chunk_mesh(&zone, &Canopy::of(&zone), 64, 64, 128, 128, 1);
        let ours: Vec<[f32; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_TANGENT) {
            Some(bevy::mesh::VertexAttributeValues::Float32x4(v)) => v.clone(),
            _ => panic!("tangents"),
        };
        mesh.generate_tangents().unwrap();
        let theirs = match mesh.attribute(Mesh::ATTRIBUTE_TANGENT) {
            Some(bevy::mesh::VertexAttributeValues::Float32x4(v)) => v.clone(),
            _ => panic!("tangents"),
        };
        let w = 65;
        // (A vertex in a hole over an underground stair is in no
        // triangle: mikktspace leaves it alone.)
        let mut used = vec![false; ours.len()];
        for k in mesh.indices().expect("indexed").iter() {
            used[k] = true;
        }
        let (mut worst, mut sum, mut count) = (1.0f32, 0.0f32, 0.0f32);
        for j in 2..w - 2 {
            for i in 2..w - 2 {
                let k = j * w + i;
                if !used[k] {
                    continue;
                }
                let (a, b) = (Vec4::from(ours[k]), Vec4::from(theirs[k]));
                assert_eq!(a.w, b.w, "handedness at {i},{j}");
                let cos = a.truncate().dot(b.truncate().normalize());
                worst = worst.min(cos);
                sum += cos;
                count += 1.0;
            }
        }
        // (mikktspace averages faces; on steep cells it leans a few degrees.)
        assert!(worst > 0.95, "directions agree: worst cos {worst}");
        assert!(sum / count > 0.995, "on average: cos {}", sum / count);
    }
}

#[cfg(test)]
mod drowned_tests {
    use super::*;

    #[test]
    fn the_forest_floor_lies_under_the_trees_not_across_the_gaps() {
        // A sparse copse: three trees 20 m apart.
        let trees = [[0.0, 0.0], [20.0, 0.0], [0.0, 20.0]];
        let c = Canopy::from_trees([-50.0, -50.0], [50.0, 50.0], &trees);
        // A lone tree's litter is light; a stand's floor is full.
        assert!(c.floor([0.0, 0.0]) > 0.6, "under a tree");
        // Two trees 4 m apart share a floor between them.
        let pair = Canopy::from_trees([-50.0, -50.0], [50.0, 50.0], &[[0.0, 0.0], [4.0, 0.0]]);
        assert!(pair.floor([2.0, 0.0]) > 0.8, "between a pair");
        assert_eq!(c.floor([10.0, 12.0]), 0.0, "the gap between them");
        assert_eq!(c.floor([200.0, 0.0]), 0.0, "off the grid");
        // A closed stand, a tree every 4 m: floor all through.
        let mut stand = Vec::new();
        for i in 0..10 {
            for j in 0..10 {
                stand.push([i as f32 * 4.0, j as f32 * 4.0]);
            }
        }
        let c = Canopy::from_trees([-50.0, -50.0], [50.0, 50.0], &stand);
        assert!(c.floor([18.0, 18.0]) > 0.99 && c.floor([2.0, 2.0]) > 0.99);
    }

    #[test]
    fn the_reservoir_bed_is_silt_and_the_tide_mark_damp() {
        let line = drowned_line("Sallow Reservoir").unwrap();
        assert!(drowned_line("Marrow Creek").is_none());
        assert_eq!(drowned(line, 33.0).0, 1.0, "the bed");
        assert_eq!(drowned(line, 52.0), (0.0, 0.0), "the crest");
        let (_, band) = drowned(line, line - 1.2);
        assert!(band > 0.9, "the tide mark: {band}");
        assert_eq!(drowned(line, 40.0).1, 0.0, "the bed isn't a band");
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    extraction_core::geom::smoothstep(e0, e1, x)
}
