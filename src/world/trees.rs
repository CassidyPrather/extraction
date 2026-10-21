//! Procedural trees and their scattering through the map's forests.
//!
//! Trees are generated meshes (a few variants per species), instanced many
//! times with random yaw, scale and lean. Each gets a static trunk collider.

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use extraction_core::map::TreeSpecies;
use extraction_core::noise::Rng;

use super::{Layer, Zone};

const VARIANTS: usize = extraction_core::forest::TREE_VARIANTS;

pub struct TreesPlugin;

impl Plugin for TreesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MaterialPlugin::<FarCrownMaterial>::default(),
            MaterialPlugin::<TreeSwayMaterial>::default(),
            super::impostors::ImpostorPlugin,
        ))
        .add_systems(Update, blow_trees)
        .add_systems(
            PostUpdate,
            (spawn_forests, stream_near_detail, fog_cull).chain(),
        )
        .add_systems(
            PostUpdate,
            cut_near_detail
                .after(TransformSystems::Propagate)
                .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
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
/// The scattering itself lives in `extraction_core::forest`, shared with
/// the Unity exporter.
pub fn scatter_trees(zone: &Zone) -> Vec<TreeInstance> {
    extraction_core::forest::scatter_trees(&zone.map, &zone.hm)
        .into_iter()
        .map(|t| TreeInstance {
            species: t.species,
            variant: t.variant,
            pos: Vec3::from_array(t.pos),
            yaw: t.yaw,
            scale: t.scale,
        })
        .collect()
}

/// A tree in the world: logic side (collider). Meshes attach client-side.
#[derive(Component, Clone, Copy, Debug)]
pub struct Tree {
    pub species: TreeSpecies,
    pub variant: usize,
}

/// The trunks of one chunk of forest, as one compound collider (thousands
/// of separate static bodies cost the physics step dearly).
#[derive(Component, Debug)]
pub struct TreeTrunks;

/// Logic: every tree gets a trunk (bullets stop, robots can't see through,
/// nobody walks through). The trees themselves are plain entities, anchors
/// for their meshes; the trunks are gathered into one static compound
/// collider per chunk.
pub fn spawn_tree_colliders(mut commands: Commands, zone: Res<Zone>) {
    let started = std::time::Instant::now();
    let trees = scatter_trees(&zone);
    let mut chunks: std::collections::HashMap<(i32, i32), Vec<(Position, Rotation, Collider)>> =
        std::collections::HashMap::new();
    for t in &trees {
        commands.spawn((
            Tree {
                species: t.species,
                variant: t.variant,
            },
            Transform::from_translation(t.pos)
                .with_rotation(Quat::from_rotation_y(t.yaw))
                .with_scale(Vec3::splat(t.scale)),
            Visibility::default(),
        ));
        let (cx, cz) = chunk_of(t.pos);
        let origin = Vec3::new(cx as f32 * CHUNK, 0.0, cz as f32 * CHUNK);
        let r = trunk_radius(t.species) * t.scale;
        chunks.entry((cx, cz)).or_default().push((
            Position(t.pos - origin),
            Rotation::default(),
            Collider::capsule_endpoints(
                r,
                Vec3::new(0.0, 0.5 * t.scale, 0.0),
                Vec3::new(0.0, 4.0 * t.scale, 0.0),
            ),
        ));
    }
    let n = chunks.len();
    for ((cx, cz), shapes) in chunks {
        commands.spawn((
            Name::new("Tree trunks"),
            TreeTrunks,
            Transform::from_xyz(cx as f32 * CHUNK, 0.0, cz as f32 * CHUNK),
            RigidBody::Static,
            Collider::compound(shapes),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
    info!("{n} trunk colliders");
    info!(
        "{} trees scattered in {:.0} ms",
        trees.len(),
        started.elapsed().as_secs_f32() * 1000.0
    );
}

/// Client: how the forest is drawn, built for thousands of trees at VR
/// frame rates.
///
/// - Trunks are merged into one mesh per [`CHUNK`]-metre square of map
///   (per bark material), full detail near and a coarse LOD past
///   [`FAR_LOD`], and nothing past the fog: a few hundred draws for the
///   whole forest.
/// - Foliage near (within `impostors::NEAR`) is each tree's own alpha-
///   tested cards (and the spruces' dark core), streamed in and out round
///   the camera; beyond it, impostors: a picture of the same cards taken
///   at start, on crossed quads merged per chunk. The two swap per tree,
///   at once, by the same distance (no dithered cross-fade).
fn spawn_forests(
    mut commands: Commands,
    trees: Query<(Entity, &Tree, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fade_materials: ResMut<Assets<FarCrownMaterial>>,
    mut sway_materials: ResMut<Assets<TreeSwayMaterial>>,
    mut impostor_materials: ResMut<Assets<super::impostors::ImpostorMaterial>>,
    stand_in: Res<crate::render::lod::StandInMaterial>,
    mut images: ResMut<Assets<Image>>,
    mut library: ResMut<crate::render::materials::MaterialLibrary>,
    server: Res<AssetServer>,
    zone: Res<Zone>,
    mut done: Local<bool>,
) {
    if *done || trees.is_empty() {
        return;
    }
    *done = true;
    let assets = {
        use crate::render::materials::Mat;
        let foliage_dir = crate::asset_root_path().join("textures/foliage");
        let textured = foliage_dir.join("spruce_branch/albedo.ktx2").exists()
            && foliage_dir.join("birch_leaves/albedo.ktx2").exists();
        let flat_bark = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            reflectance: 0.2,
            ..default()
        });
        let core = materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.6, 0.55),
            perceptual_roughness: 0.9,
            reflectance: 0.15,
            // One-sided: standing in a spruce (only the trunk stops you),
            // its core seen from inside was most of the view, black.
            ..default()
        });
        let card = |name: &str, tint: Color, materials: &mut Assets<StandardMaterial>| {
            let tex: Handle<Image> = server
                .load_builder()
                .with_settings(|s: &mut bevy::image::ImageLoaderSettings| {
                    s.is_srgb = true;
                })
                .load(format!("textures/foliage/{name}/albedo.ktx2"));
            materials.add(StandardMaterial {
                base_color: tint,
                base_color_texture: Some(tex),
                alpha_mode: foliage_alpha(0.45),
                perceptual_roughness: 0.8,
                reflectance: 0.2,
                double_sided: true,
                cull_mode: None,
                diffuse_transmission: 0.25,
                ..default()
            })
        };
        let (needles, leaves) = if textured {
            (
                Some(card(
                    "spruce_branch",
                    Color::srgb(0.85, 0.88, 0.82),
                    &mut materials,
                )),
                Some(card(
                    "birch_leaves",
                    Color::srgb(0.62, 0.66, 0.55),
                    &mut materials,
                )),
            )
        } else {
            (None, None)
        };
        let bark = |s: TreeSpecies,
                    library: &mut crate::render::materials::MaterialLibrary,
                    materials: &mut Assets<StandardMaterial>| {
            if !textured {
                return flat_bark.clone();
            }
            library.get(
                match s {
                    TreeSpecies::Spruce | TreeSpecies::Snag => Mat::BarkSpruce,
                    TreeSpecies::Pine => Mat::BarkPine,
                    TreeSpecies::Birch => Mat::BarkBirch,
                },
                &server,
                materials,
            )
        };
        let mut handles = std::collections::HashMap::new();
        for s in [
            TreeSpecies::Spruce,
            TreeSpecies::Pine,
            TreeSpecies::Birch,
            TreeSpecies::Snag,
        ] {
            let b = bark(s, &mut library, &mut materials);
            for v in 0..VARIANTS {
                let seed = 1000 + v as u64 * 17 + s as u64 * 101;
                let burnt = zone.map.ash > 0.0;
                let m = build_tree(s, seed, textured, false, burnt);
                let far = build_tree(s, seed, textured, true, burnt);
                let card_mat = match s {
                    TreeSpecies::Birch => leaves.clone(),
                    _ => needles.clone(),
                };
                let crown = m.crown.as_ref().map(|c| meshes.add(c.clone()));
                let bounds = m.cards.as_ref().and_then(mesh_bounds);
                // The live near cards move in the wind; the impostor's
                // camera photographs the still ones.
                let live = card_mat.as_ref().and_then(|h| materials.get(h)).map(|m| {
                    sway_materials.add(TreeSwayMaterial {
                        base: m.clone(),
                        extension: TreeSway::default(),
                    })
                });
                let cards = m.cards.zip(card_mat).map(|(c, mat)| (meshes.add(c), mat));
                // Far foliage: this variant's cards (and a spruce's core)
                // photographed for its impostor.
                let impostor = match (&cards, bounds) {
                    (Some((mesh, mat)), Some((min, max))) => {
                        let card_mat = materials.get(mat).cloned().unwrap_or_default();
                        let core_mat = materials.get(&core).cloned().unwrap_or_default();
                        let core_part = crown
                            .clone()
                            .filter(|_| s == TreeSpecies::Spruce)
                            .map(|c| (c, &core_mat, CORE_SCALE));
                        let imp = super::impostors::capture(
                            &mut commands,
                            &mut images,
                            &mut materials,
                            handles.len(),
                            (mesh.clone(), &card_mat),
                            core_part,
                            min,
                            max,
                        );
                        let tint = card_mat.base_color;
                        let mat = impostor_materials.add(super::impostors::material(
                            imp.image.clone(),
                            tint_to_white(tint),
                        ));
                        Some((imp, mat))
                    }
                    _ => None,
                };
                handles.insert(
                    (s, v),
                    VariantAssets {
                        crown,
                        trunk_cpu: m.trunk,
                        crown_cpu: m.crown,
                        trunk_far: far.trunk,
                        // Without an impostor (no foliage textures) the far
                        // crown stands out to the fog: a pine's or birch's
                        // is one coarse blob, not a dozen (a spruce's cone
                        // is already cheap).
                        crown_far: match far.crown {
                            Some(c) if impostor.is_none() && triangles(&c) > 200 => blob_like(&c),
                            c => c,
                        },
                        bark: b.clone(),
                        cards,
                        live,
                        impostor,
                    },
                );
            }
        }
        TreeAssets {
            foliage: fade_materials.add(FarCrownMaterial {
                base: StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.85,
                    reflectance: 0.25,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                },
                extension: NearFade {
                    fade: Vec4::new(62.0, 74.0, 0.0, 0.0),
                },
            }),
            core,
            handles,
        }
    };
    // Group trees into chunks; merge each chunk's trunks (per bark) and crowns.
    let mut chunks: std::collections::HashMap<(i32, i32), Vec<(Entity, Tree, Transform)>> =
        std::collections::HashMap::new();
    for (e, tree, t) in &trees {
        let key = chunk_of(t.translation);
        chunks.entry(key).or_default().push((e, *tree, *t));
    }
    let mut grid = ForestGrid::default();
    let mut chunk_count = 0;
    let mut impostors: std::collections::HashMap<
        ((i32, i32), (TreeSpecies, usize)),
        super::impostors::Builder,
    > = Default::default();
    let fetched = assets.handles.values().any(|v| v.impostor.is_some());
    let far_lod = if fetched { FAR_LOD } else { FALLBACK_FAR_LOD };
    info!(
        "forest: foliage {}",
        if fetched {
            "fetched (cards, impostors)"
        } else {
            "FALLBACK: no foliage textures, untextured crowns (run scripts/fetch_assets.py)"
        }
    );
    for (key, members) in &chunks {
        for (e, tree, t) in members {
            grid.cells
                .entry(*key)
                .or_default()
                .push((*e, *tree, t.translation));
        }
        let parent = commands
            .spawn((
                Name::new("Forest chunk"),
                Transform::IDENTITY,
                Visibility::default(),
            ))
            .id();
        // Full detail near, the coarse LOD past `FAR_LOD` (cross-faded),
        // and nothing past the fog (see `fog_cull`).
        for far in [false, true] {
            let mut trunks: Vec<(Handle<StandardMaterial>, Mesh)> = Vec::new();
            let mut crowns: Option<Mesh> = None;
            for (_, tree, t) in members {
                let v = &assets.handles[&(tree.species, tree.variant)];
                let (trunk, crown) = if far {
                    (&v.trunk_far, &v.crown_far)
                } else {
                    (&v.trunk_cpu, &v.crown_cpu)
                };
                let trunk = trunk.clone().transformed_by(*t);
                match trunks.iter_mut().find(|(m, _)| *m == v.bark) {
                    Some((_, mesh)) => {
                        if let Err(e) = mesh.merge(&trunk) {
                            warn!("forest: trunk merge failed: {e:?}");
                        }
                    }
                    None => trunks.push((v.bark.clone(), trunk)),
                }
                // (The near crowns' material dissolves within 74 m, where the
                // cards would be: without cards they were drawn unseen.)
                if let Some(c) = crown
                    .as_ref()
                    .filter(|_| v.impostor.is_none() && (far || fetched))
                {
                    let c = c.clone().transformed_by(*t);
                    match &mut crowns {
                        Some(m) => {
                            let _ = m.merge(&c);
                        }
                        None => crowns = Some(c),
                    }
                }
            }
            let range = lod_range(far, FOG_CULL_DEFAULT, far_lod);
            if far {
                // Past `FAR_LOD` bark is a few pixels a metre: trunks and
                // crowns go in one vertex-coloured mesh (see
                // `render::lod`), a draw a chunk instead of four or five.
                let mut stand = crate::render::lod::StandIn::default();
                for (bark, mesh) in &trunks {
                    if let Some(m) = materials.get(bark) {
                        stand.add(mesh, crate::render::lod::far_colour(m));
                    }
                }
                if let Some(c) = &crowns {
                    stand.add_coloured(c);
                }
                if let Some(mesh) = stand.build() {
                    commands.spawn((
                        crate::render::centred(&mut meshes, mesh),
                        MeshMaterial3d(stand_in.0.clone()),
                        bevy::light::NotShadowCaster,
                        range.clone(),
                        FarForest,
                        ChildOf(parent),
                    ));
                }
                continue;
            }
            for (bark, mesh) in trunks {
                let mut ec = commands.spawn((
                    crate::render::centred(&mut meshes, mesh),
                    MeshMaterial3d(bark),
                    range.clone(),
                    ChildOf(parent),
                ));
                ec.insert(TRUNK_SHADOWS);
            }
            if let Some(c) = crowns {
                commands.spawn((
                    crate::render::centred(&mut meshes, c),
                    MeshMaterial3d(assets.foliage.clone()),
                    bevy::light::NotShadowCaster,
                    range.clone(),
                    ChildOf(parent),
                ));
            }
        }
        // Far foliage: impostors, one mesh per variant per block of
        // chunks (below).
        let block = (
            key.0.div_euclid(IMPOSTOR_BLOCK),
            key.1.div_euclid(IMPOSTOR_BLOCK),
        );
        for (_, tree, t) in members {
            let v = &assets.handles[&(tree.species, tree.variant)];
            if let Some((imp, _)) = &v.impostor {
                super::impostors::quads(
                    imp,
                    t,
                    impostors
                        .entry((block, (tree.species, tree.variant)))
                        .or_default(),
                );
            }
        }
        chunk_count += 1;
    }
    // Impostors are only ever far, and their near cut is the shader's, per
    // tree: merged over 3x3 chunks they are a ninth of the draws (they
    // were two thirds of the forest's).
    for ((_, key), b) in impostors {
        if b.is_empty() {
            continue;
        }
        let (_, mat) = assets.handles[&key].impostor.as_ref().unwrap();
        commands.spawn((
            Name::new("Forest impostors"),
            crate::render::centred(&mut meshes, b.build()),
            MeshMaterial3d(mat.clone()),
            bevy::light::NotShadowCaster,
            impostor_range(FOG_CULL_DEFAULT),
            ImpostorForest,
        ));
    }
    info!(
        "forest: {} trees in {chunk_count} chunks",
        trees.iter().count()
    );
    commands.insert_resource(grid);
    commands.insert_resource(ForestAssets(assets));
    commands.insert_resource(ForestBuilt);
}

