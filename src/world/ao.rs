//! Ambient occlusion for the buildings the recipes build from boxes
//! (`world::buildings`), baked into vertex colours when a building's
//! meshes are made: the floor darkens along the walls and round the
//! furniture's feet, the walls into their corners and under the eaves.
//! The props' models carry their own, baked in Blender (`art/props/_kit.py`
//! `bake_ao`); `StandardMaterial` multiplies a mesh's vertex colours into
//! its base colour, so neither needs a shader of its own.
//!
//! Each vertex casts a few short rays over its hemisphere against the
//! building's pieces (as boxes); the nearer what they meet, the darker.
//!
//! The colour's alpha carries the sky above the vertex, for the rain: 1
//! open, 0 under a roof or the eaves (one ray straight up). Opaque
//! materials ignore alpha; the wet-surface patch (`render::fog`) reads it,
//! so what's under a roof stays dry.
//! A box's faces are cut where the shading changes, so the darkening has
//! vertices to sit on; faces buried in other pieces aren't made at all.

use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

/// Whether to bake at all: `EXTRACTION_NO_AO=1` leaves it out (and the
/// props' baked colours out of the buildings they're merged into), to see
/// or measure the difference.
pub fn enabled() -> bool {
    std::env::var_os("EXTRACTION_NO_AO").is_none()
}

/// Whether to bake the sky's light through the openings too (`SkyField`):
/// `EXTRACTION_NO_SKY_VIEW=1` leaves it out (every indoor surface lit
/// alike, as before), to see or measure the difference.
pub fn sky_view_enabled() -> bool {
    enabled() && std::env::var_os("EXTRACTION_NO_SKY_VIEW").is_none()
}

/// What a house's room has of the sky field ([`SkyField`]) on average,
/// and the share of its light before the field a surface with that much
/// keeps (as it was: the windows' side gets more, the far side less).
pub const SKY_ROOM: f32 = 0.08;
pub const SKY_ROOM_LIGHT: f32 = 1.0;
/// How steeply the light falls off from an opening: in a 5 m room the
/// wall facing the window comes out two and a half to three times the
/// wall beside it, the far corners dark (a real room falls ten to one;
/// three keeps what you came for findable).
pub const SKY_FALLOFF: f32 = 2.0;
/// The least a surface keeps, deep in a shut room, and the most one by
/// an opening gets, as shares of what it has now.
pub const SKY_MIN: f32 = 0.15;
pub const SKY_MAX: f32 = 2.0;

/// The sky's light on a surface with `view` of the sky field, as a share
/// of what the renderer gives it now. The ambient stays dimmed to
/// `weather::INTERIOR_LIGHT` while the eye is inside (so the eyes' tuning
/// holds); this shares it out over the room. Out in the open air (and so
/// outside walls), as now. Colour only: the vertex alpha is the sky over
/// it, for the wet and the snow.
pub fn sky_light(view: f32) -> f32 {
    let inside = (SKY_ROOM_LIGHT * (view / SKY_ROOM).powf(SKY_FALLOFF)).clamp(SKY_MIN, SKY_MAX);
    let open = ((view - 0.6) / 0.35).clamp(0.0, 1.0);
    let open = open * open * (3.0 - 2.0 * open);
    inside + (1.0 - inside) * open
}

/// How far over where a thing lies its light is read (m).
pub const LYING: f32 = 0.15;

/// The least a piece of furniture keeps of its light today, however
/// dark its corner: its silhouette and grain still read.
pub const PIECE_MIN: f32 = 0.3;

