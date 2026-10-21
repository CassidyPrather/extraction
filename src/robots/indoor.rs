//! Robots indoors and underground: a site's own navigation grid, patrols,
//! and the crawlers that walk them.
//!
//! An underground site (or any building with a plan of its own) gives its
//! walkable grid in site-local x/z, with a floor height per cell
//! ([`LocalNav`]). A walker carrying one paths, searches and picks cover on
//! that grid instead of the Zone's ([`NavRef`]). **Crawlers** are small
//! walkers that live in a site: they [`Patrol`] a route of points, pausing
//! to look round at each, and when they hear or glimpse something they
//! investigate, search and fight with the same brain as any walker, with a
//! short-range spark gun that crackles before it fires ([`CrawlerCharge`]).
//!
//! Sites register themselves in [`IndoorSites`]; the director's
//! [`spawn_residents`] puts each site's crawlers on their routes once.
//! When what's passable changes (water rises, gas fills a gallery, a
//! furnace vents) the site sends a new grid ([`SiteGrid`]) and its
//! residents path on that from then on.

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::nav::NavGrid;

use super::{Awareness, Brain, Job, JobKind, PartRole, Robot, RobotPart, part_bundle};
use crate::weapons::ballistics::Shot;
use crate::world::Zone;

/// A site's own walkable grid: cells in site-local x/z (`xf` places the
/// site in the world), with a floor height per cell (site-local y, NaN
/// for rock) when the site has more than one level.
#[derive(Component, Clone, Debug)]
pub struct LocalNav {
    pub grid: Arc<NavGrid>,
    pub xf: Transform,
    pub floors: Option<Arc<Vec<f32>>>,
}

impl LocalNav {
    fn local(&self, p: Vec3) -> Vec3 {
        self.xf.compute_affine().inverse().transform_point3(p)
    }

    fn floor(&self, l: Vec3) -> Option<f32> {
        let Some(floors) = &self.floors else {
            return Some(0.0);
        };
        let (i, j) = self.grid.cell_of([l.x, l.z])?;
        let y = floors[j * self.grid.n + i];
        (!y.is_nan()).then_some(y)
    }

    fn world(&self, p: [f32; 2]) -> Vec3 {
        let l = Vec3::new(p[0], 0.0, p[1]);
        let y = self.floor(l).unwrap_or(0.0);
        self.xf.transform_point(l.with_y(y))
    }

    /// On this site's walkable floor (and near its height, where there are
    /// levels).
    pub fn walkable(&self, p: Vec3) -> bool {
        let l = self.local(p);
        self.grid.walkable([l.x, l.z]) && self.floor(l).is_some_and(|y| (l.y - y).abs() < 2.5)
    }

    /// Inside the site at all (a sound or a robot that's in it).
    pub fn contains(&self, p: Vec3) -> bool {
        let l = self.local(p);
        self.floor(l).is_some_and(|y| (l.y - y).abs() < 3.0)
    }

    /// A path across the site, world points (floors included).
    pub fn path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let (a, b) = (self.local(from), self.local(to));
        self.grid
            .path([a.x, a.z], [b.x, b.z])
            .map(|p| p.into_iter().map(|q| self.world(q)).collect())
    }
}

/// Whichever grid a robot walks: the Zone's, or a site's.
#[derive(Clone, Copy)]
pub enum NavRef<'a> {
    Zone(&'a NavGrid),
    Local(&'a LocalNav),
}

impl NavRef<'_> {
    pub fn walkable(&self, p: Vec3) -> bool {
        match self {
            NavRef::Zone(g) => g.walkable([p.x, p.z]),
            NavRef::Local(l) => l.walkable(p),
        }
    }

    /// Waypoints after the start (y is the floor's on a site, 0 outside:
    /// walkers steer on x/z).
    pub fn path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        match self {
            NavRef::Zone(g) => g.path([from.x, from.z], [to.x, to.z]).map(|p| {
                p.into_iter()
                    .skip(1)
                    .map(|q| Vec3::new(q[0], 0.0, q[1]))
                    .collect()
            }),
            NavRef::Local(l) => l.path(from, to).map(|p| p.into_iter().skip(1).collect()),
        }
    }

    /// Length of the walk from `from` to `to`, if there is one.
    pub fn walk_length(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let p = self.path(from, to)?;
        let mut here = from;
        let mut len = 0.0;
        for q in p {
            len += Vec2::new(q.x - here.x, q.z - here.z).length();
            here = q;
        }
        Some(len)
    }
}