/// Chunk size for merged forest meshes, metres.
pub const CHUNK: f32 = 48.0;
/// Impostors merge over this many chunks a side.
const IMPOSTOR_BLOCK: i32 = 3;
/// A chunk (by its centre) switches to the coarse LOD this far away.
const FAR_LOD: f32 = 130.0;
/// Without the foliage textures (`scripts/fetch_assets.py` not run) there
/// are no cards and no impostors, and the near crowns (whose material
/// dissolves within 74 m, where cards would be) were over a million
/// triangles in a forested view, mostly unseen. That fallback draws no
/// near crowns and switches to the far LOD (a coarse blob a crown) here,
/// where the near crowns would have finished fading in.
const FALLBACK_FAR_LOD: f32 = 75.0;
/// Until the weather says otherwise, chunks vanish this far out.
const FOG_CULL_DEFAULT: f32 = 560.0;

/// A chunk's far-LOD mesh: hidden beyond the fog.
#[derive(Component)]
pub struct FarForest;

/// Near or far LOD's distance band. Chunk meshes sit at the origin with
/// world-space vertices, so distance is measured to their bounds' centre.
fn lod_range(far: bool, fog: f32, at: f32) -> bevy::camera::visibility::VisibilityRange {
    let switch = (at - 10.0)..(at + 10.0);
    if far {
        bevy::camera::visibility::VisibilityRange {
            start_margin: switch,
            end_margin: fog..(fog + 20.0),
            use_aabb: true,
        }
    } else {
        bevy::camera::visibility::VisibilityRange {
            start_margin: 0.0..0.0,
            end_margin: switch,
            use_aabb: true,
        }
    }
}