/// The light a piece of furniture standing in `lo..hi` (building-local)
/// takes from the room ([`sky_light`]): the brightest of the air beside
/// its four sides at half its height and over its top (not the inside of
/// it, a solid cell), never darker than the floor it stands on, nor than
/// [`PIECE_MIN`].
pub fn piece_light(field: &SkyField, lo: Vec3, hi: Vec3) -> f32 {
    let mid = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5;
    let off = field.cell * 0.75;
    let around = [
        Vec3::new(mid.x + half.x + off, mid.y, mid.z),
        Vec3::new(mid.x - half.x - off, mid.y, mid.z),
        Vec3::new(mid.x, mid.y, mid.z + half.z + off),
        Vec3::new(mid.x, mid.y, mid.z - half.z - off),
        Vec3::new(mid.x, hi.y + off, mid.z),
    ];
    let side = around.iter().map(|p| field.at(*p)).fold(0.0, f32::max);
    let floor = [
        Vec3::new(mid.x + half.x + off, lo.y + off, mid.z),
        Vec3::new(mid.x - half.x - off, lo.y + off, mid.z),
        Vec3::new(mid.x, lo.y + off, mid.z + half.z + off),
        Vec3::new(mid.x, lo.y + off, mid.z - half.z - off),
    ]
    .iter()
    .map(|p| field.at(*p))
    .fold(0.0, f32::max);
    sky_light(side).max(sky_light(floor)).max(PIECE_MIN)
}

/// Scales a mesh's vertex colours (not their alpha) by `k`: a piece of
/// furniture lit as the spot in the room it stands in.
pub fn tint(mesh: &mut Mesh, k: f32) {
    ensure_colours(mesh);
    if let Some(VertexAttributeValues::Float32x4(c)) = mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR) {
        for c in c.iter_mut() {
            c[0] *= k;
            c[1] *= k;
            c[2] *= k;
        }
    }
}

/// How far a vertex looks for what shades it.
pub const REACH: f32 = 1.0;
/// How dark a fully shut-in corner gets (as the props' bake).
pub const STRENGTH: f32 = 0.75;
/// Short faces that darken are cut into cells no bigger than this.
pub const CELL: f32 = 0.6;
const RAYS: usize = 24;

/// A piece of the building as an oriented box.
#[derive(Clone, Copy)]
struct Obb {
    centre: Vec3,
    rot: Quat,
    half: Vec3,
}

impl Obb {
    /// Where a ray from `o` along `d` meets the box, if it starts outside
    /// it and meets it within `max`.
    fn hit(&self, o: Vec3, d: Vec3, max: f32) -> Option<f32> {
        let inv = self.rot.inverse();
        let o = inv * (o - self.centre);
        let d = inv * d;
        let (mut t0, mut t1) = (0.0f32, max);
        for k in 0..3 {
            let (oo, dd, h) = (o[k], d[k], self.half[k]);
            if dd.abs() < 1e-8 {
                if oo.abs() > h {
                    return None;
                }
                continue;
            }
            let (a, b) = ((-h - oo) / dd, (h - oo) / dd);
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            t0 = t0.max(a);
            t1 = t1.min(b);
            if t0 > t1 {
                return None;
            }
        }
        // Starting inside (a vertex buried where two pieces join) isn't
        // shade: what's buried isn't seen.
        let inside = (0..3).all(|k| o[k].abs() < self.half[k]);
        (!inside && t0 > 0.0).then_some(t0)
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let c = self.centre + self.rot * (self.half * Vec3::new(sx, sy, sz));
                    lo = lo.min(c);
                    hi = hi.max(c);
                }
            }
        }
        (lo, hi)
    }
}

/// The sky field's cells (m), twice that in a hall of more than
/// `SKY_CELLS_MAX` of them.
pub const SKY_CELL: f32 = 0.5;
const SKY_CELLS_MAX: f32 = 60_000.0;
/// The field has settled when no cell moves more than this in a pass.
const SKY_SETTLED: f32 = 1e-3;
const SKY_PASSES_MAX: usize = 400;
/// How much of a cell's light a solid face beside it gives back (a wall's,
/// a floor's: the light that comes round corners).
pub const SKY_BOUNCE: f32 = 0.55;

/// The sky's light through a building, on a coarse grid: 1 outside (and
/// under open sky, through a hole in the roof), spread into the rooms
/// through their openings, each cell the mean of its open neighbours'
/// and, for a solid one, `SKY_BOUNCE` of its own. A room by a window is
/// lit, its far end less, the one through the next doorway less again;
/// a shut room has only what comes in by its openings. Not traced: what
/// the sun lights (the shadow maps do).
pub struct SkyField {
    lo: Vec3,
    cell: f32,
    dims: [usize; 3],
    light: Vec<f32>,
}