/// A round to walk while there's nothing else to do: each point in turn,
/// a look round at each, then back to the first.
#[derive(Component, Clone, Debug)]
pub struct Patrol {
    pub route: Vec<Vec3>,
    pub next: usize,
    pub dwell: f32,
}

/// Pause at each patrol point (s).
pub const PATROL_DWELL: f32 = 3.0;
/// A patrol walks at this share of its usual pace.
pub const PATROL_PACE: f32 = 0.6;

impl Patrol {
    pub fn new(route: Vec<Vec3>) -> Self {
        Self {
            route,
            next: 0,
            dwell: 0.0,
        }
    }

    /// Where to go this tick (None: stand and look), and where to look.
    pub fn step(&mut self, pos: Vec3, dt: f32, sweep: f32) -> (Option<Vec3>, Option<Vec3>) {
        if self.route.is_empty() {
            return (None, None);
        }
        if self.dwell > 0.0 {
            self.dwell -= dt;
            let look = pos + Vec3::new(sweep.cos(), 0.0, sweep.sin()) * 5.0;
            return (None, Some(look));
        }
        let to = self.route[self.next % self.route.len()];
        if Vec2::new(to.x - pos.x, to.z - pos.z).length() < 1.2 {
            self.next = (self.next + 1) % self.route.len();
            self.dwell = PATROL_DWELL;
            return (None, None);
        }
        (Some(to), None)
    }
}

/// A site's residents, registered by whoever builds the site (Underground
/// fills these when its sites are placed).
#[derive(Clone, Debug)]
pub struct IndoorSite {
    pub nav: LocalNav,
    /// World points to patrol (room centres along the spine work).
    pub patrol: Vec<Vec3>,
    pub spawn: Vec3,
    pub crawlers: u32,
    /// Full-size armed walkers on the same round, slower (a room with the
    /// headroom: they don't fit a 1.5 m corridor).
    pub wardens: u32,
    /// Its residents start asleep in their bays, and wake on a loud
    /// enough noise ([`WAKE_RADIUS`]).
    pub dormant: bool,
}

/// A site's grid has changed (new costs, cells blocked or opened): `site`
/// is its index in [`IndoorSites`]. Its residents path on `grid` from now.
#[derive(Message, Clone, Debug)]
pub struct SiteGrid {
    pub site: usize,
    pub grid: Arc<NavGrid>,
}

/// A robot that lives in a site: where its bay is (it goes back there to
/// sleep when the machines are recalled).
#[derive(Component, Debug, Clone, Copy)]
pub struct Resident {
    pub bay: Vec3,
}

/// Asleep in its bay: blind, still. A noise that carries at least
/// [`WAKE_RADIUS`] and reaches it wakes it.
#[derive(Component, Debug, Clone, Copy)]
pub struct Dormant;

/// Waking: up in this many seconds (then off on its round, or to the
/// noise that woke it).
#[derive(Component, Debug, Clone, Copy)]
pub struct Stirring {
    pub left: f32,
    pub toward: Option<Vec3>,
}

/// A resident woke (for its sound and the body's stir).
#[derive(Message, Clone, Copy, Debug)]
pub struct Waking {
    pub robot: Entity,
    pub at: Vec3,
}

/// A warden: a full-size walker on a site's round (not the Heavy
/// machine, `heavy_mind`).
#[derive(Component, Debug, Clone, Copy)]
pub struct Warden;

/// A noise must carry at least this far to wake a sleeping resident (m):
/// a breaker's clank, a shot, a grenade; not a footstep.
pub const WAKE_RADIUS: f32 = 25.0;
/// Waking takes this long (s).
pub const WAKE_TIME: f32 = 2.0;
/// A heavy walks its round at this share of its pace.
pub const WARDEN_PACE: f32 = 0.5;

#[derive(Resource, Default, Debug)]
pub struct IndoorSites {
    pub sites: Vec<IndoorSite>,
    /// Their crawlers are out.
    pub spawned: bool,
}