/// Far trees go with the fog: well beyond what the weather lets you see
/// (the fog's last few percent still show a treeline; plus half a chunk,
/// as range is to a chunk's centre) nothing is drawn.
fn fog_cull(
    weather: Res<super::sky::Weather>,
    mut far: Query<
        (
            &mut bevy::camera::visibility::VisibilityRange,
            Has<ImpostorForest>,
        ),
        Or<(With<FarForest>, With<ImpostorForest>)>,
    >,
    added: Query<(), Or<(Added<FarForest>, Added<ImpostorForest>)>>,
) {
    if !weather.is_changed() && added.is_empty() {
        return;
    }
    let fog = weather.visibility * 1.3 + CHUNK * 0.7;
    for (mut r, impostor) in &mut far {
        // Only the far end moves with the weather (the near switch is the
        // chunk's own: see `FALLBACK_FAR_LOD`).
        let want = if impostor {
            impostor_range(fog)
        } else {
            lod_range(true, fog, FAR_LOD)
        };
        if r.end_margin != want.end_margin {
            r.end_margin = want.end_margin;
        }
    }
}

/// A tree's cards (and core) show within the impostors' cut and hide
/// beyond it, by the same horizontal distance from the same camera the
/// impostor shader measures, every frame: the tree swaps all at once.
fn cut_near_detail(
    cams: Query<(&GlobalTransform, Option<&bevy::camera::RenderTarget>), crate::render::EyeCamera>,
    trees: Query<(&GlobalTransform, &NearDetail)>,
    mut vis: Query<&mut Visibility>,
) {
    let eyes: Vec<Vec2> = crate::render::eyes(&cams)
        .map(|g| g.translation().xz())
        .collect();
    if eyes.is_empty() {
        return;
    }
    let cut = super::impostors::NEAR;
    for (gt, detail) in &trees {
        let p = gt.translation().xz();
        let d = eyes
            .iter()
            .map(|e| e.distance(p))
            .fold(f32::INFINITY, f32::min);
        let want = if d < cut {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        for kid in &detail.0 {
            if let Ok(mut v) = vis.get_mut(*kid) {
                if *v != want {
                    *v = want;
                }
            }
        }
    }
}

/// A chunk's impostors: drawn out to the fog (the near cut is the
/// shader's, per tree).
#[derive(Component)]
pub struct ImpostorForest;

fn impostor_range(fog: f32) -> bevy::camera::visibility::VisibilityRange {
    // To a block's centre: the rest of a block's half-width further.
    let fog = fog + (IMPOSTOR_BLOCK - 1) as f32 * CHUNK * 0.7;
    bevy::camera::visibility::VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: fog..(fog + 20.0),
        use_aabb: true,
    }
}

/// Near cards (and cores) cast the sun's shadow only this close: past it
/// the trunks' shadows carry the wood, as they do past the impostors' cut
/// (see `render::budget::ShadowReach`).
const CARD_SHADOWS: crate::render::budget::ShadowReach = crate::render::budget::ShadowReach(30.0);

/// A chunk's near trunks cast the sun's shadow only while its centre is
/// this close (the cascades reach 90 m; past about 60 m in drizzle a
/// trunk's shadow is a faint ruled line).
const TRUNK_SHADOWS: crate::render::budget::ShadowReach = crate::render::budget::ShadowReach(60.0);