impl SkyField {
    fn new(occ: &Occluders) -> Self {
        // The building's bounds and a cell of open air round them; a big
        // hall's in coarser cells.
        let inner = occ.lo + Vec3::splat(REACH);
        let outer = occ.lo
            + Vec3::new(occ.dims[0] as f32, occ.dims[1] as f32, occ.dims[2] as f32) * occ.cell
            - Vec3::splat(REACH);
        let span = outer - inner;
        let cell = if span.x * span.y * span.z / SKY_CELL.powi(3) > SKY_CELLS_MAX {
            SKY_CELL * 2.0
        } else {
            SKY_CELL
        };
        let lo = inner - Vec3::splat(cell);
        let size = outer + Vec3::splat(cell) - lo;
        let dims = [
            (size.x / cell).ceil().max(3.0) as usize,
            (size.y / cell).ceil().max(3.0) as usize,
            (size.z / cell).ceil().max(3.0) as usize,
        ];
        let at = |x: usize, y: usize, z: usize| {
            lo + (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * cell
        };
        let idx = |x: usize, y: usize, z: usize| (z * dims[1] + y) * dims[0] + x;
        let n = dims[0] * dims[1] * dims[2];
        let mut solid = vec![false; n];
        let mut fixed = vec![false; n];
        let mut light = vec![0.0f32; n];
        for z in 0..dims[2] {
            for y in 0..dims[1] {
                for x in 0..dims[0] {
                    let i = idx(x, y, z);
                    let p = at(x, y, z);
                    solid[i] = occ.inside(p);
                    let edge = x == 0
                        || z == 0
                        || y + 1 == dims[1]
                        || x + 1 == dims[0]
                        || z + 1 == dims[2];
                    // The open air round it, under open sky, and out of
                    // its walls (under the eaves, on a porch).
                    let out = occ
                        .walls
                        .is_some_and(|h| p.x.abs() > h.x + 0.1 || p.z.abs() > h.y + 0.1);
                    if !solid[i] && (edge || out || occ.sky(p, Vec3::Y) > 0.5) {
                        fixed[i] = true;
                        light[i] = 1.0;
                    }
                }
            }
        }
        let mut next = light.clone();
        // Until it settles: no cell moving more than `SKY_SETTLED` a pass.
        for _ in 0..SKY_PASSES_MAX {
            let mut moved = 0.0f32;
            for z in 0..dims[2] {
                for y in 0..dims[1] {
                    for x in 0..dims[0] {
                        let i = idx(x, y, z);
                        if solid[i] || fixed[i] {
                            continue;
                        }
                        let mut sum = 0.0;
                        let mut k = 0.0;
                        let mut each = |j: Option<usize>| {
                            match j {
                                Some(j) if !solid[j] => sum += light[j],
                                // Ground under the bottom row: a floor.
                                _ => sum += SKY_BOUNCE * light[i],
                            }
                            k += 1.0;
                        };
                        each((x > 0).then(|| idx(x - 1, y, z)));
                        each((x + 1 < dims[0]).then(|| idx(x + 1, y, z)));
                        each((y > 0).then(|| idx(x, y - 1, z)));
                        each((y + 1 < dims[1]).then(|| idx(x, y + 1, z)));
                        each((z > 0).then(|| idx(x, y, z - 1)));
                        each((z + 1 < dims[2]).then(|| idx(x, y, z + 1)));
                        next[i] = sum / k;
                        moved = moved.max((next[i] - light[i]).abs());
                    }
                }
            }
            std::mem::swap(&mut light, &mut next);
            if moved < SKY_SETTLED {
                break;
            }
        }
        // A solid cell takes its open neighbours' best (a face's light is
        // the air in front of it).
        for i in 0..n {
            if solid[i] {
                light[i] = 0.0;
            }
        }
        Self {
            lo,
            cell,
            dims,
            light,
        }
    }

    /// The light at `p` in the open: the cell's, or if that's solid (a
    /// shelf's box round what lies on it, a wall), the brightest open cell
    /// next to it.
    pub fn open_at(&self, p: Vec3) -> f32 {
        let here = self.at(p);
        if here > 1e-6 {
            return here;
        }
        let mut best = 0.0f32;
        for r in 1..=2 {
            for dx in -r..=r {
                for dy in -r..=r {
                    for dz in -r..=r {
                        let d = Vec3::new(dx as f32, dy as f32, dz as f32) * self.cell;
                        best = best.max(self.at(p + d));
                    }
                }
            }
            if best > 1e-6 {
                break;
            }
        }
        best
    }

    /// The light in the cell at `p` (1 outside the field).
    pub fn at(&self, p: Vec3) -> f32 {
        let c = ((p - self.lo) / self.cell).floor();
        if c.min_element() < 0.0 {
            return 1.0;
        }
        let (x, y, z) = (c.x as usize, c.y as usize, c.z as usize);
        if x >= self.dims[0] || y >= self.dims[1] || z >= self.dims[2] {
            return 1.0;
        }
        self.light[(z * self.dims[1] + y) * self.dims[0] + x]
    }
}

/// What can shade a building's surfaces, binned on a coarse grid.
pub struct Occluders {
    boxes: Vec<Obb>,
    lo: Vec3,
    cell: f32,
    dims: [usize; 3],
    bins: Vec<Vec<u32>>,
    dirs: Vec<Vec3>,
    /// The boxes over each column of the grid (x, z), for the sky.
    columns: Vec<Vec<u32>>,
    /// The sky's light through the building, made the first time it's
    /// asked for.
    field: std::sync::OnceLock<SkyField>,
    /// Its walls' half extents (x, z), if known (`within`).
    walls: Option<Vec2>,
}

impl Occluders {
    /// From (half extents, pose) of a building's solid pieces.
    pub fn new(pieces: impl IntoIterator<Item = (Vec3, Transform)>) -> Self {
        let boxes: Vec<Obb> = pieces
            .into_iter()
            .filter(|(h, _)| h.min_element() > 0.005)
            .map(|(half, at)| Obb {
                centre: at.translation,
                rot: at.rotation,
                half,
            })
            .collect();
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for b in &boxes {
            let (l, h) = b.bounds();
            lo = lo.min(l);
            hi = hi.max(h);
        }
        if boxes.is_empty() {
            lo = Vec3::ZERO;
            hi = Vec3::ONE;
        }
        let cell = 1.0;
        let lo = lo - Vec3::splat(REACH);
        let size = (hi + Vec3::splat(REACH)) - lo;
        let dims = [
            (size.x / cell).ceil().max(1.0) as usize,
            (size.y / cell).ceil().max(1.0) as usize,
            (size.z / cell).ceil().max(1.0) as usize,
        ];
        let mut bins = vec![Vec::new(); dims[0] * dims[1] * dims[2]];
        for (i, b) in boxes.iter().enumerate() {
            let (l, h) = b.bounds();
            let a = ((l - Vec3::splat(REACH) - lo) / cell)
                .floor()
                .max(Vec3::ZERO);
            let z = ((h + Vec3::splat(REACH) - lo) / cell).floor();
            for x in a.x as usize..=(z.x as usize).min(dims[0] - 1) {
                for y in a.y as usize..=(z.y as usize).min(dims[1] - 1) {
                    for zz in a.z as usize..=(z.z as usize).min(dims[2] - 1) {
                        bins[(zz * dims[1] + y) * dims[0] + x].push(i as u32);
                    }
                }
            }
        }
        let mut columns = vec![Vec::new(); dims[0] * dims[2]];
        for (i, b) in boxes.iter().enumerate() {
            let (l, h) = b.bounds();
            let a = ((l - lo) / cell).floor().max(Vec3::ZERO);
            let z = ((h - lo) / cell).floor();
            for x in a.x as usize..=(z.x as usize).min(dims[0] - 1) {
                for zz in a.z as usize..=(z.z as usize).min(dims[2] - 1) {
                    columns[zz * dims[0] + x].push(i as u32);
                }
            }
        }
        // Cosine-weighted directions about +Y, the same for every vertex
        // (a fixed pattern, so a rebuild bakes the same).
        let dirs = (0..RAYS)
            .map(|k| {
                let u = (k as f32 + 0.5) / RAYS as f32;
                let a = k as f32 * 2.399_963; // the golden angle
                let r = u.sqrt();
                Vec3::new(r * a.cos(), (1.0 - u).max(0.0).sqrt(), r * a.sin())
            })
            .collect();
        Self {
            boxes,
            lo,
            cell,
            dims,
            bins,
            dirs,
            columns,
            field: std::sync::OnceLock::new(),
            walls: None,
        }
    }

