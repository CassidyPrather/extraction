//! Weathering by valley: robots that work Cinder Works come in sooted,
//! their dark metal heat-blued; Marrow Creek's are streaked with rust and
//! furred with moss on their tops; Kettle Pass's carry a little dust. It's
//! painted once into the robot meshes' vertex colours (which the shell and
//! metal materials multiply), so it costs no draws and no materials: every
//! robot of a kind shares its meshes, and a raid is in one valley.

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;

/// What a valley does to a robot's surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Valley {
    /// Soot, and heat-blued metal (Cinder Works).
    Cinder,
    /// Rust streaks and moss (Marrow Creek).
    Marrow,
    /// A little dust (Kettle Pass).
    Kettle,
    /// Clean (the training yard, anywhere else).
    None,
}

impl Valley {
    /// How much of the shell's own faint glow survives the grime (the glow
    /// isn't touched by vertex colour, so it's dimmed to match).
    pub fn shell_glow(self) -> f32 {
        match self {
            Valley::Cinder => 0.45,
            Valley::Marrow => 0.75,
            Valley::Kettle => 0.9,
            Valley::None => 1.0,
        }
    }

    pub fn of_map(name: &str) -> Self {
        let n = name.to_ascii_lowercase();
        if n.contains("cinder") {
            Valley::Cinder
        } else if n.contains("marrow") {
            Valley::Marrow
        } else if n.contains("kettle") {
            Valley::Kettle
        } else {
            Valley::None
        }
    }
}

/// A robot mesh to weather, and which of its materials it wears (the
/// shell takes grime and moss; dark metal takes heat-blueing and rust).
#[derive(Component, Clone, Copy)]
pub(crate) struct Weather {
    pub shell: bool,
}

/// A small hash noise in [0, 1).
fn noise(p: Vec3) -> f32 {
    let k = (p.dot(Vec3::new(12.9898, 78.233, 37.719))).sin() * 43_758.547;
    k - k.floor()
}

/// Smooth value noise over a lattice of `cell` metres.
fn patches(p: Vec3, cell: f32) -> f32 {
    let q = p / cell;
    let (i, f) = (q.floor(), q - q.floor());
    let s = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let n = |dx: f32, dy: f32, dz: f32| noise(i + Vec3::new(dx, dy, dz));
    let x00 = n(0., 0., 0.) + (n(1., 0., 0.) - n(0., 0., 0.)) * s.x;
    let x10 = n(0., 1., 0.) + (n(1., 1., 0.) - n(0., 1., 0.)) * s.x;
    let x01 = n(0., 0., 1.) + (n(1., 0., 1.) - n(0., 0., 1.)) * s.x;
    let x11 = n(0., 1., 1.) + (n(1., 1., 1.) - n(0., 1., 1.)) * s.x;
    let y0 = x00 + (x10 - x00) * s.y;
    let y1 = x01 + (x11 - x01) * s.y;
    y0 + (y1 - y0) * s.z
}

/// The vertex colour for a point on a robot's surface (mesh space, metres)
/// with its normal, in `valley`, on shell or on dark metal.
pub fn grime(valley: Valley, p: Vec3, n: Vec3, shell: bool) -> [f32; 4] {
    let n = n.normalize_or(Vec3::Y);
    // Grime gathers underneath and runs down the sides in streaks.
    let under = (-n.y).clamp(0.0, 1.0);
    let side = 1.0 - n.y.abs();
    let streak = {
        let s = ((p.x * 31.0 + p.z * 23.0).sin() * 0.5 + 0.5).powi(6);
        s * side * (0.6 + 0.4 * patches(p, 0.3))
    };
    let top = n.y.clamp(0.0, 1.0);
    let blotch = patches(p, 0.18);
    let mix = |a: Vec3, b: Vec3, t: f32| a + (b - a) * t.clamp(0.0, 1.0);
    let white = Vec3::ONE;
    let c = match (valley, shell) {
        (Valley::None, _) => white,
        (Valley::Cinder, true) => {
            let soot = Vec3::new(0.34, 0.32, 0.31);
            mix(
                white,
                soot,
                0.4 + 0.45 * under + 0.6 * streak + 0.3 * blotch,
            )
        }
        (Valley::Cinder, false) => {
            // Heat-blued: a blue-violet cast, patchy.
            let blue = Vec3::new(0.62, 0.66, 0.95);
            mix(white, blue, 0.35 + 0.5 * blotch)
        }
        (Valley::Marrow, true) => {
            let moss = Vec3::new(0.55, 0.68, 0.38);
            let rust = Vec3::new(0.8, 0.52, 0.34);
            let c = mix(white, moss, top * (blotch - 0.25).max(0.0) * 2.0);
            mix(c, rust, streak * 1.2 + 0.25 * under + 0.1)
        }
        (Valley::Marrow, false) => {
            let rust = Vec3::new(0.85, 0.5, 0.3);
            mix(white, rust, 0.15 + 0.5 * blotch * (0.4 + 0.6 * side))
        }
        (Valley::Kettle, true) => {
            let dust = Vec3::new(0.93, 0.9, 0.84);
            mix(white, dust, 0.3 + 0.6 * top * blotch)
        }
        (Valley::Kettle, false) => mix(white, Vec3::new(0.9, 0.88, 0.84), 0.3),
    };
    [c.x, c.y, c.z, 1.0]
}