/// The spruce's dark core, scaled inside its cards.
const CORE_SCALE: Vec3 = Vec3::new(0.62, 0.96, 0.62);

/// The card material's tint is baked into the picture: the impostor's own
/// is white.
fn tint_to_white(_tint: Color) -> Color {
    Color::WHITE
}

/// Bounds of a mesh's positions (local).
fn triangles(m: &Mesh) -> usize {
    m.indices().map_or(0, |i| i.len() / 3)
}

/// One coarse blob filling a crown's bounds, in its mean vertex colour.
fn blob_like(crown: &Mesh) -> Option<Mesh> {
    let (lo, hi) = mesh_bounds(crown)?;
    let colour = match crown.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(bevy::mesh::VertexAttributeValues::Float32x4(c)) if !c.is_empty() => {
            let sum = c.iter().fold([0.0f32; 4], |a, b| {
                [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
            });
            sum.map(|x| x / c.len() as f32)
        }
        _ => [0.1, 0.12, 0.06, 1.0],
    };
    let mut b = MeshBuilder {
        coarse: true,
        ..default()
    };
    b.blob((lo + hi) * 0.5, (hi - lo) * 0.45, 7, |_, _| colour);
    Some(b.build())
}

fn mesh_bounds(m: &Mesh) -> Option<(Vec3, Vec3)> {
    let pos = m.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?;
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for p in pos {
        lo = lo.min(Vec3::from_array(*p));
        hi = hi.max(Vec3::from_array(*p));
    }
    lo.is_finite().then_some((lo, hi))
}
/// Near detail appears within this distance of the camera...
pub const NEAR_SPAWN: f32 = 80.0;
/// ...and is dropped beyond this one.
pub const NEAR_DROP: f32 = 95.0;

fn chunk_of(p: Vec3) -> (i32, i32) {
    ((p.x / CHUNK).floor() as i32, (p.z / CHUNK).floor() as i32)
}

/// Trees by chunk, for streaming near detail.
#[derive(Resource, Default)]
struct ForestGrid {
    cells: std::collections::HashMap<(i32, i32), Vec<(Entity, Tree, Vec3)>>,
}

#[derive(Resource)]
struct ForestAssets(TreeAssets);

/// The forest's meshes are built (`spawn_forests` has run).
#[derive(Resource)]
pub struct ForestBuilt;

/// Whether the forest is done arriving: built, and every impostor's
/// picture taken (each a capture rig until its readback comes in).
#[derive(bevy::ecs::system::SystemParam)]
pub struct ForestSettled<'w, 's> {
    built: Option<Res<'w, ForestBuilt>>,
    captures: Query<'w, 's, (), With<super::impostors::Capture>>,
}

impl ForestSettled<'_, '_> {
    pub fn settled(&self) -> bool {
        self.built.is_some() && self.captures.is_empty()
    }
}

/// On a tree whose near detail is spawned: the detail entities.
#[derive(Component)]
struct NearDetail(Vec<Entity>);

/// Spawns and drops foliage cards round the camera.
fn stream_near_detail(
    mut commands: Commands,
    grid: Option<Res<ForestGrid>>,
    assets: Option<Res<ForestAssets>>,
    cams: Query<(&GlobalTransform, Option<&bevy::camera::RenderTarget>), crate::render::EyeCamera>,
    detailed: Query<(Entity, &NearDetail, &GlobalTransform)>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    let (Some(grid), Some(assets)) = (grid, assets) else {
        return;
    };
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.25;
    let eyes: Vec<Vec3> = crate::render::eyes(&cams)
        .map(|g| g.translation())
        .collect();
    if eyes.is_empty() {
        return;
    }
    // Horizontal, as the impostors' cut is.
    let near = |p: Vec3, r: f32| eyes.iter().any(|e| e.xz().distance(p.xz()) < r);
    // Drop what's now far.
    for (e, detail, gt) in &detailed {
        if !near(gt.translation(), NEAR_DROP) {
            for c in &detail.0 {
                commands.entity(*c).despawn();
            }
            commands.entity(e).remove::<NearDetail>();
        }
    }
    // Add what's now near.
    let assets = &assets.0;
    let reach = (NEAR_SPAWN / CHUNK).ceil() as i32 + 1;
    // Shown or hidden at the impostors' cut by `cut_near_detail`.
    let fade = Visibility::Hidden;
    let mut seen = std::collections::HashSet::new();
    for eye in &eyes {
        let (cx, cz) = chunk_of(*eye);
        for dz in -reach..=reach {
            for dx in -reach..=reach {
                let Some(cell) = grid.cells.get(&(cx + dx, cz + dz)) else {
                    continue;
                };
                for (e, tree, p) in cell {
                    if !seen.insert(*e) || detailed.contains(*e) || !near(*p, NEAR_SPAWN) {
                        continue;
                    }
                    let v = &assets.handles[&(tree.species, tree.variant)];
                    let mut kids = Vec::new();
                    if let Some((cards, card_mat)) = &v.cards {
                        let mut card = commands.spawn((
                            Mesh3d(cards.clone()),
                            fade.clone(),
                            CARD_SHADOWS,
                            ChildOf(*e),
                        ));
                        match &v.live {
                            Some(live) => card.insert(MeshMaterial3d(live.clone())),
                            None => card.insert(MeshMaterial3d(card_mat.clone())),
                        };
                        kids.push(card.id());
                        // Spruces need a dark core to read as dense.
                        if tree.species == TreeSpecies::Spruce {
                            if let Some(crown) = &v.crown {
                                kids.push(
                                    commands
                                        .spawn((
                                            Mesh3d(crown.clone()),
                                            MeshMaterial3d(assets.core.clone()),
                                            Transform::from_scale(CORE_SCALE),
                                            fade.clone(),
                                            CARD_SHADOWS,
                                            ChildOf(*e),
                                        ))
                                        .id(),
                                );
                            }
                        }
                    }
                    commands.entity(*e).insert(NearDetail(kids));
                }
            }
        }
    }
}

/// Near trees' foliage cards: standard PBR whose crowns lean with the
/// wind (`shaders/tree_sway.wgsl`).
pub type TreeSwayMaterial = bevy::pbr::ExtendedMaterial<StandardMaterial, TreeSway>;

#[derive(Asset, bevy::render::render_resource::AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct TreeSway {
    /// xy: the wind's direction (world x/z); z: strength 0..1; w: gusts.
    #[uniform(100)]
    pub wind: Vec4,
    /// Unused by trees (the ground cover's push from the player's legs).
    #[uniform(100)]
    pub pusher: Vec4,
    /// The player's hands up in the branches: xyz where, w how far round
    /// them the foliage parts (0: not pushing; `render::hand_push`).
    #[uniform(100)]
    pub hand_a: Vec4,
    #[uniform(100)]
    pub hand_b: Vec4,
}

