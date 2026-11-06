//! A part shot up looks it, on every machine: as its strength falls into
//! a worse band (`damage::Band`), what's drawn of it wears that band.
//!
//! - **Hurt**: its paint dulled and darkened, rougher, its glow down.
//! - **Failing**: darker still, sooty; an armour plate knocked askew on
//!   its mounts (a dent's worth, [`ASKEW`]); a lens crazed with cracks
//!   where it was last hit.
//!
//! What's lit (an eye, a lamp, a vent's glow) is left to what animates it
//! (`visuals`, each machine's look), and a round's own mark
//! (`fx::shell_marks`) stays a mark. Mended (`mender`), it's put back as
//! it was. The look of each band is drawn from the piece's own material
//! ([`worn`]), so a heavy's grey shell, a drone's pale one and a
//! dirigible's fabric each go their own way. `hit_gauge` holds every part
//! of every machine to it.

use std::collections::HashMap;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::damage::{Band, RobotStruck};
use super::{PartRole, RobotPart};

/// A plate's tilt on its mounts once failing (radians, ~5°).
pub const ASKEW: f32 = 0.09;
/// A loose piece this near (m) a part with nothing of its own drawn is
/// that part's too.
const TOUCHING: f32 = 0.15;
/// A material glowing more than this is lit, and left to what lights it.
const LIT: f32 = 0.5;

pub struct WearPlugin;

impl Plugin for WearPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WornMaterials>()
            .add_systems(Update, (last_hits, wear).chain());
    }
}

/// A piece drawn for a part (or a machine) away from it: a leg's
/// segments posed on their own, a heavy's limbs. On a part, the piece is
/// that part's; on a machine, it's whichever part's hit volume it's
/// nearest, as are the machine's own pieces not drawn under a part (a
/// heavy's hull, a sentry's housing).
#[derive(Component, Clone, Copy, Debug)]
pub struct DrawnFor(pub Entity);

/// How worn a part looks, and what it looked like before.
#[derive(Component, Default)]
pub struct Worn {
    pub band: u8,
    was: Vec<(Entity, Handle<StandardMaterial>, Transform)>,
    cracks: Vec<Entity>,
}

/// Where a part was last struck (local to it) and from which side.
#[derive(Component, Clone, Copy)]
struct LastHit {
    at: Vec3,
    normal: Vec3,
}

/// The worn copy of each material for each band.
#[derive(Resource, Default)]
struct WornMaterials(HashMap<(AssetId<StandardMaterial>, u8), Handle<StandardMaterial>>);

/// A band's number: 0 whole, 1 hurt, 2 failing (and gone: what's left of
/// a dead part is failing to look at).
pub fn band_of(part: &RobotPart) -> u8 {
    match Band::of(part.hp / part.max_hp.max(1e-3)) {
        Band::Whole => 0,
        Band::Hurt => 1,
        Band::Failing | Band::Gone => 2,
    }
}

/// `m` worn to band `band`: darker toward soot, rougher, less metal, its
/// glow down.
pub fn worn(m: &StandardMaterial, band: u8) -> StandardMaterial {
    let (keep, rough, glow) = match band {
        0 => return m.clone(),
        1 => (0.68, 0.2, 0.55),
        _ => (0.42, 0.35, 0.25),
    };
    let soot = LinearRgba::rgb(0.035, 0.03, 0.025);
    let c = m.base_color.to_linear();
    let mix = |a: f32, s: f32| a * keep + s * (1.0 - keep);
    let mut w = m.clone();
    w.base_color = Color::LinearRgba(LinearRgba::new(
        mix(c.red, soot.red),
        mix(c.green, soot.green),
        mix(c.blue, soot.blue),
        c.alpha,
    ));
    w.perceptual_roughness = (m.perceptual_roughness + rough).min(1.0);
    w.metallic = m.metallic * (1.0 - rough);
    w.emissive = m.emissive * glow;
    w
}

fn lit(m: &StandardMaterial) -> bool {
    let e = m.emissive;
    m.unlit || e.red.max(e.green).max(e.blue) > LIT
}

fn last_hits(
    mut commands: Commands,
    mut struck: MessageReader<RobotStruck>,
    parts: Query<&GlobalTransform, With<RobotPart>>,
) {
    for s in struck.read() {
        let Ok(gt) = parts.get(s.part) else {
            continue;
        };
        let inv = gt.affine().inverse();
        let n = s.normal.normalize_or(-s.dir);
        commands.entity(s.part).try_insert(LastHit {
            at: inv.transform_point3(s.at),
            normal: (gt.rotation().inverse() * n).normalize_or(Vec3::Z),
        });
    }
}