    /// The sky over a point on a surface facing `n`: 1 if rain falls on
    /// it, 0 under something (one ray straight up).
    pub fn sky(&self, p: Vec3, n: Vec3) -> f32 {
        let c = ((p - self.lo) / self.cell).floor();
        if c.x < 0.0 || c.z < 0.0 {
            return 1.0;
        }
        let (x, z) = (c.x as usize, c.z as usize);
        if x >= self.dims[0] || z >= self.dims[2] {
            return 1.0;
        }
        let o = p + n * 0.02 + Vec3::Y * 0.01;
        let covered = self.columns[z * self.dims[0] + x]
            .iter()
            .any(|&i| self.boxes[i as usize].hit(o, Vec3::Y, 100.0).is_some());
        if covered { 0.0 } else { 1.0 }
    }

    /// The sky's light in the open cell off a surface facing `n` at `p`
    /// ([`SkyField`]), 0..=1: 1 out in the open.
    pub fn sky_view(&self, p: Vec3, n: Vec3) -> f32 {
        let field = self.field.get_or_init(|| SkyField::new(self));
        field.at(p + n * (field.cell * 0.6))
    }

    /// The same, knowing the building's walls stand round its footprint
    /// (`map::prefab_footprint`, half extents x, z): past them is the open
    /// air, eaves and porches included.
    pub fn within(mut self, walls: Vec2) -> Self {
        self.walls = Some(walls);
        self
    }

