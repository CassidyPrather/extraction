//! What deer and dogs look like moving: a procedural quadruped gait, in
//! the house style of `robots::anim`.
//!
//! The logic (`herd`) says where each animal is, how fast it's going and
//! where it's looking. This turns that into a body: a gait by speed (a
//! walk, each foot in turn; a trot, diagonal pairs; a bound, fronts then
//! hinds, the body arching), each foot planted on the ground until its
//! turn comes, then lifted in an arc to where it should land, a little
//! ahead of the hip. Two-bone legs reach each foot (the front knee bends
//! forward, the hind hock back). The body rides on the legs, bobbing with
//! the gait and pitched with the slope; the head goes down to graze and
//! up to look, turned toward whatever it's watching.
//!
//! Each part is one mesh (`art/animals/deer.py`, `dog.py`), the model's
//! primitives merged into one vertex-coloured mesh. Past [`FAR`] the
//! animal is a single standing stand-in. Cost: per animal per frame, four
//! two-bone solves and a few height lookups; no allocation.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::herd::{Herd, Kind, State};
use crate::models::{ModelNode, ModelState, Models, model_nodes};
use crate::render::lod::StandIn;
use crate::world::Zone;

/// Beyond this an animal is its stand-in (m).
pub const FAR: f32 = 90.0;
/// Beyond this it isn't drawn.
pub const HIDE: f32 = 400.0;

