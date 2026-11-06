//! A gauge of what a round landing on a machine looks and sounds like:
//! every part of every machine in a `--gallery robots:...+hits` line-up
//! (or with `EXTRACTION_HIT_GAUGE=1`) is shot in turn, as a round would
//! strike it (a `Hit`, as `ballistics` writes one), and what follows is
//! counted:
//!
//! - within [`WINDOW`] frames (100 ms at 90 Hz), the effect at the point
//!   (meshes and lights spawned within [`NEAR`] m of it, not on the
//!   machine) and the sounds placed there (their cues, `audio_mix`);
//! - a second round that leaves the part failing (under a third of its
//!   strength, as a few more rounds would), and [`SETTLE`] frames later the
//!   machine's drawn meshes held against how they were before the first:
//!   a mesh or material changed, or a new piece on the part (a crack, a
//!   dent), is the part looking damaged. A round's mark (`fx::shell_marks`)
//!   is a mark, not a damaged look: it's counted apart.
//!
//! Each part's row goes to the log (`hit gauge:`) and, with
//! `EXTRACTION_HIT_GAUGE_OUT`, to that file as tab-separated lines. The
//! test `every_part_of_every_machine_answers_a_hit` holds every row to it;
//! `scripts/hit_gauge.sh` renders the contact sheet.

use std::collections::{HashMap, HashSet};

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{PartRole, Robot, RobotPart};

/// Frames a round's effect and sound have to show in (100 ms at 90 Hz).
pub const WINDOW: u32 = 9;
/// How near the point (m) an effect or a sound counts as the hit's.
pub const NEAR: f32 = 2.5;
/// Frames after the second round that the damaged look is taken (1.2 s:
/// past an eye's hurt flicker, `visuals::alert_eyes`).
pub const SETTLE: u32 = 108;
/// Frames the line-up stands before the first round (dressed, settled).
pub const STAND: u32 = 300;
/// The least a round's effect spans at the eye (degrees), out to 30 m:
/// what reads as a hit in a fight at 15 to 25 m.
pub const MIN_SPAN: f32 = 0.4;
/// The same as an area at the eye (pixels, [`PX_PER_DEG`]): a hit's
/// pieces cover at least a square [`MIN_SPAN`] on a side. (Its span alone
/// can't tell: sparks fly half a metre in 100 ms, too thin to see.)
pub const MIN_AREA: f32 = MIN_SPAN * PX_PER_DEG * MIN_SPAN * PX_PER_DEG;
/// How far (m, along the ground: a flyer rises to its height) a line-up
/// machine may go from where it was put by the time it stands.
pub const STAY: f32 = 0.5;
/// The first round's damage (a rifle round's, `items`), or less on a part
/// with little to it.
const ROUND: f32 = 40.0;
/// The second round leaves the part at this share of its strength.
const FAILING: f32 = 0.3;

/// One part's row.
#[derive(Clone, Debug)]
pub struct Row {
    pub kind: String,
    pub role: PartRole,
    /// The first round: effect pieces and cues, and the frame each first
    /// showed on (1-based, `None` if not within the window).
    pub fx: usize,
    pub fx_at: Option<u32>,
    pub cues: Vec<&'static str>,
    pub cue_at: Option<u32>,
    /// The first round's effect at its biggest within the window: its
    /// span at the eye (degrees) and its area there (pixels, [`Seen`]).
    pub span: f32,
    pub area: f32,
    /// How far off the camera was (m).
    pub dist: f32,
    /// The second round's.
    pub fx2: usize,
    pub cues2: Vec<&'static str>,
    /// Marks left on the part.
    pub marks: usize,
    /// Drawn pieces of the machine changed or added (marks aside).
    pub look: usize,
    /// The part's strength after the two rounds (share).
    pub left: f32,
}

impl Row {
    pub fn seen(&self) -> bool {
        self.fx > 0 && self.fx2 > 0
    }
    pub fn heard(&self) -> bool {
        !self.cues.is_empty() && !self.cues2.is_empty()
    }
    pub fn looks_damaged(&self) -> bool {
        self.look > 0
    }
    pub fn ok(&self) -> bool {
        self.seen() && self.heard() && self.looks_damaged()
    }