    /// The sky field's light where a thing lies at `p` (in the open over
    /// it: `SkyField::open_at`), as loot is tinted (`render::loot_light`).
    pub fn lying_at(&self, p: Vec3) -> f32 {
        let field = self.field.get_or_init(|| SkyField::new(self));
        field.open_at(p + Vec3::Y * LYING)
    }

    /// The sky field, if it was made (for the furniture placed later).
    pub fn into_field(self) -> Option<SkyField> {
        self.field.into_inner()
    }

    /// The light at a point from its occlusion near to and, with the sky
    /// view on, the sky it sees ([`sky_light`]).
    pub fn shade(&self, p: Vec3, n: Vec3) -> f32 {
        let near = self.light(p, n);
        if sky_view_enabled() {
            near * sky_light(self.sky_view(p, n))
        } else {
            near
        }
    }

    /// The top of what covers a point (where a ray straight up from it
    /// last meets a piece), if anything does: a roof over a room, the
    /// highest of a tower's floors.
    pub fn cover_top(&self, p: Vec3) -> Option<f32> {
        let c = ((p - self.lo) / self.cell).floor();
        if c.x < 0.0 || c.z < 0.0 {
            return None;
        }
        let (x, z) = (c.x as usize, c.z as usize);
        if x >= self.dims[0] || z >= self.dims[2] {
            return None;
        }
        self.columns[z * self.dims[0] + x]
            .iter()
            .filter_map(|&i| self.boxes[i as usize].hit(p, Vec3::Y, 100.0))
            .map(|t| p.y + t)
            .reduce(f32::max)
    }

    /// Whether a point is inside one of the pieces.
    pub fn inside(&self, p: Vec3) -> bool {
        self.near(p).iter().any(|&i| {
            let b = &self.boxes[i as usize];
            let q = b.rot.inverse() * (p - b.centre);
            (0..3).all(|k| q[k].abs() < b.half[k])
        })
    }

    fn near(&self, p: Vec3) -> &[u32] {
        let c = ((p - self.lo) / self.cell).floor();
        if c.min_element() < 0.0 {
            return &[];
        }
        let (x, y, z) = (c.x as usize, c.y as usize, c.z as usize);
        if x >= self.dims[0] || y >= self.dims[1] || z >= self.dims[2] {
            return &[];
        }
        &self.bins[(z * self.dims[1] + y) * self.dims[0] + x]
    }

