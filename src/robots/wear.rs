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

use std::collections::{HashMap, HashSet};

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
            .add_systems(Update, ((last_hits, wear).chain(), gutter));
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

/// A part still going but failing: what sends up a machine's soot
/// (`fx::sputter`), gutters its lights ([`gutter`]) and may set off its
/// flare. One test for all three, so they can't drift apart.
pub fn failing(part: &RobotPart) -> bool {
    part.alive() && Band::of(part.hp / part.max_hp.max(1e-3)) == Band::Failing
}

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
        let middle = |at: Vec3, a: &avian3d::prelude::ColliderAabb| a.center().distance(at);
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
                    // (Inside two volumes, a hub inside the body's: the
                    // one whose middle it's nearest.)
                    .min_by(|a, b| {
                        reach(at, a.2)
                            .total_cmp(&reach(at, b.2))
                            .then(middle(at, a.2).total_cmp(&middle(at, b.2)))
                    })
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

/// A machine with a part failing gutters: what's lit of it (its eye, its
/// lamps) drops out for a moment now and then, irregularly, and comes back
/// no brighter than it was. It never brightens or pulses, as a machine's
/// warning does (a lock, an aim, a chirp): it reads as broken, not as
/// charging. For the machines that dress themselves (`heavy::OwnLook`) and
/// the sorter; the rest gutter a hurt eye in `visuals`. Each lit piece
/// swaps to a dark copy of its own material (shared a material, as
/// [`worn`]'s are) and back, so a machine beside it with the same
/// material isn't touched.
#[derive(Component)]
struct Guttered {
    robot: Entity,
    lit: Handle<StandardMaterial>,
}

/// How long a gutter's drop-outs and the light between them last (s).
const OUT: (f32, f32) = (0.06, 0.22);
const BETWEEN: (f32, f32) = (0.15, 1.1);
/// A dropped-out light's glow, of its own.
const DIM: f32 = 0.06;

/// `EXTRACTION_GUTTER=out`, read once.
struct HeldOut(bool);

impl Default for HeldOut {
    fn default() -> Self {
        Self(std::env::var("EXTRACTION_GUTTER").is_ok_and(|v| v == "out"))
    }
}

#[derive(Default)]
struct GutterState {
    out: bool,
    left: f32,
    seed: u64,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn gutter(
    mut commands: Commands,
    parts: Query<&RobotPart>,
    robots: Query<
        (Entity, &super::Robot),
        Or<(With<super::heavy::OwnLook>, With<super::sorter::Sorter>)>,
    >,
    children: Query<&Children>,
    mats: Query<&MeshMaterial3d<StandardMaterial>, With<Mesh3d>>,
    guttered: Query<(Entity, &Guttered)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: ResMut<WornMaterials>,
    mut states: Local<HashMap<Entity, GutterState>>,
    held: Local<HeldOut>,
    time: Res<Time>,
) {
    let held = &held.0;
    let dt = time.delta_secs();
    let failing: HashSet<Entity> = parts
        .iter()
        .filter(|p| failing(p))
        .map(|p| p.robot)
        .collect();
    // Back as it was, where a machine's mended or wrecked (a wreck's
    // lights go out its own way).
    let live = |r: Entity| robots.get(r).is_ok_and(|(_, rb)| !rb.wrecked) && failing.contains(&r);
    states.retain(|r, _| live(*r));
    for (robot, _) in &robots {
        if !live(robot) {
            continue;
        }
        let st = states.entry(robot).or_insert_with(|| GutterState {
            seed: robot.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15),
            left: 0.4,
            ..default()
        });
        st.left -= dt;
        if st.left > 0.0 {
            continue;
        }
        // `EXTRACTION_GUTTER=out`: held out, for a still of how a light
        // that's dropped out looks (a still catches one about a frame in
        // five).
        st.out = !st.out || *held;
        st.seed = st
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let k = (st.seed >> 40) as f32 / (1u64 << 24) as f32;
        let (lo, hi) = if st.out { OUT } else { BETWEEN };
        st.left = lo + (hi - lo) * k;
        // Its lit pieces, under it.
        let mut stack = vec![robot];
        while let Some(e) = stack.pop() {
            if let Ok(c) = children.get(e) {
                stack.extend(c.iter());
            }
            let Ok(m) = mats.get(e) else {
                continue;
            };
            if st.out {
                let Some(orig) = materials.get(&m.0).cloned() else {
                    continue;
                };
                if !lit(&orig) || guttered.contains(e) {
                    continue;
                }
                let key = (m.0.id(), GUTTER_BAND);
                let dark = match made.0.get(&key) {
                    Some(h) => h.clone(),
                    None => {
                        let mut d = orig.clone();
                        d.emissive = orig.emissive * DIM;
                        let h = materials.add(d);
                        made.0.insert(key, h.clone());
                        h
                    }
                };
                commands.entity(e).try_insert((
                    Guttered {
                        robot,
                        lit: m.0.clone(),
                    },
                    MeshMaterial3d(dark),
                ));
            } else if let Ok((_, g)) = guttered.get(e) {
                commands
                    .entity(e)
                    .try_insert(MeshMaterial3d(g.lit.clone()))
                    .remove::<Guttered>();
            }
        }
    }
    // A machine no longer failing (mended, or wrecked: a wreck's lights
    // go out its own way) gets its lights back.
    for (e, g) in &guttered {
        if !live(g.robot) {
            commands
                .entity(e)
                .try_insert(MeshMaterial3d(g.lit.clone()))
                .remove::<Guttered>();
        }
    }
}