    pub fn line(&self) -> String {
        let cues = |c: &[&str]| {
            let mut c = c.to_vec();
            c.sort();
            c.dedup();
            if c.is_empty() {
                "-".to_owned()
            } else {
                c.join(",")
            }
        };
        format!(
            "{}\t{:?}\tfx {} (frame {})\t{:.2} deg, {:.0} px at {:.0} m\tsound {} (frame {})\tagain fx {} sound {}\tmarks {}\tlook {}\tleft {:.0}%\t{}",
            self.kind,
            self.role,
            self.fx,
            self.fx_at.map_or("-".into(), |f| f.to_string()),
            self.span,
            self.area,
            self.dist,
            cues(&self.cues),
            self.cue_at.map_or("-".into(), |f| f.to_string()),
            self.fx2,
            cues(&self.cues2),
            self.marks,
            self.look,
            self.left * 100.0,
            if self.ok() {
                "ok".to_owned()
            } else {
                let mut why = Vec::new();
                if !self.seen() {
                    why.push("UNSEEN");
                }
                if !self.heard() {
                    why.push("UNHEARD");
                }
                if !self.looks_damaged() {
                    why.push("NO DAMAGED LOOK");
                }
                why.join(" ")
            }
        )
    }
}

/// What the gauge has found, and where it is.
#[derive(Resource, Default)]
pub struct HitGauge {
    frame: u32,
    targets: Vec<(Entity, Entity, String)>,
    next: usize,
    /// The part being shot: when its first round went in.
    at: u32,
    point: Vec3,
    meshes_before: HashSet<Entity>,
    sounds_before: HashSet<Entity>,
    look_before: HashMap<Entity, (AssetId<Mesh>, AssetId<StandardMaterial>)>,
    row: Option<Row>,
    pub rows: Vec<Row>,
    /// Each machine as it stands, and how many of its meshes the player's
    /// camera draws ([`drawn_meshes`]).
    pub drawn: Vec<(String, usize)>,
    /// Each machine and how far (m) it went between its first frame and
    /// its standing one: a line-up machine stands where it's put.
    pub moved: Vec<(String, f32)>,
    placed: HashMap<Entity, Vec3>,
    /// First rounds only, for how a hit reads at range.
    pub quick: bool,
    pub done: bool,
}

pub struct HitGaugePlugin;

impl Plugin for HitGaugePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, gauge.run_if(enabled));
    }
}

fn enabled(opts: Option<Res<crate::LaunchOptions>>) -> bool {
    std::env::var_os("EXTRACTION_HIT_GAUGE").is_some()
        || opts
            .and_then(|o| o.gallery.clone())
            .is_some_and(|g| crate::dev::gallery_word(&g, "hits"))
}

/// The machine's name as the line-up has it.
pub fn kind_name(world: &World, e: Entity) -> String {
    use super::*;
    let has = |c: bevy::ecs::component::ComponentId| world.entity(e).contains_id(c);
    let named = [
        (world.component_id::<heavy::Heavy>(), "heavy"),
        (world.component_id::<sentry::Sentry>(), "sentry"),
        (world.component_id::<spotter::Spotter>(), "spotter"),
        (world.component_id::<minelayer::Minelayer>(), "minelayer"),
        (world.component_id::<mender::Mender>(), "mender"),
        (world.component_id::<plough::Plough>(), "plough"),
        (world.component_id::<hound::Hound>(), "hound"),
        (world.component_id::<grappler::Grappler>(), "grappler"),
        (world.component_id::<indoor::Crawler>(), "crawler"),
    ];
    for (id, name) in named {
        if id.is_some_and(has) {
            return name.into();
        }
    }
    match world.get::<Robot>(e).map(|r| r.kind) {
        Some(RobotKind::Drone) => "drone",
        Some(RobotKind::Walker) => "walker",
        Some(RobotKind::Dirigible) => "dirigible",
        Some(RobotKind::Pylon) => "pylon",
        Some(RobotKind::Sorter) => "sorter",
        None => "?",
    }
    .into()
}