    /// The light reaching a point on a surface facing `n`, 1 (open) down
    /// to 1 - STRENGTH (shut in).
    pub fn light(&self, p: Vec3, n: Vec3) -> f32 {
        let near = self.near(p);
        if near.is_empty() {
            return 1.0;
        }
        let turn = Quat::from_rotation_arc(Vec3::Y, n);
        let o = p + n * 0.01;
        let mut shade = 0.0;
        for d in &self.dirs {
            let d = turn * *d;
            let mut best = REACH;
            for &i in near {
                if let Some(t) = self.boxes[i as usize].hit(o, d, best) {
                    best = t;
                }
            }
            if best < REACH {
                shade += 1.0 - best / REACH;
            }
        }
        let occ = (shade / RAYS as f32 * 2.0).min(1.0);
        1.0 - STRENGTH * occ
    }

    /// Bakes the light at each vertex of a mesh (in the occluders' space)
    /// into its vertex colours.
    pub fn bake(&self, mesh: &mut Mesh) {
        if !enabled() {
            ensure_colours(mesh);
            return;
        }
        let (Some(VertexAttributeValues::Float32x3(p)), Some(VertexAttributeValues::Float32x3(n))) = (
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
        ) else {
            return;
        };
        let col: Vec<[f32; 4]> = p
            .iter()
            .zip(n)
            .map(|(p, n)| {
                let (p, n) = (Vec3::from(*p), Vec3::from(*n).normalize_or(Vec3::Y));
                let v = self.shade(p, n);
                [v, v, v, self.sky(p, n)]
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    }
}

/// Gives a mesh white vertex colours if it has none, so it merges with
/// baked ones.
pub fn ensure_colours(mesh: &mut Mesh) {
    if mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none() {
        let n = mesh.count_vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; n]);
    }
}

/// A box, in the building's space, with its occlusion baked: UVs in metres
/// measured from the building's origin in the box's orientation (so the
/// boxes of a wall carry its courses across; see `buildings`), tangents.
///
/// Cut only where the shading changes: a face that's buried in another
/// piece (a wall's end in its neighbour, a plinth's top under the floor)
/// isn't made at all; one that's evenly lit is one quad; one that darkens
/// is cut in bands a quarter and three quarters of a metre in from its
/// edges, where the darkening is (a short face, into cells of [`CELL`]).
///
/// `sky`: carry the sky over each vertex in the colours' alpha (see the
/// module docs); false for a blended material (glass), whose alpha is its
/// own.
pub fn shaded_box(size: Vec3, at: Transform, occ: &Occluders, sky: bool) -> Mesh {
    let h = size * 0.5;
    let offset = at.rotation.inverse() * at.translation;
    // (normal, u axis, v axis) per face, as `metric_box`.
    let faces = [
        (Vec3::X, Vec3::NEG_Z, Vec3::NEG_Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::NEG_Y),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::Z, Vec3::X, Vec3::NEG_Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::NEG_Y),
    ];
    let world = |p: Vec3| at.transform_point(p);
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    let mut col = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for (n, u, v) in faces {
        let hu = (u * h).abs().max_element();
        let hv = (v * h).abs().max_element();
        let hn = (n * h).abs().max_element();
        let wn = at.rotation * n;
        let centre = n * hn;
        // Dropped only when it's covered all over (its middle and its
        // corners): a shelf's box against the middle of a wall hides only
        // that much of it (and is drawn as a model, not a box).
        let covered = [
            (0.0, 0.0),
            (-0.9, -0.9),
            (0.9, -0.9),
            (0.9, 0.9),
            (-0.9, 0.9),
        ]
        .iter()
        .all(|&(su, sv)| {
            let p = centre + u * hu * su + v * hv * sv;
            occ.inside(world(p) + wn * 0.02)
        });
        if covered {
            continue;
        }
        // The light a little in from a point of the face (a corner sits on
        // the next piece's edge; just inside it, it's what's seen).
        let light = |su: f32, sv: f32| {
            let inset = |s: f32, half: f32| s * (half - 0.02).max(half * 0.5) / half.max(1e-4);
            let p = centre + u * hu * inset(su, hu) + v * hv * inset(sv, hv);
            if enabled() {
                occ.shade(world(p), wn)
            } else {
                1.0
            }
        };
        let probe = [
            light(-1.0, -1.0),
            light(1.0, -1.0),
            light(1.0, 1.0),
            light(-1.0, 1.0),
            light(0.0, 0.0),
        ];
        let even = probe
            .iter()
            .fold(0.0f32, |m, l| m.max((l - probe[4]).abs()))
            < 0.04;
        let cuts = |half: f32| -> Vec<f32> {
            let len = 2.0 * half;
            if even {
                vec![-1.0, 1.0]
            } else if len > 1.8 {
                [0.0, 0.25, 0.75, len - 0.75, len - 0.25, len]
                    .iter()
                    .map(|d| d / half - 1.0)
                    .collect()
            } else {
                let k = (len / CELL).ceil().max(1.0) as usize;
                (0..=k).map(|i| -1.0 + 2.0 * i as f32 / k as f32).collect()
            }
        };
        let (us, vs) = (cuts(hu), cuts(hv));
        let base = pos.len() as u32;
        for &sv in &vs {
            for &su in &us {
                let p = centre + u * hu * su + v * hv * sv;
                pos.push(world(p).to_array());
                nrm.push(wn.to_array());
                let r = p + offset;
                uv.push([r.dot(u), r.dot(v)]);
                let l = if even {
                    // The probes already have the corners.
                    match (su > 0.0, sv > 0.0) {
                        (false, false) => probe[0],
                        (true, false) => probe[1],
                        (true, true) => probe[2],
                        (false, true) => probe[3],
                    }
                } else {
                    light(su, sv)
                };
                let open = if sky && enabled() {
                    let inset =
                        |s: f32, half: f32| s * (half - 0.02).max(half * 0.5) / half.max(1e-4);
                    occ.sky(
                        world(centre + u * hu * inset(su, hu) + v * hv * inset(sv, hv)),
                        wn,
                    )
                } else {
                    1.0
                };
                col.push([l, l, l, open]);
            }
        }
        let row = us.len() as u32;
        for j in 0..vs.len() as u32 - 1 {
            for i in 0..row - 1 {
                let a = base + j * row + i;
                let (b, c, d) = (a + 1, a + row + 1, a + row);
                // Counter-clockwise seen from outside (as `metric_box`).
                idx.extend_from_slice(&[a, c, b, a, d, c]);
            }
        }
    }
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
    .with_inserted_indices(Indices::U32(idx));
    let _ = mesh.generate_tangents();
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trial's numbers (`-- --nocapture --ignored`): the sky view across
    /// a house's floor, and what baking it costs.
    #[test]
    #[ignore]
    fn sky_view_trial() {
        use crate::world::buildings;
        for id in [
            "house_plank",
            "farmhouse",
            "shop",
            "factory_hall",
            "house_plank_drowned",
        ] {
            let geo = buildings::prefab(id).unwrap();
            let f = extraction_core::map::prefab_footprint(id);
            let walls = Vec2::new(f[0], f[2]) * 0.5;
            let occ = buildings::occluders(&geo).within(walls);
            let y = geo.floor.unwrap_or(0.35) + 0.02;
            let mut row = String::new();
            for z in [-3.0f32, -1.5, 0.0, 1.5, 3.0] {
                for x in [-3.0f32, -1.5, 0.0, 1.5, 3.0] {
                    row += &format!("{:5.2}", occ.sky_view(Vec3::new(x, y, z), Vec3::Y));
                }
                row += "\n";
            }
            let n = geo.pieces.len();
            // (A fresh one: the field above is made already.)
            let occ = buildings::occluders(&geo).within(walls);
            let t = std::time::Instant::now();
            let mut verts = 0;
            for p in &geo.pieces {
                if let crate::world::buildings::Shape::Box(s) = p.shape {
                    verts += shaded_box(s, p.at, &occ, true).count_vertices();
                }
            }
            let ms = t.elapsed().as_secs_f32() * 1000.0;
            println!(
                "{id} ({n} pieces): bake {ms:.0} ms, {verts} vertices (sky view {})\n{row}",
                sky_view_enabled()
            );
        }
    }