/// Each vertex of a foliage card: the card's own centre (tree space), so
/// the sway shader can shrink a card near the eye as a whole.
pub const ATTRIBUTE_CARD_CENTRE: bevy::mesh::MeshVertexAttribute =
    bevy::mesh::MeshVertexAttribute::new(
        "CardCentre",
        0x5EED_CA2D,
        bevy::render::render_resource::VertexFormat::Float32x3,
    );

/// A card shrinks toward its centre from this far from the eye...
pub const CARD_SHRINK_FROM: f32 = 2.0;
/// ...and is gone by this near: in a headset a card at arm's length is a
/// flat blurred sheet across the view, and under a spruce clump the cards
/// within 2 m were a quarter of the view's overdraw. The trunks and the
/// dark core stay; so do the cards' shadows.
pub const CARD_GONE_AT: f32 = 1.2;

/// How much of a card is left `d` metres from the eye (the shader's
/// `smoothstep`, for the overdraw gauge).
pub fn card_scale(d: f32) -> f32 {
    let t = ((d - CARD_GONE_AT) / (CARD_SHRINK_FROM - CARD_GONE_AT)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl bevy::pbr::MaterialExtension for TreeSway {
    fn vertex_shader() -> bevy::shader::ShaderRef {
        "shaders/tree_sway.wgsl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // The card's centre at location 8, where the mesh has it.
        if let Ok(centre) = layout
            .0
            .get_layout(&[ATTRIBUTE_CARD_CENTRE.at_shader_location(8)])
        {
            if let Some(buffer) = descriptor.vertex.buffers.first_mut() {
                buffer.attributes.extend(centre.attributes);
                descriptor.vertex.shader_defs.push("CARD_CENTRE".into());
            }
        }
        Ok(())
    }
}

fn blow_trees(
    mut materials: ResMut<Assets<TreeSwayMaterial>>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    mut last: Local<f32>,
) {
    let strength = conditions.map_or(0.4, |c| c.wind());
    if (strength - *last).abs() < 1e-3 && !materials.is_empty() && *last != 0.0 {
        return;
    }
    *last = strength;
    for (_, m) in materials.iter_mut() {
        m.extension.wind = Vec4::new(0.8, 0.6, strength, 0.8);
    }
}

/// The far crowns' material: standard PBR that dissolves near the camera.
pub type FarCrownMaterial = bevy::pbr::ExtendedMaterial<StandardMaterial, NearFade>;

#[derive(Asset, bevy::render::render_resource::AsBindGroup, Reflect, Debug, Clone)]
pub struct NearFade {
    /// x..y: fully gone at x metres, fully there at y.
    #[uniform(100)]
    pub fade: Vec4,
}

impl bevy::pbr::MaterialExtension for NearFade {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        "shaders/near_fade.wgsl".into()
    }
}

struct VariantAssets {
    /// The far foliage, when there are cards to photograph.
    impostor: Option<(
        super::impostors::Impostor,
        Handle<super::impostors::ImpostorMaterial>,
    )>,
    trunk_cpu: Mesh,
    crown_cpu: Option<Mesh>,
    /// The far LOD's.
    trunk_far: Mesh,
    crown_far: Option<Mesh>,
    bark: Handle<StandardMaterial>,
    crown: Option<Handle<Mesh>>,
    cards: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    /// The cards' material as near trees wear it: swaying.
    live: Option<Handle<TreeSwayMaterial>>,
}

struct TreeAssets {
    foliage: Handle<FarCrownMaterial>,
    core: Handle<StandardMaterial>,
    handles: std::collections::HashMap<(TreeSpecies, usize), VariantAssets>,
}

/// Foliage's alpha: a test by default; `EXTRACTION_A2C=1` turns it into
/// alpha to coverage, which lets the 4x MSAA smooth the cards' cut edges
/// (an alpha-tested edge crawls in a headset as the head moves). It's off
/// until it's been timed on the headset machine: in the software
/// renderer it cost the opaque pass 40-70% wherever foliage filled the
/// view. Compare the session log's `gpu:` lines with and without it.
pub fn foliage_alpha(cutoff: f32) -> AlphaMode {
    static A2C: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *A2C.get_or_init(|| std::env::var("EXTRACTION_A2C").is_ok_and(|v| v == "1")) {
        AlphaMode::AlphaToCoverage
    } else {
        AlphaMode::Mask(cutoff)
    }
}

fn trunk_radius(s: TreeSpecies) -> f32 {
    match s {
        TreeSpecies::Spruce => 0.22,
        TreeSpecies::Pine => 0.2,
        TreeSpecies::Birch => 0.14,
        TreeSpecies::Snag => 0.18,
    }
}

/// Accumulates triangles with per-vertex colour and UVs.
#[derive(Default)]
struct MeshBuilder {
    /// The far LOD: fewer sides and rings, no twigs.
    coarse: bool,
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    uv: Vec<[f32; 2]>,
    /// Per vertex, the card it's on's centre (cards only).
    card_centre: Vec<[f32; 3]>,
    idx: Vec<u32>,
}

/// Sprig regions of the spruce atlas (u0, v0, u1, v1), stem at the bottom:
/// the two big sprigs, then the two smaller ones above them. Measured from
/// the atlas's alpha (a rect that strays into the bark strip down the left
/// or the stems along the bottom draws an opaque brown board).
const SPRIGS: [[f32; 4]; 4] = [
    [0.300, 0.385, 0.656, 0.768],
    [0.660, 0.446, 0.935, 0.765],
    [0.183, 0.041, 0.444, 0.326],
    [0.646, 0.034, 0.944, 0.375],
];
/// The hanging birch branch, stem at the top.
const BIRCH_LEAVES: [f32; 4] = [0.21, 0.0, 0.81, 1.0];
/// Every atlas rect a card is cut from (each has an outline in
/// `render::cards`).
#[cfg(test)]
pub const CARD_RECTS: [[f32; 4]; 5] = [SPRIGS[0], SPRIGS[1], SPRIGS[2], SPRIGS[3], BIRCH_LEAVES];

impl MeshBuilder {
    fn vertex(&mut self, p: Vec3, n: Vec3, c: [f32; 4]) -> u32 {
        self.vertex_uv(p, n, c, [0.0, 0.0])
    }

    fn vertex_uv(&mut self, p: Vec3, n: Vec3, c: [f32; 4], uv: [f32; 2]) -> u32 {
        self.pos.push(p.to_array());
        self.nrm.push(n.normalize_or_zero().to_array());
        self.col.push(c);
        self.uv.push(uv);
        (self.pos.len() - 1) as u32
    }

    /// A textured card from `base` to `base + along`, `width` wide across
    /// `across`. The atlas rect's bottom edge (v1) sits at the base, unless
    /// `hanging`, when the top edge does. Normals bend outward from `centre`
    /// so a card crown lights like a volume, not a pile of planes.
    fn card(
        &mut self,
        base: Vec3,
        along: Vec3,
        across: Vec3,
        width: f32,
        rect: [f32; 4],
        centre: Vec3,
        hanging: bool,
    ) {
        let side = across.normalize_or(Vec3::X) * width * 0.5;
        let [u0, v0, u1, v1] = rect;
        let (vb, vt) = if hanging { (v0, v1) } else { (v1, v0) };
        let n = |p: Vec3| ((p - centre).normalize_or(Vec3::Y) + Vec3::Y * 0.6).normalize();
        let c = [1.0, 1.0, 1.0, 1.0];
        // The polygon round the rect's leaves, not the whole quad: the
        // empty corners were shaded for nothing (`render::cards`). Points
        // as (across 0..1, base-to-tip 0..1).
        let pts: Vec<(f32, f32, [f32; 2])> = crate::render::cards::outline(rect)
            .iter()
            .map(|&[s, t]| {
                let v = v0 + t * (v1 - v0);
                (s, (v - vb) / (vt - vb), [u0 + s * (u1 - u0), v])
            })
            .collect();
        // Wound as the quad was (counter-clockwise in those terms).
        let wound: f32 = (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum();
        let mid = (base + along * 0.5).to_array();
        let ids: Vec<u32> = pts
            .iter()
            .map(|&(s, f, uv)| {
                let p = base - side + side * 2.0 * s + along * f;
                self.card_centre.push(mid);
                self.vertex_uv(p, n(p), c, uv)
            })
            .collect();
        for i in 1..ids.len() - 1 {
            if wound > 0.0 {
                self.idx.extend_from_slice(&[ids[0], ids[i], ids[i + 1]]);
            } else {
                self.idx.extend_from_slice(&[ids[0], ids[i + 1], ids[i]]);
            }
        }
    }

    /// Two cards crossed along the same spine.
    fn cross_cards(
        &mut self,
        base: Vec3,
        along: Vec3,
        width: f32,
        rect: [f32; 4],
        centre: Vec3,
        hanging: bool,
    ) {
        let a = along.normalize_or(Vec3::Y);
        let h = a.cross(Vec3::Y).normalize_or(Vec3::X);
        let v = a.cross(h).normalize_or(Vec3::Z);
        self.card(base, along, h, width, rect, centre, hanging);
        self.card(base, along, v, width * 0.8, rect, centre, hanging);
    }

    /// A tapered tube along a polyline of (centre, radius) rings.
    fn tube(&mut self, rings: &[(Vec3, f32)], sides: usize, color: impl Fn(Vec3, f32) -> [f32; 4]) {
        let thinned: Vec<(Vec3, f32)>;
        let (rings, sides) = if self.coarse {
            // Twigs vanish at a distance; trunks keep every other ring.
            if rings.len() <= 2 && rings.first().is_some_and(|r| r.1 < 0.1) {
                return;
            }
            thinned = rings
                .iter()
                .enumerate()
                .filter(|(k, _)| k % 2 == 0 || *k + 1 == rings.len())
                .map(|(_, r)| *r)
                .collect();
            (&thinned[..], (sides / 2).max(3))
        } else {
            (rings, sides)
        };
        let base = self.pos.len() as u32;
        let mut along = 0.0;
        // Bark wraps once round the base girth, so it doesn't pinch at the top.
        let girth = rings.first().map_or(0.2, |r| r.1) * std::f32::consts::TAU;
        for (k, (c, r)) in rings.iter().enumerate() {
            let dir = if k + 1 < rings.len() {
                (rings[k + 1].0 - *c).normalize_or_zero()
            } else {
                (*c - rings[k.saturating_sub(1)].0).normalize_or_zero()
            };
            if k > 0 {
                along += c.distance(rings[k - 1].0);
            }
            let side = dir.any_orthonormal_vector();
            let up = dir.cross(side);
            for s in 0..=sides {
                let a = s as f32 / sides as f32 * std::f32::consts::TAU;
                let n = side * a.cos() + up * a.sin();
                let p = *c + n * *r;
                self.vertex_uv(p, n, color(p, a), [s as f32 / sides as f32 * girth, along]);
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
        let (rings, segs) = if self.coarse { (4, 6) } else { (7, 10) };
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
        self.build_with(true)
    }

    fn build_with(self, colours: bool) -> Mesh {
        let mut m = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_indices(Indices::U32(self.idx));
        // (`EXTRACTION_CARD_SHRINK=0`: whole cards however near, to compare.)
        let shrink = std::env::var("EXTRACTION_CARD_SHRINK").map_or(true, |v| v != "0");
        if shrink && !self.card_centre.is_empty() && self.card_centre.len() == m.count_vertices() {
            m.insert_attribute(ATTRIBUTE_CARD_CENTRE, self.card_centre);
        }
        if colours {
            m.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        } else {
            // Textured bark is normal-mapped: without tangents the main
            // pass can't draw it (the shadow pass still does, so a
            // trunkless crown casts a trunk's shadow).
            let _ = m.generate_tangents();
        }
        m
    }
}

fn lin(r: f32, g: f32, b: f32) -> [f32; 4] {
    let c = Color::srgb(r, g, b).to_linear();
    [c.red, c.green, c.blue, 1.0]
}

/// One species variant's meshes, local space, base at the origin.
pub struct TreeMeshes {
    pub trunk: Mesh,
    /// The solid crown: the far LOD, and (shrunk) the near LOD's core.
    pub crown: Option<Mesh>,
    /// Alpha-tested foliage cards: the near LOD.
    pub cards: Option<Mesh>,
}

/// Builds a species variant. `textured` drops the trunk's vertex colours
/// so scanned bark shows through.
/// `coarse` is the far LOD: the same tree (the same random draws), with
/// fewer sides, rings and whorls, and no twigs.
/// `burnt`: the valley burned (`MapDef::ash`); dead wood is charred and
/// the pines that lived through it are scorched round the foot.
fn build_tree(
    species: TreeSpecies,
    seed: u64,
    textured: bool,
    coarse: bool,
    burnt: bool,
) -> TreeMeshes {
    let mut rng = Rng::new(seed);
    let mut crng = Rng::new(seed ^ 0xCA2D);
    let mut trunk = MeshBuilder {
        coarse,
        ..default()
    };
    let mut crown = MeshBuilder {
        coarse,
        ..default()
    };
    let mut cards = MeshBuilder::default();
    let tau = std::f32::consts::TAU;
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
                let jag = rng.range(0.0, 1.0);
                // Every point draws from the generator, drawn or not, so
                // the far LOD (every other whorl, long points only) stays
                // the same tree.
                let points: Vec<(Vec3, Vec3, [f32; 4], bool)> = (0..=sides * 2)
                    .map(|s| {
                        let a = s as f32 / (sides * 2) as f32 * std::f32::consts::TAU + jag;
                        // Alternate long and short points for branch tips.
                        let rr = if s % 2 == 0 { r } else { r * 0.62 } * rng.range(0.85, 1.1);
                        let dir = Vec3::new(a.cos(), 0.0, a.sin());
                        let p = dir * rr + Vec3::Y * (y - droop * (rr / r));
                        let tip = rr / r;
                        let shade = 0.75 + 0.35 * tip + 0.15 * t;
                        let c = lin(0.11 * shade, 0.155 * shade, 0.115 * shade);
                        (p, (dir + Vec3::Y * 0.8).normalize(), c, s % 2 == 0)
                    })
                    .collect();
                if crown.coarse && w % 2 == 1 {
                    continue;
                }
                let base = crown.pos.len() as u32;
                let top = crown.vertex(
                    Vec3::new(0.0, y + r * 0.55 + 0.3, 0.0),
                    Vec3::Y,
                    lin(0.10, 0.14, 0.10),
                );
                let mut n = 0u32;
                for (p, nrm, c, long) in points {
                    if !crown.coarse || long {
                        crown.vertex(p, nrm, c);
                        n += 1;
                    }
                }
                for s in 0..n - 1 {
                    crown
                        .idx
                        .extend_from_slice(&[top, base + 2 + s, base + 1 + s]);
                }
            }
            // Cards: drooping sprig plates and upright fins, whorl by whorl.
            let wc = 22;
            for w in 0..wc {
                let t = w as f32 / (wc - 1) as f32;
                let y = first + t * (h - first - 0.7);
                let r = rmax * (1.0 - t).powf(0.85) + 0.4;
                let k = (8.0 - t * 4.0) as usize;
                let phase = crng.range(0.0, tau);
                // The last whorls' few short plates read as a cross on the
                // spire: the core's narrow top is the tip instead.
                if t > 0.9 {
                    continue;
                }
                let centre = Vec3::new(0.0, y, 0.0);
                for i in 0..k {
                    let a = phase + i as f32 / k as f32 * tau + crng.range(-0.25, 0.25);
                    let out = Vec3::new(a.cos(), 0.0, a.sin());
                    let len = r * crng.range(0.85, 1.1);
                    let along = out * len + Vec3::Y * (-0.3 * len + 0.15);
                    let base = centre + out * 0.12;
                    cards.card(
                        base,
                        along,
                        out.cross(Vec3::Y),
                        len * 0.8,
                        SPRIGS[crng.below(2)],
                        centre,
                        false,
                    );
                    cards.card(
                        base + Vec3::Y * 0.05,
                        along * 0.85,
                        Vec3::Y,
                        len * 0.6,
                        SPRIGS[2 + crng.below(2)],
                        centre,
                        false,
                    );
                }
            }
        }
        TreeSpecies::Pine => {
            let h = rng.range(15.0, 21.0);
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
            // (Its own generator: the tree's shape is the same burnt or not.)
            let scorch = if burnt {
                Rng::new(seed ^ 0x5C02).range(1.0, 2.6)
            } else {
                0.0
            };
            trunk.tube(&rings, 7, |p, a| {
                // Grey-brown below, the orange "fox" bark of a Scots pine above.
                let t = (p.y / h).clamp(0.0, 1.0);
                let lo = Vec3::new(0.24, 0.20, 0.17);
                let hi = Vec3::new(0.55, 0.32, 0.18);
                let mut c = lo.lerp(hi, ((t - 0.35) * 2.5).clamp(0.0, 1.0));
                // Fire-blackened round the foot, higher on one side.
                let line = scorch * (0.75 + 0.35 * a.sin());
                if p.y < line {
                    c *= 0.22;
                }
                lin(c.x, c.y, c.z)
            });
            // A broad, ragged umbrella: clumps of needles out on long limbs
            // at uneven heights, mostly near the top. (A few big balls
            // stacked up the trunk read as topiary in the first headset
            // session.)
            let crown_base = h * rng.range(0.48, 0.58);
            let clumps = 17 + rng.below(6);
            for _ in 0..clumps {
                let t = rng.range(0.0, 1.0).powf(0.6);
                let a = rng.range(0.0, std::f32::consts::TAU);
                let y = crown_base + t * (h - crown_base) * 0.97;
                let reach = rng.range(1.0, 3.8) * (1.0 - t * 0.5);
                let off = Vec3::new(a.cos(), 0.0, a.sin()) * reach;
                let c = Vec3::new(0.0, y, 0.0) + lean * (y / h).powi(2) + off;
                // Flat pads, wider than deep: a Scots pine's crown is
                // layered plates of needles, not balls.
                let rad = Vec3::new(
                    rng.range(1.5, 2.3),
                    rng.range(0.5, 0.75),
                    rng.range(1.5, 2.3),
                ) * (1.0 - t * 0.3);
                crown.blob(c, rad * 0.8, rng.next_u64() as u32, |_, l| {
                    let s = 0.55 + 0.45 * l;
                    lin(0.12 * s, 0.16 * s, 0.09 * s)
                });
                // A limb out to the clump, rising a little, and needle
                // cards round it.
                let on_trunk =
                    Vec3::new(0.0, c.y - 0.3 - reach * 0.25, 0.0) + lean * (c.y / h).powi(2);
                trunk.tube(&[(on_trunk, 0.06), (c, 0.025)], 5, |_, _| {
                    lin(0.5, 0.32, 0.2)
                });
                // Needle tufts spread through the pad, lying near level
                // and tipped up at the ends. (Fanned out from one point
                // they made a starburst: palms, in the headset.)
                for _ in 0..14 {
                    let a = crng.range(0.0, tau);
                    let dir = Vec3::new(a.cos(), crng.range(-0.05, 0.3), a.sin()).normalize();
                    let b = crng.range(0.0, tau);
                    let at = c + Vec3::new(
                        b.cos() * rad.x * crng.range(0.0, 0.7),
                        crng.range(-0.3, 0.4) * rad.y,
                        b.sin() * rad.z * crng.range(0.0, 0.7),
                    );
                    let len = rad.x * crng.range(0.55, 0.85);
                    cards.cross_cards(
                        at - dir * len * 0.5,
                        dir * len,
                        len * 0.8,
                        SPRIGS[crng.below(4)],
                        c,
                        false,
                    );
                }
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
            // One loose, narrow oval crown of hanging twigs: many small
            // clusters scattered through it, each a dark core with lighter
            // drooping cards round it. (A few big solid blobs strung up the
            // trunk looked like topiary in the first headset session.)
            let crown_base = h * 0.4;
            let top = h * 1.02;
            let width = rng.range(1.7, 2.5);
            let clusters = 14 + rng.below(5);
            // Late summer: a few birches are starting to turn.
            let turned = rng.range(0.0, 0.4);
            for b in 0..clusters {
                let t = ((b as f32 + rng.range(0.0, 1.0)) / clusters as f32).min(0.98);
                let y = crown_base + t * (top - crown_base);
                // Widest a third of the way up, closing to a point at the top.
                let envelope = width * (std::f32::consts::PI * (0.12 + 0.88 * t)).sin().powf(0.7);
                let a = rng.range(0.0, std::f32::consts::TAU);
                let off = Vec3::new(a.cos(), 0.0, a.sin()) * envelope * rng.range(0.35, 1.0);
                let c = Vec3::new(0.0, y, 0.0) + lean * (y / h).powi(2) + off;
                let rad = Vec3::splat(rng.range(0.5, 0.85)) * Vec3::new(1.0, 0.8, 1.0);
                let yellow = (turned + rng.range(-0.2, 0.2)).clamp(0.0, 1.0);
                // A branch out to the cluster, with leaves hanging off it.
                let on_trunk = Vec3::new(0.0, c.y - 0.7, 0.0) + lean * (c.y / h).powi(2);
                trunk.tube(
                    &[(on_trunk, 0.035), (c + Vec3::Y * 0.3, 0.012)],
                    5,
                    |_, _| lin(0.7, 0.68, 0.64),
                );
                for _ in 0..5 {
                    let a = crng.range(0.0, tau);
                    let out = Vec3::new(a.cos(), 0.0, a.sin());
                    let base = c
                        + out * rad.x * crng.range(0.3, 1.1)
                        + Vec3::Y * rad.y * crng.range(0.0, 0.9);
                    let along = (out * 0.35 - Vec3::Y) * crng.range(1.0, 1.6);
                    let across = Vec3::new(-out.z, 0.0, out.x);
                    cards.card(base, along, across, 1.1, BIRCH_LEAVES, c, true);
                    cards.card(base, along, out, 0.9, BIRCH_LEAVES, c, true);
                }
                crown.blob(c, rad, rng.next_u64() as u32, move |_, l| {
                    let s = 0.45 + 0.35 * l;
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
            // Dead wood weathers silver; in a burnt valley it's charcoal
            // from the foot up to a ragged line, and soot-dark above.
            let char_line = if burnt {
                Rng::new(seed ^ 0xC4A2).range(1.5, 4.5)
            } else {
                0.0
            };
            let s = seed as u32;
            trunk.tube(&rings, 6, |p, a| {
                let v = 0.36 + 0.06 * (p.y * 3.1).sin();
                if burnt {
                    let rag = extraction_core::noise::perlin(a * 1.5, p.y * 0.8, s) * 1.2;
                    if p.y < char_line + rag {
                        let c = 0.05 + 0.03 * (p.y * 7.0 + a * 3.0).sin().abs();
                        return lin(c, c * 0.95, c * 0.9);
                    }
                    let v = v * 0.62;
                    return lin(v, v * 0.95, v * 0.88);
                }
                lin(v, v * 0.97, v * 0.93)
            });
            // Broken branch stubs.
            for _ in 0..(3 + rng.below(4)) {
                let y = rng.range(2.0, h * 0.9);
                let a = rng.range(0.0, std::f32::consts::TAU);
                let dir = Vec3::new(a.cos(), rng.range(-0.1, 0.5), a.sin()).normalize();
                let start = Vec3::new(0.0, y, 0.0) + lean * (y / h);
                let len = rng.range(0.6, 2.2);
                let v = if burnt { 0.1 } else { 0.33 };
                trunk.tube(&[(start, 0.06), (start + dir * len, 0.02)], 4, |_, _| {
                    lin(v, v * 0.97, v * 0.91)
                });
            }
        }
    }
    TreeMeshes {
        trunk: trunk.build_with(!textured),
        crown: (!crown.pos.is_empty()).then(|| crown.build()),
        cards: (!cards.pos.is_empty()).then(|| cards.build_with(false)),
    }
}

#[cfg(test)]
mod atlas_tests {
    /// Every sprig rect holds needles (but isn't solid) and stays clear of
    /// the atlas's opaque bark strip and stems (`fetch_assets.py` builds the atlas; the
    /// test is skipped when it hasn't been run).
    #[test]
    fn sprig_rects_are_mostly_clear() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/textures/foliage/spruce_branch/albedo.png"
        );
        let Ok(bytes) = std::fs::read(path) else {
            eprintln!("no atlas at {path}: skipped");
            return;
        };
        let img = bevy::image::Image::from_buffer(
            &bytes,
            bevy::image::ImageType::Extension("png"),
            bevy::image::CompressedImageFormats::NONE,
            false,
            bevy::image::ImageSampler::Default,
            bevy::asset::RenderAssetUsages::default(),
        )
        .unwrap();
        let (w, h) = (img.width(), img.height());
        let data = img.data.as_ref().unwrap();
        struct Px<'a>(&'a [u8], u32);
        impl Px<'_> {
            fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
                let i = ((y * self.1 + x) * 4) as usize;
                [self.0[i], self.0[i + 1], self.0[i + 2], self.0[i + 3]]
            }
        }
        let img = Px(data, w);
        for r in super::SPRIGS {
            let (x0, y0) = ((r[0] * w as f32) as u32, (r[1] * h as f32) as u32);
            let (x1, y1) = ((r[2] * w as f32) as u32, (r[3] * h as f32) as u32);
            let mut opaque = 0usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    opaque += (img.get_pixel(x, y)[3] > 115) as usize;
                }
            }
            let frac = opaque as f32 / ((x1 - x0) * (y1 - y0)) as f32;
            assert!(
                (0.05..0.45).contains(&frac),
                "sprig {r:?} is {:.0}% opaque: off its sprig?",
                frac * 100.0
            );
            // Clear of the bark strip down the left and the stems along
            // the bottom (whose diagonal reaches up the right edge).
            assert!(
                r[0] > 0.17 && r[3] < 0.77,
                "sprig {r:?} reaches the strip or stems"
            );
        }
    }
}