/// A small walker that lives in a site, with a spark gun.
#[derive(Component, Debug, Default)]
pub struct Crawler {
    pub cooldown: f32,
    /// Seconds into charging a zap (0: not charging).
    pub charge: f32,
    /// Wrecked, and its parts no longer stop the player.
    pub cleared: bool,
}

/// The spark gun's emitter part.
#[derive(Component)]
pub struct CrawlerGun;

/// A crawler began charging its spark gun (the tell: crackle, glow).
#[derive(Message, Clone, Copy, Debug)]
pub struct CrawlerCharge {
    pub crawler: Entity,
    pub at: Vec3,
}

/// A crawler zapped: from its emitter toward where it aimed (for the arc;
/// the round itself is an ordinary `Shot` and hits or misses as one).
#[derive(Message, Clone, Copy, Debug)]
pub struct CrawlerZap {
    pub crawler: Entity,
    pub from: Vec3,
    pub to: Vec3,
}

/// The spark gun reaches this far (m)...
pub const ZAP_RANGE: f32 = 18.0;
/// ...charges this long before each zap (s)...
pub const ZAP_CHARGE: f32 = 0.5;
/// ...hits this hard...
pub const ZAP_DAMAGE: f32 = 12.0;
/// ...and recharges this long between (s).
pub const ZAP_COOLDOWN: f32 = 1.6;
/// A crawler fights from no further than this (m).
pub const CRAWLER_HOLD: f32 = 10.0;
/// How much its core takes (a walker's is 130).
pub const CRAWLER_HP: f32 = 100.0;
/// Hearing you within reach of its gun, a crawler lies in wait this long
/// before it comes to look (s).
pub const AMBUSH: f32 = 8.0;
/// How big a crawler is against a walker (Robot bodies' `spawn_walker_sized`).
pub const CRAWLER_SIZE: f32 = 0.6;

pub struct IndoorPlugin;

impl Plugin for IndoorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IndoorSites>()
            .add_message::<CrawlerCharge>()
            .add_message::<CrawlerZap>()
            .add_message::<Waking>()
            .add_message::<SiteGrid>()
            .add_systems(
                FixedUpdate,
                (
                    regrid_sites,
                    spawn_residents,
                    crawler_guns,
                    clear_wrecks,
                    wake_residents,
                    recall_residents,
                )
                    .after(super::perceive),
            );
    }
}

/// A crawler on its site, patrolling `route`.
pub fn spawn_crawler(
    commands: &mut Commands,
    zone: &Zone,
    site: &IndoorSite,
    at: Vec3,
    route: Vec<Vec3>,
) -> Entity {
    let job = Job {
        kind: JobKind::Watch,
        site: at,
        duration: f32::INFINITY,
        exit: at,
    };
    // A small walker standing on the site's floor (`at.y`), not on the
    // terrain over it.
    let e = super::walker::spawn_walker_sized(
        commands,
        zone,
        at,
        job,
        false,
        CRAWLER_HP,
        CRAWLER_SIZE,
        Some(at.y),
    );
    commands
        .entity(e)
        .insert((
            Name::new("Crawler"),
            Crawler::default(),
            super::Breed("Crawler"),
            Resident { bay: at },
            site.nav.clone(),
            Patrol::new(route),
            Brain::Working {
                at,
                left: f32::INFINITY,
            },
        ))
        .with_children(|c| {
            c.spawn((
                Name::new("Crawler emitter"),
                CrawlerGun,
                part_bundle(
                    e,
                    PartRole::Weapon,
                    30.0,
                    Collider::sphere(0.08),
                    // Under the visor, proud of the orb (the model's
                    // `emitter`, art/robots/walker.py).
                    Transform::from_xyz(0.0, -0.1, -0.44),
                ),
            ));
        });
    e
}