/// Paints each robot mesh once, for this raid's valley.
pub(crate) fn weather_meshes(
    marked: Query<(&Mesh3d, &Weather), Added<Weather>>,
    zone: Option<Res<crate::world::Zone>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut done: Local<std::collections::HashSet<AssetId<Mesh>>>,
) {
    let Some(zone) = zone else {
        return;
    };
    let valley = Valley::of_map(&zone.map.name);
    if valley == Valley::None {
        return;
    }
    for (mesh, w) in &marked {
        if !done.insert(mesh.0.id()) {
            continue;
        }
        let Some(mut m) = meshes.get_mut(&mesh.0) else {
            continue;
        };
        let (
            Some(VertexAttributeValues::Float32x3(pos)),
            Some(VertexAttributeValues::Float32x3(nrm)),
        ) = (
            m.attribute(Mesh::ATTRIBUTE_POSITION),
            m.attribute(Mesh::ATTRIBUTE_NORMAL),
        )
        else {
            continue;
        };
        let col: Vec<[f32; 4]> = pos
            .iter()
            .zip(nrm.iter())
            .map(|(p, n)| grime(valley, Vec3::from(*p), Vec3::from(*n), w.shell))
            .collect();
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(valley: Valley, shell: bool) -> Vec<[f32; 4]> {
        let mut out = Vec::new();
        for i in 0..400 {
            let a = i as f32 * 0.37;
            let p = Vec3::new(a.cos() * 0.4, (i as f32 * 0.013).sin() * 0.4, a.sin() * 0.4);
            out.push(grime(valley, p, p, shell));
        }
        out
    }

    fn mean(c: &[[f32; 4]]) -> Vec3 {
        c.iter().map(|c| Vec3::new(c[0], c[1], c[2])).sum::<Vec3>() / c.len() as f32
    }

    /// Each valley marks its robots its own way, within colour's range; the
    /// clean ones are untouched.
    #[test]
    fn valleys_weather_their_own_way() {
        for v in [Valley::Cinder, Valley::Marrow, Valley::Kettle, Valley::None] {
            for shell in [true, false] {
                for c in sample(v, shell) {
                    assert!(c.iter().all(|x| (0.0..=1.0).contains(x) && x.is_finite()));
                }
            }
        }
        assert_eq!(mean(&sample(Valley::None, true)), Vec3::ONE);
        let soot = mean(&sample(Valley::Cinder, true));
        assert!(soot.length() < Vec3::ONE.length() * 0.85, "sooted: {soot}");
        let blue = mean(&sample(Valley::Cinder, false));
        assert!(blue.z > blue.x + 0.1, "heat-blued: {blue}");
        let marrow = mean(&sample(Valley::Marrow, false));
        assert!(marrow.x > marrow.z + 0.1, "rusted: {marrow}");
        assert_eq!(Valley::of_map("Cinder Works"), Valley::Cinder);
        assert_eq!(Valley::of_map("Marrow Creek"), Valley::Marrow);
        assert_eq!(Valley::of_map("Training Yard"), Valley::None);
    }
}