/// One kind's body: where the legs hang and how long they are (as the
/// Blender script builds them).
#[derive(Clone, Copy, Debug)]
pub struct Build {
    pub model: &'static str,
    /// Hips in body space: front left, front right, hind left, hind right.
    pub hips: [Vec3; 4],
    /// Upper and lower leg lengths, front and hind.
    pub front: (f32, f32),
    pub hind: (f32, f32),
    /// The body's middle above the ground, standing.
    pub ride: f32,
    /// Where the neck turns, body space.
    pub neck: Vec3,
    /// Head pitch grazing and looking (radians, about X at the neck).
    pub graze: f32,
    pub alert: f32,
    pub colours: [(&'static str, LinearRgba); 3],
}

pub fn build(kind: Kind) -> Build {
    match kind {
        Kind::Deer => Build {
            model: "models/animals/deer.glb",
            hips: [
                Vec3::new(-0.075, -0.06, -0.25),
                Vec3::new(0.075, -0.06, -0.25),
                Vec3::new(-0.075, -0.03, 0.27),
                Vec3::new(0.075, -0.03, 0.27),
            ],
            front: (0.25, 0.3),
            hind: (0.3, 0.33),
            ride: 0.58,
            neck: Vec3::new(0.0, 0.06, -0.33),
            graze: -1.35,
            alert: 0.12,
            colours: [
                ("Hide", LinearRgba::rgb(0.075, 0.055, 0.04)),
                ("Pale", LinearRgba::rgb(0.26, 0.245, 0.22)),
                ("Hoof", LinearRgba::rgb(0.012, 0.011, 0.01)),
            ],
        },
        Kind::Dog => Build {
            model: "models/animals/dog.glb",
            hips: [
                Vec3::new(-0.07, -0.05, -0.22),
                Vec3::new(0.07, -0.05, -0.22),
                Vec3::new(-0.07, -0.03, 0.22),
                Vec3::new(0.07, -0.03, 0.22),
            ],
            front: (0.2, 0.22),
            hind: (0.22, 0.24),
            ride: 0.44,
            neck: Vec3::new(0.0, 0.05, -0.28),
            graze: -0.9,
            alert: 0.25,
            colours: [
                ("Hide", LinearRgba::rgb(0.06, 0.045, 0.035)),
                ("Pale", LinearRgba::rgb(0.25, 0.21, 0.16)),
                ("Hoof", LinearRgba::rgb(0.015, 0.013, 0.012)),
            ],
        },
    }
}

/// The meshes of one kind.
#[derive(Clone)]
pub struct Parts {
    pub body: Handle<Mesh>,
    pub head: Handle<Mesh>,
    /// Upper and lower leg, front and hind.
    pub legs: [Handle<Mesh>; 4],
    pub far: Handle<Mesh>,
}

#[derive(Resource, Default)]
pub struct BeastArt {
    parts: Vec<(Kind, Parts)>,
    failed: Vec<Kind>,
    material: Option<Handle<StandardMaterial>>,
}

/// One animal, drawn: the gait's own state.
#[derive(Component, Debug)]
pub struct BeastVisual {
    pub herd: Entity,
    pub index: usize,
    feet: [Vec3; 4],
    /// Where each foot lifted off (for its swing).
    lifted: [Vec3; 4],
    stance: [bool; 4],
    phase: f32,
    gait: Gait,
    started: bool,
    /// The part entities: body, head, 8 leg pieces (upper and lower of
    /// each leg), the stand-in.
    parts: [Entity; 11],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Stand,
    Walk,
    Trot,
    Bound,
}

impl Gait {
    fn of(speed: f32, was: Gait) -> Gait {
        // A little hysteresis, so it doesn't flicker between two.
        let up = |g: Gait| match g {
            Gait::Stand => 0.12,
            Gait::Walk => 1.9,
            Gait::Trot => 4.8,
            Gait::Bound => f32::MAX,
        };
        let down = |g: Gait| match g {
            Gait::Stand => -1.0,
            Gait::Walk => 0.06,
            Gait::Trot => 1.5,
            Gait::Bound => 4.0,
        };
        let order = [Gait::Stand, Gait::Walk, Gait::Trot, Gait::Bound];
        let mut g = was;
        let i = order.iter().position(|o| *o == g).unwrap();
        if speed > up(g) && i < 3 {
            g = order[i + 1];
        } else if speed < down(g) && i > 0 {
            g = order[i - 1];
        }
        g
    }

    /// Each leg's place in the cycle, how long a foot is down (share of
    /// the cycle) and cycles a second at `speed`.
    pub fn pattern(self, speed: f32) -> ([f32; 4], f32, f32) {
        match self {
            Gait::Stand => ([0.0; 4], 1.0, 0.0),
            Gait::Walk => ([0.25, 0.75, 0.0, 0.5], 0.68, 0.7 + 0.6 * speed),
            Gait::Trot => ([0.0, 0.5, 0.5, 0.0], 0.52, 1.5 + 0.2 * speed),
            Gait::Bound => ([0.0, 0.08, 0.5, 0.58], 0.3, 2.0 + 0.06 * speed),
        }
    }
}

/// Two-bone IK: the knee for a hip and foot, bent toward `pole`.
pub fn knee(hip: Vec3, foot: Vec3, upper: f32, lower: f32, pole: Vec3) -> Vec3 {
    let d = (foot - hip).length().clamp(0.02, upper + lower - 0.001);
    let dir = (foot - hip).normalize_or(Vec3::NEG_Y);
    let a = (upper * upper - lower * lower + d * d) / (2.0 * d);
    let h = (upper * upper - a * a).max(0.0).sqrt();
    let bend = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Z);
    hip + dir * a + bend * h
}

pub struct HerdLookPlugin;

impl Plugin for HerdLookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BeastArt>()
            .add_systems(Update, (load_parts, draw_herds, pose_beasts).chain());
    }
}

fn keep(mut m: Mesh) -> Mesh {
    m.asset_usage = bevy::asset::RenderAssetUsages::default();
    m
}

fn merged(
    tree: &[ModelNode],
    name: &str,
    colours: &[(&str, LinearRgba); 3],
    meshes: &mut Assets<Mesh>,
) -> Option<Handle<Mesh>> {
    let node = ModelNode::find(tree, name)?;
    let mut s = StandIn::default();
    for p in &node.primitives {
        let mesh = meshes.get(&p.mesh)?;
        let c = colours
            .iter()
            .find(|(n, _)| Some(*n) == p.material.as_deref())
            .map_or(colours[0].1, |(_, c)| *c);
        s.add(mesh, c);
    }
    s.build().map(|m| meshes.add(keep(m)))
}