/// Once there are sites, their crawlers go out on their rounds, each
/// starting at a different point of the route.
fn spawn_residents(
    mut commands: Commands,
    mut sites: ResMut<IndoorSites>,
    zone: Res<Zone>,
    director: Option<Res<super::director::Director>>,
) {
    if sites.spawned || sites.sites.is_empty() || !director.is_some_and(|d| d.enabled) {
        return;
    }
    sites.spawned = true;
    for site in &sites.sites {
        let n = site.patrol.len().max(1);
        for k in 0..site.crawlers as usize {
            let mut route = site.patrol.clone();
            let start = (k * n) / site.crawlers.max(1) as usize;
            let len = route.len();
            route.rotate_left(start.min(len));
            let at = route.first().copied().unwrap_or(site.spawn);
            info!("director: a crawler on its round at {at:.0}");
            let e = spawn_crawler(&mut commands, &zone, site, at, route);
            if site.dormant {
                commands.entity(e).insert(Dormant);
            }
        }
        for k in 0..site.wardens as usize {
            let mut route = site.patrol.clone();
            let len = route.len();
            route.rotate_left((k * n / site.wardens.max(1) as usize).min(len));
            let at = route.first().copied().unwrap_or(site.spawn);
            info!("director: a warden on its round at {at:.0}");
            let e = spawn_warden(&mut commands, &zone, site, at, route);
            if site.dormant {
                commands.entity(e).insert(Dormant);
            }
        }
    }
}

/// A site's new grid: the site keeps it for residents yet to come out, and
/// those already out (the ones carrying its old grid) take it and plan
/// their way again.
fn regrid_sites(
    mut grids: MessageReader<SiteGrid>,
    mut sites: ResMut<IndoorSites>,
    mut residents: Query<(&mut LocalNav, &mut super::walker::Walker), With<Resident>>,
) {
    for g in grids.read() {
        let Some(site) = sites.sites.get_mut(g.site) else {
            warn!("a new grid for site {}, which isn't there", g.site);
            continue;
        };
        let old = std::mem::replace(&mut site.nav.grid, g.grid.clone());
        for (mut nav, mut w) in &mut residents {
            if Arc::ptr_eq(&nav.grid, &old) {
                nav.grid = g.grid.clone();
                w.path.clear();
                w.goal = None;
            }
        }
    }
}

/// A full-size armed walker living on a site's round.
pub fn spawn_warden(
    commands: &mut Commands,
    zone: &Zone,
    site: &IndoorSite,
    at: Vec3,
    route: Vec<Vec3>,
) -> Entity {
    let job = Job {
        kind: JobKind::Watch,
        site: at,
        duration: f32::INFINITY,
        exit: at,
    };
    let e =
        super::walker::spawn_walker_sized(commands, zone, at, job, true, 130.0, 1.0, Some(at.y));
    commands.entity(e).insert((
        Name::new("Warden"),
        Warden,
        super::Breed("Warden"),
        Resident { bay: at },
        site.nav.clone(),
        Patrol::new(route),
        Brain::Working {
            at,
            left: f32::INFINITY,
        },
    ));
    e
}

/// Sleeping residents wake on a loud enough noise that reaches them.
fn wake_residents(
    mut commands: Commands,
    mut noises: MessageReader<super::Noise>,
    sleepers: Query<(Entity, &GlobalTransform, &super::Senses), (With<Dormant>, Without<Stirring>)>,
    mut stirring: Query<(Entity, &mut Stirring, &mut Brain)>,
    raid: Res<crate::raid::Raid>,
    time: Res<Time>,
    mut out: MessageWriter<Waking>,
) {
    let loud: Vec<super::Noise> = noises
        .read()
        .filter(|n| n.radius >= WAKE_RADIUS)
        .cloned()
        .collect();
    // Recalled, they sleep for good.
    if !raid.recalled {
        for (e, gt, senses) in &sleepers {
            let at = gt.translation();
            let woke = loud
                .iter()
                .find(|n| n.at.distance(at) < n.radius * senses.hearing);
            if let Some(n) = woke {
                commands.entity(e).insert(Stirring {
                    left: WAKE_TIME,
                    toward: (n.source.is_some() || n.radius > WAKE_RADIUS * 1.5).then_some(n.at),
                });
                out.write(Waking { robot: e, at });
            }
        }
    }
    for (e, mut s, mut brain) in &mut stirring {
        s.left -= time.delta_secs();
        if s.left <= 0.0 {
            commands.entity(e).remove::<(Dormant, Stirring)>();
            if let Some(at) = s.toward {
                *brain = Brain::Investigate {
                    at,
                    left: super::search::INVESTIGATE_TIME,
                };
            }
        }
    }
}