/// How many of a machine's meshes the player's camera would draw: shown
/// (their visibility inherited all the way down) and on its layer (none
/// set, or the first). A machine with none can't be seen, whatever its
/// parts do.
pub fn drawn_meshes(world: &mut World, robot: Entity) -> usize {
    use bevy::camera::visibility::RenderLayers;
    let mut roots = vec![robot];
    let mut q = world.query::<(Entity, &RobotPart)>();
    roots.extend(
        q.iter(world)
            .filter(|(_, p)| p.robot == robot)
            .map(|(e, _)| e),
    );
    let mut q = world.query::<(Entity, &super::wear::DrawnFor)>();
    let drawn_for: Vec<(Entity, Entity)> = q.iter(world).map(|(e, d)| (e, d.0)).collect();
    for (e, d) in drawn_for {
        if d == robot || world.get::<RobotPart>(d).is_some_and(|p| p.robot == robot) {
            roots.push(e);
        }
    }
    let mut seen = HashSet::new();
    roots
        .into_iter()
        .flat_map(|r| subtree(world, r))
        .filter(|e| seen.insert(*e))
        .filter(|e| {
            world.get::<Mesh3d>(*e).is_some()
                && world
                    .get::<InheritedVisibility>(*e)
                    .is_some_and(|v| v.get())
                && world
                    .get::<RenderLayers>(*e)
                    .is_none_or(|l| l.intersects(&RenderLayers::layer(0)))
        })
        .count()
}

/// Every entity under `root`, itself included.
fn subtree(world: &World, root: Entity) -> Vec<Entity> {
    let mut out = vec![root];
    let mut i = 0;
    while i < out.len() {
        if let Some(c) = world.get::<Children>(out[i]) {
            out.extend(c.iter());
        }
        i += 1;
    }
    out
}

/// The machine's drawn pieces: each one's mesh and material, marks aside.
fn look_of(
    world: &mut World,
    robot: Entity,
    marks: &[AssetId<StandardMaterial>],
) -> HashMap<Entity, (AssetId<Mesh>, AssetId<StandardMaterial>)> {
    let mut out = HashMap::new();
    // Its own, and what's drawn for it or its parts away from it.
    let mut roots = vec![robot];
    let mut q = world.query::<(Entity, &super::wear::DrawnFor)>();
    let drawn: Vec<(Entity, Entity)> = q.iter(world).map(|(e, d)| (e, d.0)).collect();
    // (Its parts that stand apart from it: a sentry's box, a spotter's
    // winch.)
    let mut q = world.query::<(Entity, &RobotPart)>();
    roots.extend(
        q.iter(world)
            .filter(|(_, p)| p.robot == robot)
            .map(|(e, _)| e),
    );
    for (e, d) in drawn {
        let of = d == robot || world.get::<RobotPart>(d).is_some_and(|p| p.robot == robot);
        if of {
            roots.push(e);
        }
    }
    let all: Vec<Entity> = roots.into_iter().flat_map(|r| subtree(world, r)).collect();
    for e in all {
        let (Some(mesh), Some(mat)) = (
            world.get::<Mesh3d>(e),
            world.get::<MeshMaterial3d<StandardMaterial>>(e),
        ) else {
            continue;
        };
        if marks.contains(&mat.id()) {
            continue;
        }
        if world
            .get::<Visibility>(e)
            .is_some_and(|v| *v == Visibility::Hidden)
        {
            continue;
        }
        out.insert(e, (mesh.id(), mat.id()));
    }
    out
}

fn mark_materials(world: &World) -> Vec<AssetId<StandardMaterial>> {
    world
        .get_resource::<crate::weapons::impacts::FxAssets>()
        .map(|a| vec![a.bright_pock.id(), a.hole.id()])
        .unwrap_or_default()
}