    /// A floor next to a wall is darker at the wall's foot than in the
    /// open, and a wall's face away from everything is lit.
    #[test]
    fn corners_darken_and_the_open_does_not() {
        let floor = (
            Vec3::new(5.0, 0.05, 5.0),
            Transform::from_xyz(0.0, -0.05, 0.0),
        );
        let wall = (Vec3::new(0.1, 1.5, 5.0), Transform::from_xyz(0.0, 1.5, 0.0));
        let occ = Occluders::new([floor, wall]);
        let at_foot = occ.light(Vec3::new(0.15, 0.0, 0.0), Vec3::Y);
        let open = occ.light(Vec3::new(3.0, 0.0, 0.0), Vec3::Y);
        let wall_face = occ.light(Vec3::new(0.1, 2.5, 0.0), Vec3::X);
        assert!(at_foot < 0.8, "at the wall's foot: {at_foot}");
        assert!(open > 0.98, "in the open: {open}");
        assert!(wall_face > 0.98, "high on the wall: {wall_face}");
    }

    /// Rain reaches the floor outside and the roof, not the floor under
    /// the roof, nor the ground under the eaves: what's sheltered stays dry.
    #[test]
    fn under_a_roof_is_dry() {
        let floor = (
            Vec3::new(8.0, 0.05, 8.0),
            Transform::from_xyz(0.0, -0.05, 0.0),
        );
        // A 4 x 4 m roof 3 m up, overhanging a 3 x 3 m room.
        let roof = (Vec3::new(2.0, 0.1, 2.0), Transform::from_xyz(0.0, 3.0, 0.0));
        let occ = Occluders::new([floor, roof]);
        assert_eq!(
            occ.sky(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
            0.0,
            "under the roof"
        );
        assert_eq!(
            occ.sky(Vec3::new(1.8, 0.0, 0.5), Vec3::Y),
            0.0,
            "under the eaves"
        );
        assert_eq!(
            occ.sky(Vec3::new(3.0, 0.0, 0.0), Vec3::Y),
            1.0,
            "in the open"
        );
        assert_eq!(
            occ.sky(Vec3::new(0.0, 3.1, 0.0), Vec3::Y),
            1.0,
            "on the roof"
        );
        // The building's boxes carry it in their colours' alpha (a box
        // under the roof, all sheltered), glass keeps its own alpha, and
        // other shapes' bake carries it too.
        let alphas = |m: &Mesh| -> Vec<f32> {
            let Some(VertexAttributeValues::Float32x4(c)) = m.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("no colours");
            };
            c.iter().map(|c| c[3]).collect()
        };
        let under = Transform::from_xyz(0.0, 0.1, 0.0);
        let boxed = shaded_box(Vec3::new(1.0, 0.2, 1.0), under, &occ, true);
        assert!(
            alphas(&boxed).iter().all(|a| *a == 0.0),
            "a box under the roof"
        );
        let glass = shaded_box(Vec3::new(1.0, 0.2, 1.0), under, &occ, false);
        assert!(
            alphas(&glass).iter().all(|a| *a == 1.0),
            "glass keeps its alpha"
        );
        let outside = shaded_box(
            Vec3::new(1.0, 0.2, 1.0),
            Transform::from_xyz(3.0, 0.1, 0.0),
            &occ,
            true,
        );
        assert!(
            alphas(&outside).iter().all(|a| *a == 1.0),
            "a box in the open"
        );
        let mut m = Cuboid::new(1.0, 0.2, 1.0)
            .mesh()
            .build()
            .transformed_by(under);
        occ.bake(&mut m);
        assert!(
            alphas(&m).iter().all(|a| *a == 0.0),
            "a baked shape under the roof"
        );
    }

    /// The cut box's faces face out (the same winding as `metric_box`).
    #[test]
    fn cut_boxes_face_out() {
        let occ = Occluders::new([(Vec3::new(1.0, 0.5, 1.5), Transform::IDENTITY)]);
        let m = shaded_box(
            Vec3::new(2.0, 1.0, 3.0),
            Transform::from_xyz(0.0, 0.0, 9.0),
            &occ,
            true,
        );
        let Some(VertexAttributeValues::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!()
        };
        let Some(VertexAttributeValues::Float32x3(n)) = m.attribute(Mesh::ATTRIBUTE_NORMAL) else {
            panic!()
        };
        let Some(Indices::U32(idx)) = m.indices() else {
            panic!()
        };
        for t in idx.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(p[i as usize]));
            let face = (b - a).cross(c - a);
            assert!(face.dot(Vec3::from(n[t[0] as usize])) > 0.0);
        }
    }
}