/// The machines recalled: residents walk back to their bays and go to
/// sleep there (capstone leaves them be).
fn recall_residents(
    mut commands: Commands,
    raid: Res<crate::raid::Raid>,
    mut residents: Query<
        (
            Entity,
            &Resident,
            &GlobalTransform,
            &mut Brain,
            &mut Job,
            &Robot,
        ),
        Without<Dormant>,
    >,
) {
    if !raid.recalled {
        return;
    }
    for (e, r, gt, mut brain, mut job, robot) in &mut residents {
        if robot.wrecked {
            continue;
        }
        let p = gt.translation();
        if Vec2::new(p.x - r.bay.x, p.z - r.bay.z).length() < 3.2 {
            commands.entity(e).insert(Dormant).remove::<Patrol>();
            continue;
        }
        if !matches!(*brain, Brain::Transit { to } if to == r.bay) {
            job.kind = JobKind::Collect; // single-minded: nothing distracts it now
            *brain = Brain::Transit { to: r.bay };
        }
    }
}

/// A wrecked crawler stops blocking people: in a 1.4 m corridor its wreck
/// is a wall nobody can step over (the sims found bots stuck behind one
/// for the rest of the raid). Its parts still take rounds and keep their
/// salvage.
fn clear_wrecks(
    mut commands: Commands,
    mut crawlers: Query<(&Robot, &mut Crawler, &Children), Changed<Robot>>,
    layers: Query<&CollisionLayers>,
) {
    for (robot, mut crawler, children) in &mut crawlers {
        if !robot.wrecked || crawler.cleared {
            continue;
        }
        crawler.cleared = true;
        for c in children.iter() {
            if let Ok(l) = layers.get(c) {
                let mut filters = l.filters;
                filters.remove(crate::world::Layer::Player);
                // A part may be torn off and gone by the time this lands.
                commands
                    .entity(c)
                    .try_insert(CollisionLayers::new(l.memberships, filters));
            }
        }
    }
}

/// The spark gun: in range and in sight, it charges (the tell), then zaps.
fn crawler_guns(
    mut crawlers: Query<(Entity, &mut Crawler, &Brain, &Awareness, &Robot, &Children)>,
    guns: Query<(&GlobalTransform, &RobotPart), With<CrawlerGun>>,
    targets: Query<&GlobalTransform>,
    stunned: Query<(), With<super::Stunned>>,
    time: Res<Time>,
    mut shots: MessageWriter<Shot>,
    mut charges: MessageWriter<CrawlerCharge>,
    mut zaps: MessageWriter<CrawlerZap>,
    mut seed: Local<u64>,
    minds: Query<&super::senses::MindSeed>,
) {
    let dt = time.delta_secs();
    for (e, mut c, brain, aware, robot, children) in &mut crawlers {
        c.cooldown -= dt;
        let target = aware
            .target
            .filter(|_| aware.can_see_target)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() + Vec3::Y * 0.9);
        let gun = children.iter().find_map(|g| guns.get(g).ok());
        let (Some(target), Some((gt, part))) = (target, gun) else {
            c.charge = 0.0;
            continue;
        };
        let muzzle = gt.translation();
        if robot.wrecked
            || stunned.contains(e)
            || !part.alive()
            || !matches!(brain, Brain::Engage)
            || muzzle.distance(target) > ZAP_RANGE
            || c.cooldown > 0.0
        {
            c.charge = 0.0;
            continue;
        }
        if c.charge < ZAP_CHARGE {
            if c.charge == 0.0 {
                charges.write(CrawlerCharge {
                    crawler: e,
                    at: muzzle,
                });
            }
            c.charge += dt;
            continue;
        }
        c.charge = 0.0;
        c.cooldown = ZAP_COOLDOWN;
        *seed = super::senses::shot_seed(&minds, e, &time);
        let dir = (target - muzzle).normalize_or(Vec3::NEG_Z);
        shots.write(Shot {
            gun: e,
            shooter: Some(e),
            origin: muzzle + dir * 0.15,
            direction: dir,
            speed: 300.0,
            damage: ZAP_DAMAGE,
            projectiles: 1,
            spread: 0.02,
            recoil: 0.0,
            seed: *seed,
        });
        zaps.write(CrawlerZap {
            crawler: e,
            from: muzzle,
            to: target,
        });
    }
}