fn all_with<T: Component>(world: &mut World) -> HashSet<Entity> {
    world
        .query_filtered::<Entity, With<T>>()
        .iter(world)
        .collect()
}

/// Where a round from the camera's side strikes `part`: on its collider,
/// coming in from `toward` (the camera), or at its middle.
fn aim(world: &mut World, part: Entity, toward: Vec3) -> (Vec3, Vec3) {
    let centre = world
        .get::<ColliderAabb>(part)
        .map(|a| a.center())
        .or_else(|| world.get::<GlobalTransform>(part).map(|g| g.translation()))
        .unwrap_or(Vec3::ZERO);
    let back = (toward - centre).normalize_or(Vec3::Z);
    let from = centre + back * 4.0;
    let Ok(dir) = Dir3::new(-back) else {
        return (centre, back);
    };
    let mut state = bevy::ecs::system::SystemState::<SpatialQuery>::new(world);
    let Ok(spatial) = state.get(world) else {
        return (centre, back);
    };
    let hit = spatial
        .ray_hits(from, dir, 8.0, 32, true, &SpatialQueryFilter::default())
        .into_iter()
        .find(|h| h.entity == part)
        .map(|h| (from + *dir * h.distance, h.normal));
    hit.unwrap_or((centre, back))
}

fn camera_at(world: &mut World) -> Vec3 {
    world
        .query_filtered::<&GlobalTransform, With<Camera3d>>()
        .iter(world)
        .next()
        .map(|g| g.translation())
        .unwrap_or(Vec3::new(0.0, 50.0, 0.0))
}

/// Writes one round at `part`, as `ballistics` does.
fn shoot(world: &mut World, part: Entity, point: Vec3, normal: Vec3, damage: f32) {
    world.write_message(crate::weapons::ballistics::Hit {
        entity: part,
        point,
        normal,
        velocity: -normal * 700.0,
        damage,
        shooter: None,
        mass_g: 3.4,
        load: Default::default(),
        blow: None,
    });
}

/// New effect pieces and sounds near the point since the round went in.
/// Pixels a degree on the desktop view the sheets are shot at: 1080 lines
/// over the free camera's 75 degrees (`render::desktop_fov`).
pub const PX_PER_DEG: f32 = 1080.0 / 75.0;

/// What showed of a round since it went in: effect pieces near the point,
/// the cues placed there, and how big the effect is at the eye: the
/// angle (degrees) of the sphere round the point that holds every piece,
/// and the pieces' summed area square to the eye in pixels
/// ([`PX_PER_DEG`]).
struct Seen {
    fx: usize,
    cues: Vec<&'static str>,
    span: f32,
    area: f32,
}

fn since(world: &mut World, g: &HitGauge, robot: Entity) -> Seen {
    let mut mine: HashSet<Entity> = subtree(world, robot).into_iter().collect();
    let mut q = world.query::<(Entity, &super::wear::DrawnFor)>();
    let roots: Vec<Entity> = q.iter(world).map(|(e, _)| e).collect();
    for r in roots {
        mine.extend(subtree(world, r));
    }
    let p = g.point;
    let eye = camera_at(world);
    let d = eye.distance(p).max(0.1);
    let (mut fx, mut reach, mut area) = (0, 0.0f32, 0.0f32);
    let mut q = world.query_filtered::<(
        Entity,
        &GlobalTransform,
        Option<&bevy::camera::primitives::Aabb>,
        Has<Mesh3d>,
    ), Or<(With<Mesh3d>, With<PointLight>)>>();
    for (e, gt, aabb, mesh) in q.iter(world) {
        if g.meshes_before.contains(&e) || mine.contains(&e) {
            continue;
        }
        let at = gt.translation();
        if at.distance(p) >= NEAR {
            continue;
        }
        fx += 1;
        if !mesh {
            continue;
        }
        // Its half size in the world: its bounds, scaled.
        let half = aabb.map_or(Vec3::splat(0.01), |a| Vec3::from(a.half_extents)) * gt.scale();
        reach = reach.max(at.distance(p) + half.max_element());
        // Square to the eye: its two largest sides.
        let mut s = [half.x, half.y, half.z];
        s.sort_by(|a, b| b.total_cmp(a));
        let deg = |m: f32| (m / d).atan().to_degrees();
        area += deg(2.0 * s[0]) * deg(2.0 * s[1]) * PX_PER_DEG * PX_PER_DEG;
    }
    let mut cues = Vec::new();
    let mut q = world.query::<(Entity, &crate::audio::Placed, &Transform)>();
    for (e, placed, t) in q.iter(world) {
        if g.sounds_before.contains(&e) {
            continue;
        }
        if t.translation.distance(p) < NEAR + 1.0 {
            cues.push(placed.cue());
        }
    }
    Seen {
        fx,
        cues,
        span: 2.0 * (reach / d).atan().to_degrees(),
        area,
    }
}