/// What's drawn of `part`: the meshes under it, short of another part's
/// and of a round's marks.
fn drawn(
    part: Entity,
    children: &Query<&Children>,
    is_part: &Query<(), With<RobotPart>>,
    mats: &Query<&MeshMaterial3d<StandardMaterial>, With<Mesh3d>>,
    marks: &[AssetId<StandardMaterial>],
    skip: &[Entity],
) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack = vec![part];
    while let Some(e) = stack.pop() {
        if e != part && (is_part.contains(e) || skip.contains(&e)) {
            continue;
        }
        if let Ok(m) = mats.get(e) {
            if !marks.contains(&m.id()) {
                out.push(e);
            }
        }
        if let Ok(c) = children.get(e) {
            stack.extend(c.iter());
        }
    }
    out
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn wear(
    mut commands: Commands,
    mut parts: Query<
        (Entity, &RobotPart, Option<&mut Worn>, Option<&LastHit>),
        Or<(Changed<RobotPart>, Without<Worn>)>,
    >,
    children: Query<&Children>,
    is_part: Query<(), With<RobotPart>>,
    mats: Query<&MeshMaterial3d<StandardMaterial>, With<Mesh3d>>,
    mut transforms: Query<&mut Transform>,
    (loose, volumes, places): (
        Query<(Entity, &DrawnFor)>,
        Query<(Entity, &RobotPart, &avian3d::prelude::ColliderAabb)>,
        Query<(&GlobalTransform, Option<&bevy::camera::primitives::Aabb>)>,
    ),
    fx: Option<Res<crate::weapons::impacts::FxAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut made: ResMut<WornMaterials>,
    mut crack_look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    let marks: Vec<_> = fx
        .as_ref()
        .map(|a| vec![a.bright_pock.id(), a.hole.id()])
        .unwrap_or_default();
    for (e, part, worn_now, last) in &mut parts {
        let band = band_of(part);
        // (First seen: worn from its next change.)
        let Some(mut w) = worn_now else {
            commands.entity(e).try_insert(Worn::default());
            continue;
        };
        if band == w.band {
            continue;
        }
        // Put back as it was, then wear it to its band.
        for (piece, mat, t) in w.was.drain(..) {
            if let Ok(mut ec) = commands.get_entity(piece) {
                ec.try_insert(MeshMaterial3d(mat));
            }
            if let Ok(mut tf) = transforms.get_mut(piece) {
                *tf = t;
            }
        }
        for c in w.cracks.drain(..) {
            if let Ok(mut ec) = commands.get_entity(c) {
                ec.despawn();
            }
        }
        w.band = band;
        if band == 0 {
            continue;
        }
        let mut pieces = drawn(e, &children, &is_part, &mats, &marks, &[]);
        // Drawn for it elsewhere, and the machine's loose pieces nearest it.
        let robot = part.robot;
        let mut near_roots = vec![robot];
        for (root, d) in &loose {
            if d.0 == e {
                pieces.extend(drawn(root, &children, &is_part, &mats, &marks, &[]));
            } else if d.0 == robot {
                near_roots.push(root);
            }
        }
        let mine: Vec<_> = volumes
            .iter()
            .filter(|(_, p, _)| p.robot == robot)
            .collect();
        let reach =
            |at: Vec3, a: &avian3d::prelude::ColliderAabb| at.clamp(a.min, a.max).distance(at);
        // (Each loose piece and how far it is from this part's volume.)
        let mut others = Vec::new();
        let own_volume = mine.iter().find(|(p, ..)| *p == e).map(|(_, _, a)| *a);
        for root in near_roots {
            for piece in drawn(root, &children, &is_part, &mats, &marks, &[]) {
                let Ok((gt, aabb)) = places.get(piece) else {
                    continue;
                };
                let at = gt.transform_point(aabb.map_or(Vec3::ZERO, |a| Vec3::from(a.center)));
                let nearest = mine
                    .iter()
                    .min_by(|a, b| reach(at, a.2).total_cmp(&reach(at, b.2)))
                    .map(|(p, ..)| *p);
                if nearest == Some(e) {
                    pieces.push(piece);
                } else if let Some(v) = own_volume {
                    others.push((piece, reach(at, v)));
                }
            }
        }
        // A part with nothing drawn of its own (a plate in front of a
        // housing that's nearer the core): what touches its volume, or the
        // nearest piece to it.
        if pieces.is_empty() {
            let touching: Vec<Entity> = others
                .iter()
                .filter(|(_, d)| *d <= TOUCHING)
                .map(|(p, _)| *p)
                .collect();
            if touching.is_empty() {
                pieces.extend(
                    others
                        .iter()
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(p, _)| *p),
                );
            } else {
                pieces = touching;
            }
        }
        pieces.sort();
        pieces.dedup();
        for piece in pieces {
            let Ok(m) = mats.get(piece) else {
                continue;
            };
            let Some(orig) = materials.get(&m.0).cloned() else {
                continue;
            };
            if lit(&orig) {
                continue;
            }
            let key = (m.0.id(), band);
            let handle = match made.0.get(&key) {
                Some(h) => h.clone(),
                None => {
                    let h = materials.add(worn(&orig, band));
                    made.0.insert(key, h.clone());
                    h
                }
            };
            let t = transforms.get(piece).copied().unwrap_or_default();
            w.was.push((piece, m.0.clone(), t));
            commands.entity(piece).try_insert(MeshMaterial3d(handle));
        }
        if band < 2 {
            continue;
        }
        match part.role {
            // Knocked askew on its mounts: each piece of the plate turned
            // a few degrees about its own level axis, the way it was hit.
            PartRole::Armour(i) => {
                let tilt = ASKEW * if i % 2 == 0 { 1.0 } else { -0.8 };
                let axis = last
                    .map(|l| l.normal.cross(Vec3::Y).normalize_or(Vec3::X))
                    .unwrap_or(Vec3::X);
                if let Ok(c) = children.get(e) {
                    for &piece in c {
                        if is_part.contains(piece) {
                            continue;
                        }
                        if let Ok(mut tf) = transforms.get_mut(piece) {
                            if !w.was.iter().any(|(p, ..)| *p == piece) {
                                let mat = mats.get(piece).map(|m| m.0.clone()).unwrap_or_default();
                                w.was.push((piece, mat, *tf));
                            }
                            tf.rotate_around(Vec3::ZERO, Quat::from_axis_angle(axis, tilt));
                        }
                    }
                }
            }
            // The lens crazed where it was hit: dark cracks across it.
            PartRole::Sensor => {
                let (mesh, mat) = crack_look
                    .get_or_insert_with(|| {
                        (
                            meshes.add(Cuboid::new(1.0, 0.1, 0.02)),
                            materials.add(StandardMaterial {
                                base_color: Color::srgb(0.02, 0.02, 0.025),
                                perceptual_roughness: 0.3,
                                ..default()
                            }),
                        )
                    })
                    .clone();
                let (at, n) = last.map_or((Vec3::ZERO, Vec3::Z), |l| (l.at, l.normal));
                let face = Quat::from_rotation_arc(Vec3::Z, n);
                for k in 0..4 {
                    let turn = Quat::from_rotation_z(k as f32 * 0.83 + 0.3);
                    let long = 0.05 + 0.025 * (k % 3) as f32;
                    let c = commands
                        .spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(mat.clone()),
                            Transform::from_translation(at + n * 0.004)
                                .with_rotation(face * turn)
                                .with_scale(Vec3::new(long, long, 1.0)),
                            NotShadowCaster,
                            ChildOf(e),
                        ))
                        .id();
                    w.cracks.push(c);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each band reads worse than the last: darker, rougher, its glow down.
    #[test]
    fn each_band_wears_worse() {
        let m = StandardMaterial {
            base_color: Color::srgb(0.6, 0.62, 0.6),
            perceptual_roughness: 0.4,
            metallic: 0.6,
            emissive: LinearRgba::rgb(0.2, 0.2, 0.2),
            ..default()
        };
        let lum = |m: &StandardMaterial| {
            let c = m.base_color.to_linear();
            c.red + c.green + c.blue
        };
        let (a, b) = (worn(&m, 1), worn(&m, 2));
        assert!(lum(&m) > lum(&a) && lum(&a) > lum(&b));
        assert!(m.perceptual_roughness < a.perceptual_roughness);
        assert!(a.perceptual_roughness < b.perceptual_roughness);
        assert!(m.emissive.red > a.emissive.red && a.emissive.red > b.emissive.red);
        assert_eq!(worn(&m, 0).base_color, m.base_color);
    }

    /// A band of a part's strength: a graze is whole, one wound hurt,
    /// near the end failing, and gone looks failing.
    #[test]
    fn bands_by_strength() {
        let p = |hp: f32| RobotPart {
            robot: Entity::PLACEHOLDER,
            role: PartRole::Core,
            hp,
            max_hp: 100.0,
        };
        assert_eq!(band_of(&p(95.0)), 0);
        assert_eq!(band_of(&p(60.0)), 1);
        assert_eq!(band_of(&p(20.0)), 2);
        assert_eq!(band_of(&p(0.0)), 2);
    }

    /// Lit pieces (an eye, a lamp) are left to what lights them.
    #[test]
    fn what_glows_is_left_alone() {
        let eye = StandardMaterial {
            emissive: LinearRgba::rgb(3.0, 0.2, 0.1),
            ..default()
        };
        assert!(lit(&eye));
        assert!(!lit(&StandardMaterial::default()));
    }
}