/// [`WornMaterials`]' key for a guttered light's dark copy.
const GUTTER_BAND: u8 = 9;

#[cfg(test)]
mod tests {
    use super::*;

    /// Pieces that share a material share its worn copy: one material a
    /// band, never one a mesh (a copy each would put every mesh in a draw
    /// group of its own, and break the merges that count on them, as a
    /// gun's weathering once did).
    #[test]
    fn worn_pieces_share_their_material_copies() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<StandardMaterial>()
            .init_asset::<Mesh>()
            .add_message::<RobotStruck>()
            .add_plugins(WearPlugin);
        let (mat, other, mesh) = {
            let w = app.world_mut();
            let mut mats = w.resource_mut::<Assets<StandardMaterial>>();
            let mat = mats.add(StandardMaterial::default());
            let other = mats.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.3, 0.4),
                ..default()
            });
            let mesh = w.resource_mut::<Assets<Mesh>>().add(Cuboid::default());
            (mat, other, mesh)
        };
        let robot = app.world_mut().spawn_empty().id();
        let mut pieces = Vec::new();
        let mut parts = Vec::new();
        for _ in 0..3 {
            let part = app
                .world_mut()
                .spawn((
                    RobotPart {
                        robot,
                        role: PartRole::Armour(0),
                        hp: 100.0,
                        max_hp: 100.0,
                    },
                    Transform::default(),
                ))
                .id();
            for m in [&mat, &mat, &other] {
                pieces.push((
                    app.world_mut()
                        .spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(m.clone()),
                            Transform::default(),
                            ChildOf(part),
                        ))
                        .id(),
                    m.id(),
                ));
            }
            parts.push(part);
        }
        app.update();
        for p in &parts {
            app.world_mut().get_mut::<RobotPart>(*p).unwrap().hp = 60.0;
        }
        app.update();
        app.update();
        let now = |app: &App, e: Entity| {
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(e)
                .unwrap()
                .0
                .id()
        };
        let mut worn: HashMap<AssetId<StandardMaterial>, HashSet<AssetId<StandardMaterial>>> =
            HashMap::new();
        for (e, was) in &pieces {
            let m = now(&app, *e);
            assert_ne!(m, *was, "a hurt part's piece wears");
            worn.entry(*was).or_default().insert(m);
        }
        for (was, copies) in &worn {
            assert_eq!(copies.len(), 1, "{was:?} worn into {} copies", copies.len());
        }
    }

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