fn gauge(world: &mut World) {
    let mut g = world.remove_resource::<HitGauge>().unwrap_or_default();
    step(world, &mut g);
    world.insert_resource(g);
}

fn step(world: &mut World, g: &mut HitGauge) {
    if g.done {
        return;
    }
    let any = world
        .query_filtered::<(), With<Robot>>()
        .iter(world)
        .next()
        .is_some();
    if !any {
        return;
    }
    g.frame += 1;
    if g.frame == 1 || g.frame == STAND {
        // Each machine and its height over the ground (one under it, or
        // off where it can't be seen, shows here).
        let zone = world.get_resource::<crate::world::Zone>().cloned();
        let mut q = world.query_filtered::<(Entity, &GlobalTransform), With<Robot>>();
        // (Where its body is: a physics body's own place, which a pinned
        // Transform can hide, as a fleeing minelayer's did.)
        let at: Vec<(Entity, Vec3)> = q
            .iter(world)
            .map(|(e, g)| (e, g.translation()))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(e, p)| (e, world.get::<Position>(e).map_or(p, |b| b.0)))
            .collect();
        let list: Vec<String> = at
            .iter()
            .map(|&(e, p)| {
                let over = zone.as_ref().map_or(0.0, |z| p.y - z.ground(p.x, p.z));
                format!(
                    "{} at {:.1},{:.1} ({over:+.1} m)",
                    kind_name(world, e),
                    p.x,
                    p.z
                )
            })
            .collect();
        info!("hit gauge: frame {}: {}", g.frame, list.join("; "));
        if g.frame == 1 {
            g.placed = at.iter().copied().collect();
        }
        if g.frame == STAND {
            g.drawn = at
                .iter()
                .map(|&(e, _)| (kind_name(world, e), drawn_meshes(world, e)))
                .collect();
            g.moved = at
                .iter()
                .filter_map(|&(e, p)| {
                    let was = g.placed.get(&e)?;
                    Some((kind_name(world, e), was.xz().distance(p.xz())))
                })
                .collect();
        }
    }
    if g.frame < STAND {
        return;
    }
    if std::env::var_os("EXTRACTION_HIT_SHOW").is_some() {
        show(world, g);
        return;
    }
    if g.frame == STAND {
        let mut parts: Vec<(Entity, Entity, PartRole)> = world
            .query::<(Entity, &RobotPart)>()
            .iter(world)
            .map(|(e, p)| (p.robot, e, p.role))
            .collect();
        parts.sort_by_key(|(r, _, role)| (r.index_u32(), format!("{role:?}")));
        g.targets = parts
            .into_iter()
            .map(|(r, e, _)| (r, e, kind_name(world, r)))
            .collect();
        info!("hit gauge: {} parts", g.targets.len());
        g.next = 0;
        g.at = 0;
    }
    let Some(&(robot, part, ref kind)) = g.targets.get(g.next) else {
        finish(g);
        return;
    };
    let kind = kind.clone();
    if world.get_entity(part).is_err() || world.get::<RobotPart>(part).is_none() {
        g.next += 1;
        g.at = 0;
        return;
    }
    let t = g.frame - g.at;
    if g.at == 0 || t == 0 {
        // The first round.
        let marks = mark_materials(world);
        g.look_before = look_of(world, robot, &marks);
        g.meshes_before = all_with::<Mesh3d>(world);
        g.meshes_before.extend(all_with::<PointLight>(world));
        g.sounds_before = all_with::<crate::audio::Placed>(world);
        let cam = camera_at(world);
        let (point, normal) = aim(world, part, cam);
        g.point = point;
        let p = world.get::<RobotPart>(part).unwrap();
        let (role, hp) = (p.role, p.hp);
        shoot(world, part, point, normal, ROUND.min(hp * 0.4));
        g.row = Some(Row {
            kind,
            role,
            fx: 0,
            fx_at: None,
            cues: Vec::new(),
            cue_at: None,
            span: 0.0,
            area: 0.0,
            dist: cam.distance(point),
            fx2: 0,
            cues2: Vec::new(),
            marks: 0,
            look: 0,
            left: 1.0,
        });
        g.at = g.frame;
        return;
    }
    if t <= WINDOW {
        let Seen {
            fx,
            cues,
            span,
            area,
        } = since(world, g, robot);
        let row = g.row.as_mut().unwrap();
        row.span = row.span.max(span);
        row.area = row.area.max(area);
        if fx > 0 && row.fx_at.is_none() {
            row.fx_at = Some(t);
        }
        if !cues.is_empty() && row.cue_at.is_none() {
            row.cue_at = Some(t);
        }
        row.fx = row.fx.max(fx);
        for c in cues {
            if !row.cues.contains(&c) {
                row.cues.push(c);
            }
        }
        return;
    }
    // At range (`HIT_GAUGE_QUICK`): how the first round reads is all.
    if g.quick || std::env::var_os("HIT_GAUGE_QUICK").is_some() {
        let row = g.row.take().unwrap();
        info!("hit gauge: {}", row.line());
        g.rows.push(row);
        g.next += 1;
        g.at = 0;
        return;
    }
    // The second round, a little after: the part left failing.
    let second = WINDOW + 21;
    if t == second {
        g.meshes_before = all_with::<Mesh3d>(world);
        g.meshes_before.extend(all_with::<PointLight>(world));
        g.sounds_before = all_with::<crate::audio::Placed>(world);
        let p = world.get::<RobotPart>(part).unwrap();
        let damage = (p.hp - p.max_hp * FAILING).max(1.0);
        let normal = (camera_at(world) - g.point).normalize_or(Vec3::Z);
        shoot(world, part, g.point, normal, damage);
        return;
    }
    if t > second && t <= second + WINDOW {
        let Seen { fx, cues, .. } = since(world, g, robot);
        let row = g.row.as_mut().unwrap();
        row.fx2 = row.fx2.max(fx);
        for c in cues {
            if !row.cues2.contains(&c) {
                row.cues2.push(c);
            }
        }
        return;
    }
    if t < second + SETTLE {
        return;
    }
    // The damaged look.
    let marks = mark_materials(world);
    let now = look_of(world, robot, &marks);
    let changed = now
        .iter()
        .filter(|(e, v)| g.look_before.get(*e) != Some(*v))
        .count();
    let gone = g
        .look_before
        .keys()
        .filter(|e| !now.contains_key(*e) && world.get_entity(**e).is_ok())
        .count();
    let marked = subtree(world, part)
        .into_iter()
        .filter(|e| {
            world
                .get::<MeshMaterial3d<StandardMaterial>>(*e)
                .is_some_and(|m| marks.contains(&m.id()))
        })
        .count();
    let left = world
        .get::<RobotPart>(part)
        .map_or(0.0, |p| p.hp / p.max_hp.max(1e-3));
    let mut row = g.row.take().unwrap();
    row.look = changed + gone;
    row.marks = marked;
    row.left = left;
    info!("hit gauge: {}", row.line());
    g.rows.push(row);
    g.next += 1;
    g.at = 0;
}