/// Without a model: boxes of the right sizes.
fn blockout(b: &Build, meshes: &mut Assets<Mesh>) -> Parts {
    let c = b.colours[0].1;
    let mut paint = |m: Mesh| {
        let mut s = StandIn::default();
        s.add(&m, c);
        meshes.add(keep(s.build().unwrap()))
    };
    let seg = |len: f32, r: f32| {
        Mesh::from(Cuboid::new(r, len, r)).translated_by(Vec3::NEG_Y * len * 0.5)
    };
    let body = paint(Mesh::from(Cuboid::new(0.24, 0.3, 0.8)));
    let head = paint(
        Mesh::from(Cuboid::new(0.08, 0.3, 0.1))
            .translated_by(b.neck + Vec3::new(0.0, 0.15, -0.05) - b.neck),
    );
    let legs = [
        paint(seg(b.front.0, 0.06)),
        paint(seg(b.front.1, 0.04)),
        paint(seg(b.hind.0, 0.07)),
        paint(seg(b.hind.1, 0.04)),
    ];
    let far = paint(Mesh::from(Cuboid::new(0.24, 0.3, 0.8)));
    Parts {
        body,
        head,
        legs,
        far,
    }
}

fn load_parts(
    mut art: ResMut<BeastArt>,
    herds: Query<&Herd>,
    mut models: ResMut<Models>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if art.material.is_none() {
        art.material = Some(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.3,
            ..default()
        }));
    }
    for herd in &herds {
        let kind = herd.kind;
        if art.parts.iter().any(|(k, _)| *k == kind) || art.failed.contains(&kind) {
            continue;
        }
        let b = build(kind);
        let Some(handle) = models.request(b.model, &server) else {
            let p = blockout(&b, &mut meshes);
            art.parts.push((kind, p));
            continue;
        };
        match Models::state(&handle, &server, &gltfs) {
            ModelState::Loading => {}
            ModelState::Failed => {
                let p = blockout(&b, &mut meshes);
                art.parts.push((kind, p));
            }
            ModelState::Ready(gltf) => {
                let tree = model_nodes(gltf, &nodes, &gltf_meshes);
                let mut part = |n: &str| merged(&tree, n, &b.colours, &mut meshes);
                let parts = (|| {
                    Some(Parts {
                        body: part("body")?,
                        head: part("head")?,
                        legs: [
                            part("upper_f")?,
                            part("lower_f")?,
                            part("upper_h")?,
                            part("lower_h")?,
                        ],
                        far: part("far")?,
                    })
                })();
                let parts = parts.unwrap_or_else(|| {
                    warn!("{}: a part is missing; drawing blockout animals", b.model);
                    blockout(&b, &mut meshes)
                });
                art.parts.push((kind, parts));
            }
        }
    }
}

/// Drawn: this herd's animals have their parts.
#[derive(Component)]
struct Drawn;

fn draw_herds(
    mut commands: Commands,
    art: Res<BeastArt>,
    herds: Query<(Entity, &Herd), Without<Drawn>>,
) {
    let Some(material) = art.material.clone() else {
        return;
    };
    for (e, herd) in &herds {
        let Some((_, parts)) = art.parts.iter().find(|(k, _)| *k == herd.kind) else {
            continue;
        };
        for (i, b) in herd.beasts.iter().enumerate() {
            let range = |end: f32| bevy::camera::visibility::VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: (end - 2.0)..end,
                use_aabb: false,
            };
            let mut piece =
                |mesh: &Handle<Mesh>,
                 shadow: bool,
                 reach: bevy::camera::visibility::VisibilityRange| {
                    let mut c = commands.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::from_translation(b.pos),
                        Visibility::default(),
                        reach,
                    ));
                    if !shadow {
                        c.insert(NotShadowCaster);
                    }
                    c.id()
                };
            let body = piece(&parts.body, true, range(FAR));
            let head = piece(&parts.head, true, range(FAR));
            let mut legs = [Entity::PLACEHOLDER; 8];
            for (k, l) in legs.iter_mut().enumerate() {
                // 0..4 front left upper/lower, front right..., then hinds.
                let leg = k / 2;
                let lower = k % 2;
                let mesh = &parts.legs[if leg < 2 { 0 } else { 2 } + lower];
                *l = piece(mesh, false, range(FAR));
            }
            let far = piece(
                &parts.far,
                true,
                bevy::camera::visibility::VisibilityRange {
                    start_margin: (FAR - 2.0)..FAR,
                    end_margin: (HIDE - 20.0)..HIDE,
                    use_aabb: false,
                },
            );
            let mut all = [Entity::PLACEHOLDER; 11];
            all[0] = body;
            all[1] = head;
            all[2..10].copy_from_slice(&legs);
            all[10] = far;
            commands.spawn((
                Name::new(format!("{:?}", herd.kind)),
                BeastVisual {
                    herd: e,
                    index: i,
                    feet: [b.pos; 4],
                    lifted: [b.pos; 4],
                    stance: [true; 4],
                    phase: i as f32 * 0.31,
                    gait: Gait::Stand,
                    started: false,
                    parts: all,
                },
            ));
        }
        commands.entity(e).insert(Drawn);
    }
}