#[cfg(test)]
mod card_shrink_tests {
    use super::*;

    /// Every card vertex knows its card's centre (each card's corners
    /// share one, so the shader shrinks a card whole), and nothing but the
    /// cards carries it.
    #[test]
    fn cards_carry_their_centres() {
        for species in [TreeSpecies::Spruce, TreeSpecies::Pine, TreeSpecies::Birch] {
            let m = build_tree(species, 1017, true, false, false);
            let cards = m.cards.expect("textured trees have cards");
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(centres)) =
                cards.attribute(ATTRIBUTE_CARD_CENTRE)
            else {
                panic!("{species:?}: no card centres");
            };
            assert_eq!(centres.len(), cards.count_vertices());
            let pos = cards
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap();
            // A centre is near its own corners.
            let far = pos
                .iter()
                .zip(centres)
                .map(|(p, c)| Vec3::from(*p).distance(Vec3::from(*c)))
                .fold(0.0f32, f32::max);
            println!("{species:?}: corners at most {far:.2} m from their card's centre");
            assert!(
                far < 4.0,
                "{species:?}: a corner {far:.1} m from its centre"
            );
            assert!(m.trunk.attribute(ATTRIBUTE_CARD_CENTRE).is_none());
        }
    }

    /// Whole from 2 m, gone by 1.2 m, smoothly between.
    #[test]
    fn a_card_shrinks_near_the_eye() {
        assert_eq!(card_scale(0.5), 0.0);
        assert_eq!(card_scale(CARD_GONE_AT), 0.0);
        assert_eq!(card_scale(CARD_SHRINK_FROM), 1.0);
        assert_eq!(card_scale(10.0), 1.0);
        let mut last = 0.0;
        for i in 0..=40 {
            let s = card_scale(1.2 + 0.02 * i as f32);
            assert!(s >= last && s - last < 0.06, "no jump at {i}: {last} > {s}");
            last = s;
        }
    }
}