fn finish(g: &mut HitGauge) {
    g.done = true;
    let ok = g.rows.iter().filter(|r| r.ok()).count();
    info!("hit gauge: done, {ok}/{} parts answer a hit", g.rows.len());
    if let Some(path) = std::env::var_os("EXTRACTION_HIT_GAUGE_OUT") {
        let text: String = g.rows.iter().map(|r| r.line() + "\n").collect();
        if let Err(e) = std::fs::write(&path, text) {
            warn!("hit gauge: can't write {path:?}: {e}");
        }
    }
}

/// Every machine the line-up has (`dev::gallery`), for the gauge.
pub const EVERY_MACHINE: &str = "walker+drone+grappler+sentry+hound+heavy+crawler+dirigible+pylon+sorter+spotter+minelayer+mender+plough";

/// The machines a `+`-separated list asks for.
pub fn kinds_asked(list: &str) -> Vec<String> {
    list.split('+').map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every part of every machine, shot: within 100 ms of each round an
    /// effect at the point and a sound there, and once it's failing the
    /// part looks it. (A client in a valley, no window and no GPU: some
    /// minutes. `HIT_GAUGE_KINDS=heavy+drone` for some of them.)
    /// The client in Marrow, `kinds` lined up `dist` m off, the gauge run
    /// over them to its end.
    fn run(kinds: &str, dist: f32, quick: bool) -> App {
        let opts = crate::LaunchOptions {
            map: "marrow_creek".into(),
            map_explicit: true,
            new_game: true,
            robots: false,
            gallery: Some(format!("robots:{kinds}+hits,0,{dist}")),
            camera: Some((Vec3::new(92.9, 0.0, 131.9), 0.0, -4.0)),
            ..Default::default()
        };
        let mut app = crate::soak::drive::client_app(opts);
        app.insert_resource(HitGauge {
            quick,
            ..Default::default()
        });
        for _ in 0..200_000 {
            app.update();
            if app
                .world()
                .get_resource::<HitGauge>()
                .is_some_and(|g| g.done)
            {
                break;
            }
        }
        app
    }

    #[test]
    #[ignore = "the whole client in a valley; scripts/hit_gauge.sh"]
    fn every_part_of_every_machine_answers_a_hit() {
        let kinds = std::env::var("HIT_GAUGE_KINDS").unwrap_or(EVERY_MACHINE.into());
        let app = run(&kinds, 17.0, false);
        let g = app.world().resource::<HitGauge>();
        assert!(g.done, "the gauge never finished ({} rows)", g.rows.len());
        for r in &g.rows {
            println!("{}", r.line());
        }
        let bad: Vec<String> = g
            .rows
            .iter()
            .filter(|r| !r.ok())
            .map(|r| r.line())
            .collect();
        assert!(!g.rows.is_empty(), "no parts shot");
        // Every machine drawn for the player's camera at all.
        for (kind, n) in &g.drawn {
            println!("{kind}: {n} meshes drawn");
        }
        let unseen: Vec<&String> = g
            .drawn
            .iter()
            .filter(|(_, n)| *n == 0)
            .map(|(k, _)| k)
            .collect();
        assert!(
            unseen.is_empty(),
            "machines nothing of which is drawn: {unseen:?}"
        );
        // Every machine stood where it was put (one that wandered off, or
        // fled the camera, isn't in its frame).
        let wandered: Vec<String> = g
            .moved
            .iter()
            .filter(|(_, d)| *d > STAY)
            .map(|(k, d)| format!("{k} {d:.1} m"))
            .collect();
        assert!(
            wandered.is_empty(),
            "machines that left their place: {wandered:?}"
        );
        let kinds: HashSet<&str> = g.drawn.iter().map(|(k, _)| k.as_str()).collect();
        for want in kinds_asked(&std::env::var("HIT_GAUGE_KINDS").unwrap_or(EVERY_MACHINE.into())) {
            assert!(
                kinds.contains(want.as_str()),
                "{want} never stood in the line-up"
            );
        }
        assert!(
            bad.is_empty(),
            "{} of {} parts:\n{}",
            bad.len(),
            g.rows.len(),
            bad.join("\n")
        );
    }

    /// A round landing on the big machines reads at fighting range: its
    /// effect covers at least [`MIN_AREA`] at the eye from 10 m out to
    /// 30 m (`HIT_GAUGE_RANGE_KINDS` for others).
    #[test]
    #[ignore = "the whole client in a valley, three times; scripts/hit_gauge.sh"]
    fn a_hit_reads_out_to_thirty_metres() {
        let kinds = std::env::var("HIT_GAUGE_RANGE_KINDS")
            .unwrap_or("heavy+plough+pylon+sorter+sentry".into());
        let mut short = Vec::new();
        for dist in [10.0, 20.0, 30.0] {
            let app = run(&kinds, dist, true);
            let g = app.world().resource::<HitGauge>();
            assert!(g.done && !g.rows.is_empty(), "no rounds at {dist} m");
            let mut by_kind: HashMap<&str, Vec<&Row>> = HashMap::new();
            for r in &g.rows {
                by_kind.entry(r.kind.as_str()).or_default().push(r);
            }
            for (kind, rows) in by_kind {
                let mut spans: Vec<f32> = rows.iter().map(|r| r.span).collect();
                let mut areas: Vec<f32> = rows.iter().map(|r| r.area).collect();
                spans.sort_by(f32::total_cmp);
                areas.sort_by(f32::total_cmp);
                let med = |v: &[f32]| v[v.len() / 2];
                println!(
                    "{dist:>2} m {kind:<10} span min {:.2} median {:.2} deg; area min {:.0} median {:.0} px ({} parts)",
                    spans[0],
                    med(&spans),
                    areas[0],
                    med(&areas),
                    rows.len()
                );
                for r in rows.iter().filter(|r| r.area < MIN_AREA) {
                    short.push(format!(
                        "{dist} m {kind} {:?} {:.0} px ({:.2} deg)",
                        r.role, r.area, r.span
                    ));
                }
            }
        }
        assert!(
            short.is_empty(),
            "hits under {MIN_SPAN} deg:\n{}",
            short.join("\n")
        );
    }
}