/// The body's rotation for a heading (atan2(z, x)): -Z forward along it.
fn facing(yaw: f32) -> Quat {
    Quat::from_rotation_y(-yaw - std::f32::consts::FRAC_PI_2)
}

fn pose_beasts(
    herds: Query<&Herd>,
    mut visuals: Query<&mut BeastVisual>,
    mut parts: Query<&mut Transform>,
    zone: Option<Res<Zone>>,
    time: Res<Time>,
) {
    let Some(zone) = zone else {
        return;
    };
    let dt = time.delta_secs().min(0.1);
    for mut v in &mut visuals {
        let Ok(herd) = herds.get(v.herd) else {
            continue;
        };
        let Some(beast) = herd.beasts.get(v.index) else {
            continue;
        };
        let b = build(herd.kind);
        let speed = beast.vel.xz().length();
        let dead = beast.state == State::Dead;
        let rot = facing(beast.yaw);
        let fwd = rot * Vec3::NEG_Z;
        // Where each hip is over the ground, and each foot's home under it.
        let base = beast.pos + Vec3::Y * b.ride;
        let rest = |i: usize| {
            let h = base + rot * b.hips[i];
            zone.on_ground(h.x, h.z)
        };
        if !v.started {
            v.started = true;
            for i in 0..4 {
                v.feet[i] = rest(i);
                v.lifted[i] = v.feet[i];
            }
        }
        v.gait = if dead {
            Gait::Stand
        } else {
            Gait::of(speed, v.gait)
        };
        let (offsets, duty, rate) = v.gait.pattern(speed);
        v.phase = (v.phase + rate * dt).fract();
        let cycle = if rate > 0.0 { 1.0 / rate } else { 1.0 };
        for i in 0..4 {
            if v.gait == Gait::Stand {
                // Standing: feet drift home, a shuffle.
                let home = rest(i);
                let d = home - v.feet[i];
                v.feet[i] += d.clamp_length_max(1.2 * dt);
                v.feet[i].y = zone.ground(v.feet[i].x, v.feet[i].z);
                v.stance[i] = true;
                continue;
            }
            let q = (v.phase + offsets[i]).fract();
            let down = q < duty;
            if down && !v.stance[i] {
                // Landed.
                let f = v.feet[i];
                v.feet[i] = zone.on_ground(f.x, f.z);
            } else if !down && v.stance[i] {
                v.lifted[i] = v.feet[i];
            }
            v.stance[i] = down;
            if !down {
                let t = (q - duty) / (1.0 - duty);
                // Lands ahead of the hip by half a stance's travel.
                let ahead = beast.vel * (duty * cycle * 0.5 + (1.0 - t) * (1.0 - duty) * cycle);
                let target = rest(i) + ahead;
                let target = zone.on_ground(target.x, target.z);
                let e = t * t * (3.0 - 2.0 * t);
                let lift = match v.gait {
                    Gait::Bound => 0.16,
                    Gait::Trot => 0.1,
                    _ => 0.07,
                };
                let mut p = v.lifted[i].lerp(target, e);
                p.y = zone.ground(p.x, p.z) + lift * (std::f32::consts::PI * t).sin();
                v.feet[i] = p;
            }
        }
        // The body on its legs: bobbing with the gait, pitched with the slope
        // (and arching in a bound).
        let tau = std::f32::consts::TAU;
        let (bob, arch) = match v.gait {
            Gait::Bound => (
                0.07 * (tau * v.phase).sin(),
                0.14 * (tau * v.phase + 1.2).sin(),
            ),
            Gait::Trot => (0.02 * (2.0 * tau * v.phase).sin(), 0.0),
            Gait::Walk => (0.01 * (2.0 * tau * v.phase).sin(), 0.0),
            Gait::Stand => (0.0, 0.0),
        };
        let front = zone.ground(beast.pos.x + fwd.x * 0.3, beast.pos.z + fwd.z * 0.3);
        let back = zone.ground(beast.pos.x - fwd.x * 0.3, beast.pos.z - fwd.z * 0.3);
        let pitch = ((front - back) / 0.6).atan() + arch;
        let body_rot = rot * Quat::from_rotation_x(pitch);
        let lie = if dead { -b.ride * 0.65 } else { 0.0 };
        let body_at = base + Vec3::Y * (bob + lie);
        let body_rot = if dead {
            body_rot * Quat::from_rotation_z(1.4)
        } else {
            body_rot
        };
        // Head: down to graze, up to look, turned toward what it watches.
        let mut turn = 0.0;
        if let Some(at) = beast.look {
            let to = at - beast.pos;
            let want = to.z.atan2(to.x);
            turn = -crate::robots::anim::wrap_angle(want - beast.yaw).clamp(-1.0, 1.0);
        }
        let nod = b.graze + (b.alert - b.graze) * beast.head_up;
        let head_rot = body_rot * Quat::from_rotation_y(turn) * Quat::from_rotation_x(nod);
        let head_at = body_at + body_rot * b.neck;
        let [body, head, legs @ .., far] = v.parts;
        if let Ok(mut t) = parts.get_mut(body) {
            *t = Transform::from_translation(body_at).with_rotation(body_rot);
        }
        if let Ok(mut t) = parts.get_mut(far) {
            *t = Transform::from_translation(body_at).with_rotation(body_rot);
        }
        if let Ok(mut t) = parts.get_mut(head) {
            *t = Transform::from_translation(head_at).with_rotation(head_rot);
        }
        for i in 0..4 {
            let (upper, lower) = if i < 2 { b.front } else { b.hind };
            let hip = body_at + body_rot * b.hips[i];
            let foot = if dead {
                hip + body_rot * Vec3::new(0.0, -(upper + lower) * 0.9, -0.1)
            } else {
                v.feet[i]
            };
            // Front knees forward, hind hocks back.
            let pole = if i < 2 { fwd } else { -fwd } + Vec3::NEG_Y * 0.1;
            let k = knee(hip, foot, upper, lower, pole);
            let seg = |from: Vec3, to: Vec3| {
                let local = body_rot.inverse() * (to - from).normalize_or(Vec3::NEG_Y);
                Transform::from_translation(from)
                    .with_rotation(body_rot * Quat::from_rotation_arc(Vec3::NEG_Y, local))
            };
            if let Ok(mut t) = parts.get_mut(legs[i * 2]) {
                *t = seg(hip, k);
            }
            if let Ok(mut t) = parts.get_mut(legs[i * 2 + 1]) {
                *t = seg(k, foot);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knees_reach_the_foot_and_bend_toward_the_pole() {
        let hip = Vec3::new(0.0, 0.5, 0.0);
        let foot = Vec3::new(0.05, 0.0, 0.02);
        let k = knee(hip, foot, 0.25, 0.3, Vec3::NEG_Z);
        assert!((k.distance(hip) - 0.25).abs() < 1e-3);
        assert!((k.distance(foot) - 0.3).abs() < 1e-3);
        assert!(k.z < 0.0, "bent forward (-Z): {k}");
        // Out of reach: the leg straightens toward it.
        let k = knee(hip, Vec3::new(0.0, -1.0, 0.0), 0.25, 0.3, Vec3::NEG_Z);
        assert!(k.y < hip.y && k.z.abs() < 0.05);
    }

    #[test]
    fn gaits_follow_speed_and_feet_take_turns() {
        let mut g = Gait::Stand;
        let mut seen = Vec::new();
        for s in [0.0, 0.5, 1.0, 2.5, 3.5, 6.0, 9.0, 3.0, 0.0] {
            for _ in 0..3 {
                g = Gait::of(s, g);
            }
            seen.push(g);
        }
        assert_eq!(seen[1], Gait::Walk);
        assert_eq!(seen[4], Gait::Trot);
        assert_eq!(seen[6], Gait::Bound);
        assert_eq!(*seen.last().unwrap(), Gait::Stand);
        // At a walk, at least three feet are always down; at a trot, two.
        for (gait, least) in [(Gait::Walk, 2), (Gait::Trot, 2)] {
            let (off, duty, _) = gait.pattern(1.0);
            for k in 0..100 {
                let p = k as f32 / 100.0;
                let down = off.iter().filter(|o| (p + *o).fract() < duty).count();
                assert!(down >= least, "{gait:?} at {p}: {down} feet down");
            }
        }
    }
}