/// Frames into a show (`EXTRACTION_HIT_SHOW`) that its rounds start
/// landing, and how often after (a screenshot any time after has some in
/// flight: a spark lives about 0.3 s).
pub const SHOW_HITS: u32 = STAND + SETTLE;
pub const SHOW_EVERY: u32 = 12;

/// For a contact sheet: every part left failing at once, and once that
/// shows, a round into each part every [`SHOW_EVERY`] frames, each part
/// put back to failing after (so it stays as worn, and stays on), so a
/// frame taken has the hits landing on the machines worn by them
/// (`scripts/hit_gauge.sh`).
fn show(world: &mut World, g: &mut HitGauge) {
    let since = g.frame.saturating_sub(SHOW_HITS);
    let landing = g.frame >= SHOW_HITS && since % SHOW_EVERY == 0;
    let mending = g.frame > SHOW_HITS && since % SHOW_EVERY == 1;
    if mending {
        for mut p in world.query::<&mut RobotPart>().iter_mut(world) {
            if p.alive() {
                p.hp = p.hp.max(p.max_hp * FAILING);
            }
        }
        return;
    }
    if g.frame != STAND && !landing {
        return;
    }
    let cam = camera_at(world);
    let parts: Vec<(Entity, f32, f32)> = world
        .query::<(Entity, &RobotPart)>()
        .iter(world)
        .map(|(e, p)| (e, p.hp, p.max_hp))
        .collect();
    for (part, hp, max) in parts {
        if hp <= 0.0 {
            continue;
        }
        let (point, normal) = aim(world, part, cam);
        let damage = if g.frame == STAND {
            (hp - max * FAILING).max(1.0)
        } else {
            (max * 0.15).min(hp * 0.6)
        };
        shoot(world, part, point, normal, damage);
    }
    if g.frame == STAND || since == 0 {
        info!("hit gauge: show, rounds in from frame {}", g.frame);
    }
}
