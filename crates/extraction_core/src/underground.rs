//! Underground sites: a bunker, a mine gallery, a service tunnel.
//!
//! A site is a small layout (`assets/underground/<id>.ron`): rooms as
//! rectangles, corridors between them, and portals (stairwells down from
//! a kiosk on the surface). Everything is in *site space*: the entrance
//! kiosk's centre on the ground, `+Z` the way its stair goes down, `y` up
//! from the ground the kiosk stands on. A map places a site as an
//! ordinary placement whose prefab is the site's id; its footprint is the
//! entrance kiosk's, and the rooms lie at `floor` below it, under the
//! ground.
//!
//! The layout is rasterised onto a fine grid ([`Raster`]): rooms and
//! corridors are open cells at the floor, stairs are open cells sloping
//! up to their kiosks, everything else is rock. The game builds walls on
//! the edges between open cells and rock; the bots' indoor grid, the
//! reach tests and the plan views all read the same raster.
//!
//! The terrain has no holes except where a stair goes down through it:
//! [`hole_cells`] lists the heightmap cells under each kiosk's stairwell,
//! which the heightfield collider and the terrain mesh leave out (the
//! kiosk's concrete apron covers the ragged edge outside its walls).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::geom::P2;
use crate::map::MapDef;

/// Raster cell size, metres.
pub const CELL: f32 = 0.25;
/// Half the clear width of a stair (and of its kiosk inside).
pub const STAIR_HALF: f32 = 0.75;
/// Wall thickness everywhere underground.
pub const WALL: f32 = 0.25;
/// A kiosk's walls' centre lines, portal space: x half-width, z half-depth.
pub const KIOSK: P2 = [STAIR_HALF + WALL * 0.5, 3.125];
/// The kiosk's floor stands this far above the ground under it.
pub const KIOSK_FLOOR: f32 = 0.15;
/// Where the stair's first step is, portal space z (a landing inside the
/// door before it).
pub const STAIR_TOP: f32 = -1.6;
/// The concrete apron round a kiosk, past its walls: it covers the edge of
/// the hole in the ground.
pub const APRON: f32 = 1.5;
/// Steepest riser and the tread.
pub const RISE_MAX: f32 = 0.19;
pub const TREAD: f32 = 0.28;
/// Headroom over a stair's nosings, and the soffit's thickness.
pub const STAIR_HEADROOM: f32 = 2.3;
/// Steps per flat run of a stair's stepped ceiling.
pub const CEILING_RUN: usize = 4;
/// Ceiling slabs' thickness.
pub const SLAB: f32 = 0.3;
/// At least this much ground over every ceiling.
pub const COVER: f32 = 0.8;
/// How far past its stairwell the ground is opened up.
pub const HOLE_PAD: f32 = 0.75;

/// What it's built of and how it's dressed.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Style {
    /// Civil-defence concrete: painted walls, steel doors, cable trays.
    Bunker,
    /// Rock, timber sets, a rail along the floor.
    Mine,
    /// Brick and concrete, pipes along the ceiling.
    Service,
}

/// A room's light.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum Lamp {
    /// None fitted.
    #[default]
    None,
    /// An emergency lamp still on.
    Steady,
    /// On, failing: it stutters.
    Flicker,
    /// A fitting, dead.
    Dead,
    /// On the site's power: dark until its stage's breaker is thrown or
    /// generator started ([`PowerDef`], the room's `stage`), then steady.
    Powered,
}

/// Furniture and junk, placed in a room (relative to its centre).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum PropKind {
    Shelf,
    Locker,
    Crate,
    SmallCrate,
    Table,
    Workbench,
    Bunk,
    Barrel,
    Pallet,
    Counter,
    /// Fallen concrete or rock: a heap you walk round.
    Rubble,
    /// A mine tub on the rail.
    Cart,
    /// A machine block (filter unit, pump, generator).
    Machine,
    /// A wall cabinet (switchgear, first aid).
    Cabinet,
    Chair,
    /// The Depot's: a wire bin (1.2 x 0.9 x 0.8), its loot inside.
    Bin,
    /// A sorting conveyor, 6 m along the turned X.
    Conveyor,
    /// A tagging station by the conveyor.
    Tagger,
    /// A "kept" crate, long (2.1 x 0.65 x 0.7), shut for good.
    KeptCrate,
    /// ...and a small one (0.8 x 0.5 x 0.6).
    KeptCrateSmall,
    /// A walker's charging cradle, its back (+Z) to the wall.
    Cradle,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Prop {
    pub kind: PropKind,
    /// Relative to the room's centre.
    pub at: P2,
    /// Degrees, as placements turn.
    #[serde(default)]
    pub rot: f32,
    /// Loot table rolled on it, if any.
    #[serde(default)]
    pub loot: Option<String>,
    /// A name it's known by (a kept crate's tag: who's in it).
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Room {
    pub id: String,
    /// Centre, site space.
    pub at: P2,
    /// Width (x) and depth (z).
    pub size: P2,
    /// Floor to ceiling.
    #[serde(default = "room_height")]
    pub height: f32,
    #[serde(default)]
    pub lamp: Lamp,
    #[serde(default)]
    pub props: Vec<Prop>,
    /// Its own floor, site y (a deeper level); the layout's otherwise.
    #[serde(default)]
    pub floor: Option<f32>,
    /// The power stage its `Lamp::Powered` lamp is on.
    #[serde(default)]
    pub stage: usize,
}

fn room_height() -> f32 {
    2.6
}

/// Which ends of a corridor have a door.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum Doors {
    #[default]
    None,
    From,
    To,
    Both,
}

/// A corridor from one room's centre to another's, through `via` points;
/// every leg runs along x or z.
#[derive(Clone, Debug, Deserialize)]
pub struct Link {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub via: Vec<P2>,
    #[serde(default = "link_width")]
    pub width: f32,
    #[serde(default)]
    pub doors: Doors,
}

fn link_width() -> f32 {
    1.5
}

/// A way in or out: a kiosk on the surface and a stair down from it.
#[derive(Clone, Debug, Deserialize)]
pub struct Portal {
    /// Kiosk centre, site space. The entrance (the first) is at the origin.
    #[serde(default)]
    pub at: P2,
    /// Degrees, as placements turn: the stair goes down along the turned
    /// +Z; the kiosk's door is on its -Z end.
    #[serde(default)]
    pub rot: f32,
    /// Ground at the kiosk, above the entrance's. The entrance's is 0.
    #[serde(default)]
    pub y: f32,
    /// A door across the stair's foot.
    #[serde(default = "yes")]
    pub door: bool,
    /// The floor its stair goes down to, site y; the layout's otherwise.
    #[serde(default)]
    pub floor: Option<f32>,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub struct Layout {
    pub name: String,
    pub style: Style,
    /// The rooms' floor, site space (negative: below the entrance).
    pub floor: f32,
    #[serde(default = "corridor_height")]
    pub corridor_height: f32,
    pub rooms: Vec<Room>,
    #[serde(default)]
    pub links: Vec<Link>,
    pub portals: Vec<Portal>,
    /// Anomalies that live down here, every cycle.
    #[serde(default)]
    pub anomalies: Vec<Hazard>,
    /// The resident crawlers' round: room ids in walking order (they
    /// path between them), and how many walk it.
    #[serde(default)]
    pub patrol: Vec<String>,
    #[serde(default)]
    pub crawlers: u32,
    /// More rounds, each with its own crawlers (a level each, say).
    #[serde(default)]
    pub rounds: Vec<Round>,
    /// The site's power, in stages: each one's switch does nothing until
    /// the one before is on.
    #[serde(default)]
    pub power: Vec<PowerDef>,
    /// Flooded rooms and corridors.
    #[serde(default)]
    pub floods: Vec<Flood>,
    /// Water that rises and falls: rooms and corridors sharing a level.
    #[serde(default)]
    pub basins: Vec<Basin>,
    /// What lowers a basin: a valve wheel or a hand pump.
    #[serde(default)]
    pub drains: Vec<Drain>,
    /// Firedamp: rooms and corridors full of gas, each a pocket that a
    /// spark sets off.
    #[serde(default)]
    pub gas: Vec<Pocket>,
    /// Rails, as polylines along the corridors (site space, legs along x
    /// or z), and the carts on them.
    #[serde(default)]
    pub rails: Vec<Vec<P2>>,
    #[serde(default)]
    pub carts: Vec<Cart>,
    /// Timber sets holding up a bad roof in a corridor: shoot one out and
    /// the roof comes down there.
    #[serde(default)]
    pub shoring: Vec<Shoring>,
    /// Particular things left about (by catalog id), every cycle.
    #[serde(default)]
    pub items: Vec<ItemSpot>,
    /// Its lamps' colour (linear RGB, full brightness 1), if not the
    /// shelters' warm tungsten: each site's own light.
    #[serde(default)]
    pub lamp_colour: Option<(f32, f32, f32)>,
}

/// A round of rooms the crawlers on it walk, in order (one room: round
/// its quarters).
#[derive(Clone, Debug, Deserialize)]
pub struct Round {
    pub rooms: Vec<String>,
    #[serde(default)]
    pub crawlers: u32,
    /// Full-size armed walkers on the round too (`robots::indoor`).
    #[serde(default)]
    pub heavy: u32,
    /// Asleep till something loud (a breaker, a shot) wakes them.
    #[serde(default)]
    pub dormant: bool,
}

/// A round as the residents get it: `patrol` and `crawlers`, or one of
/// `rounds`.
#[derive(Clone, Copy, Debug)]
pub struct Resident<'a> {
    pub rooms: &'a [String],
    pub crawlers: u32,
    pub heavy: u32,
    pub dormant: bool,
}

/// Standing water over a room's or a corridor's floor: slow to wade, loud,
/// and over [`DEEP`] the crawlers won't go in.
#[derive(Clone, Debug, Deserialize)]
pub struct Flood {
    /// A room by id...
    #[serde(default)]
    pub room: Option<String>,
    /// ... or a corridor by its ends (as its link names them).
    #[serde(default)]
    pub link: Option<(String, String)>,
    /// Water over the floor, metres.
    pub depth: f32,
}

/// Rooms and corridors under one water level (site y, not a depth: the
/// same water is knee deep on a high floor and over your head in a sump).
/// It falls as its drains are worked.
#[derive(Clone, Debug, Deserialize)]
pub struct Basin {
    pub id: String,
    #[serde(default)]
    pub rooms: Vec<String>,
    /// Corridors by their ends, as their links name them.
    #[serde(default)]
    pub links: Vec<(String, String)>,
    /// The level at the start, site y.
    pub level: f32,
}

/// A gas pocket: a room's or a corridor's air.
#[derive(Clone, Debug, Deserialize)]
pub struct Pocket {
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub link: Option<(String, String)>,
}

/// A catalog item lying in a room, `y` over its floor (on a bench, say).
#[derive(Clone, Debug, Deserialize)]
pub struct ItemSpot {
    pub id: String,
    pub room: String,
    pub at: P2,
    #[serde(default)]
    pub y: f32,
}

/// A mine cart on rail `rail`, `at` metres along it.
#[derive(Clone, Debug, Deserialize)]
pub struct Cart {
    pub rail: usize,
    pub at: f32,
}

/// Shoring in a corridor (by its ends), `along` its length (0 to 1).
#[derive(Clone, Debug, Deserialize)]
pub struct Shoring {
    pub link: (String, String),
    pub along: f32,
}

/// Where a corridor `(a, b)` runs: its points, room centre to room
/// centre through its vias.
pub fn link_points(l: &Layout, a: &str, b: &str) -> Option<Vec<P2>> {
    let k = l
        .links
        .iter()
        .find(|k| (k.from == a && k.to == b) || (k.from == b && k.to == a))?;
    let from = l.rooms.iter().find(|r| r.id == k.from)?;
    let to = l.rooms.iter().find(|r| r.id == k.to)?;
    let mut pts = vec![from.at];
    pts.extend(k.via.iter().copied());
    pts.push(to.at);
    Some(pts)
}

/// The point `s` metres along a polyline (clamped to its ends).
pub fn along(pts: &[P2], s: f32) -> P2 {
    let mut left = s.max(0.0);
    for w in pts.windows(2) {
        let d = sub(w[1], w[0]);
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if left <= len || len == 0.0 {
            let t = if len > 0.0 { left / len } else { 0.0 };
            return [w[0][0] + d[0] * t, w[0][1] + d[1] * t];
        }
        left -= len;
    }
    *pts.last().unwrap_or(&[0.0, 0.0])
}

/// A polyline's length.
pub fn length(pts: &[P2]) -> f32 {
    pts.windows(2)
        .map(|w| {
            let d = sub(w[1], w[0]);
            (d[0] * d[0] + d[1] * d[1]).sqrt()
        })
        .sum()
}

/// A way to lower a basin, standing in a room against a wall (as a power
/// switch does: it faces the turned -Z).
#[derive(Clone, Debug, Deserialize)]
pub struct Drain {
    pub kind: DrainKind,
    pub room: String,
    pub at: P2,
    #[serde(default)]
    pub rot: f32,
    /// The basin it lowers...
    pub basin: String,
    /// ... down to this level (site y), no further.
    pub to: f32,
    /// A valve's: metres a second once it's open. A pump's: metres a
    /// stroke.
    pub rate: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum DrainKind {
    /// Turn the wheel once: it drains on its own, gurgling, till done.
    Valve,
    /// Work the handle: each stroke lifts a little.
    Pump,
}

/// Water deeper than this is a wall to the walkers (as on the surface).
pub const DEEP: f32 = 0.9;

/// A stage of the site's power: a breaker board or a generator somewhere
/// awkward. Switching it on lights its stage's `Lamp::Powered` rooms, lets
/// its doors open, and makes a noise the crawlers hear.
#[derive(Clone, Debug, Deserialize)]
pub struct PowerDef {
    pub kind: PowerKind,
    /// The room it's in, and where (relative to the room's centre, against
    /// a wall: the switch faces the turned -Z).
    pub room: String,
    pub at: P2,
    #[serde(default)]
    pub rot: f32,
    /// The rooms whose doors stay shut until it's on (the door of the
    /// corridor into each), or `portal<N>`: that stair's doors, kiosk and
    /// foot.
    #[serde(default)]
    pub doors: Vec<String>,
}

impl PowerDef {
    /// The portals whose doors it holds.
    pub fn portals(&self) -> impl Iterator<Item = usize> + '_ {
        self.doors
            .iter()
            .filter_map(|id| id.strip_prefix("portal")?.parse().ok())
    }

    /// Whether it holds this door (of the raster's).
    pub fn holds(&self, l: &Layout, d: &DoorSpot) -> bool {
        d.room
            .is_some_and(|k| self.doors.iter().any(|id| l.rooms[k as usize].id == *id))
            || d.portal
                .is_some_and(|k| self.portals().any(|p| p == k as usize))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum PowerKind {
    /// A board of breakers: one big lever, a clank.
    Breaker,
    /// A diesel generator: pull-start, and it runs (loud) from then on.
    Generator,
}

/// An anomaly in a room: `kind` names an `anomaly::AnomalyKind` (Lift,
/// Burner, Snare); `at` is relative to the room's centre.
#[derive(Clone, Debug, Deserialize)]
pub struct Hazard {
    pub kind: String,
    pub room: String,
    #[serde(default)]
    pub at: P2,
    pub radius: f32,
}

impl Hazard {
    pub fn kind(&self) -> Option<crate::anomaly::AnomalyKind> {
        crate::anomaly::AnomalyKind::ALL
            .into_iter()
            .find(|k| format!("{k:?}") == self.kind)
    }
}

impl Layout {
    /// A room's floor, site y.
    /// Every round and its crawlers: `patrol` first, then `rounds`.
    pub fn residents(&self) -> Vec<Resident<'_>> {
        std::iter::once(Resident {
            rooms: self.patrol.as_slice(),
            crawlers: self.crawlers,
            heavy: 0,
            dormant: false,
        })
        .chain(self.rounds.iter().map(|r| Resident {
            rooms: r.rooms.as_slice(),
            crawlers: r.crawlers,
            heavy: r.heavy,
            dormant: r.dormant,
        }))
        .filter(|r| r.crawlers + r.heavy > 0 && !r.rooms.is_empty())
        .collect()
    }

    pub fn floor_of(&self, room: &Room) -> f32 {
        room.floor.unwrap_or(self.floor)
    }
}

/// The steepest a corridor between levels may step: a riser at most this
/// over a tread of at least [`STEP_CELLS`] cells.
pub const STEP_RISE: f32 = 0.19;
pub const STEP_CELLS: usize = 2;

fn corridor_height() -> f32 {
    2.3
}

/// Every site, by id: the layouts ship in the binary.
pub const SITES: [(&str, &str); 6] = [
    (
        "bunker_marrow",
        include_str!("../../../assets/underground/bunker_marrow.ron"),
    ),
    (
        "mine_kettle",
        include_str!("../../../assets/underground/mine_kettle.ron"),
    ),
    (
        "tunnel_cinder",
        include_str!("../../../assets/underground/tunnel_cinder.ron"),
    ),
    (
        "depot_marrow",
        include_str!("../../../assets/underground/depot_marrow.ron"),
    ),
    (
        "pump_marrow",
        include_str!("../../../assets/underground/pump_marrow.ron"),
    ),
    (
        "deep_kettle",
        include_str!("../../../assets/underground/deep_kettle.ron"),
    ),
];

fn layouts() -> &'static HashMap<&'static str, Layout> {
    static L: OnceLock<HashMap<&'static str, Layout>> = OnceLock::new();
    L.get_or_init(|| {
        SITES
            .iter()
            .map(|(id, text)| {
                let l: Layout =
                    ron::from_str(text).unwrap_or_else(|e| panic!("underground/{id}.ron: {e}"));
                (*id, l)
            })
            .collect()
    })
}

/// A site's layout, if `id` names one.
pub fn layout(id: &str) -> Option<&'static Layout> {
    layouts().get(id)
}

/// A site's raster, built once.
pub fn raster(id: &str) -> Option<&'static Raster> {
    static R: OnceLock<HashMap<&'static str, Raster>> = OnceLock::new();
    R.get_or_init(|| {
        SITES
            .iter()
            .map(|(id, _)| (*id, Raster::build(layout(id).unwrap())))
            .collect()
    })
    .get(id)
}

/// Whether a prefab id is an underground site.
pub fn is_site(id: &str) -> bool {
    layout(id).is_some()
}

/// The entrance kiosk with its apron: a site's footprint on the surface
/// (w, h, d), as `map::prefab_footprint` gives.
pub const FOOTPRINT: [f32; 3] = [
    2.0 * (KIOSK[0] + WALL * 0.5 + APRON),
    3.2,
    2.0 * (KIOSK[1] + WALL * 0.5 + APRON),
];

/// Turns a point by a placement's yaw (degrees, clockwise from above), as
/// the game's `Quat::from_rotation_y(-rot)` turns `(x, 0, z)`.
pub fn turn(p: P2, deg: f32) -> P2 {
    let (s, c) = deg.to_radians().sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

fn add(a: P2, b: P2) -> P2 {
    [a[0] + b[0], a[1] + b[1]]
}

fn sub(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

impl Portal {
    /// Site space to this portal's own (stair down +Z, door at -Z).
    pub fn to_portal(&self, p: P2) -> P2 {
        turn(sub(p, self.at), -self.rot)
    }

    pub fn to_site(&self, q: P2) -> P2 {
        add(self.at, turn(q, self.rot))
    }

    /// The kiosk's floor, site space y.
    pub fn top(&self) -> f32 {
        self.y + KIOSK_FLOOR
    }
}

/// One stair as built: `n` steps of `rise` down from the kiosk's floor,
/// then a flat landing to `foot` (portal space z) where it meets a room
/// or corridor.
#[derive(Clone, Copy, Debug)]
pub struct Stair {
    pub top: f32,
    pub n: usize,
    pub rise: f32,
    /// Where the last step ends, portal z.
    pub bottom: f32,
    /// Where the landing meets the rooms, portal z.
    pub foot: f32,
}

impl Stair {
    fn new(p: &Portal, floor: f32) -> Self {
        let drop = p.top() - floor;
        let n = (drop / RISE_MAX - 1e-4).ceil().max(1.0) as usize;
        let bottom = STAIR_TOP + n as f32 * TREAD;
        Self {
            top: p.top(),
            n,
            rise: drop / n as f32,
            bottom,
            foot: bottom,
        }
    }

    /// Top of the tread under portal z `z`: the kiosk's floor before the
    /// first step, the room floor past the last.
    pub fn floor_at(&self, z: f32) -> f32 {
        if z < STAIR_TOP {
            return self.top;
        }
        let k = (((z - STAIR_TOP) / TREAD).floor() as usize + 1).min(self.n);
        self.top - k as f32 * self.rise
    }

    /// Underside of the stepped ceiling over portal z `z` (past the kiosk):
    /// each run of [`CEILING_RUN`] steps has a flat soffit, headroom over
    /// the run's highest tread.
    pub fn ceiling_at(&self, z: f32) -> f32 {
        let k = (((z - STAIR_TOP) / TREAD).floor().max(0.0) as usize).min(self.n - 1);
        let run = k / CEILING_RUN;
        let first = (run * CEILING_RUN + 1).min(self.n);
        self.top - first as f32 * self.rise + STAIR_HEADROOM
    }

    /// Where the stair's tunnel leaves the kiosk: its soffit there is below
    /// the ground (and the kiosk's back wall closes the rest).
    pub fn kiosk_end(&self) -> f32 {
        KIOSK[1]
    }
}

/// What a raster cell is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Rock,
    Room(u16),
    /// A corridor: the link's index.
    Hall(u16),
    /// A stair (its kiosk, steps and landing): the portal's index.
    Stair(u16),
}

impl Cell {
    pub fn open(self) -> bool {
        self != Cell::Rock
    }

    /// Room or corridor: level with the floor.
    pub fn flat(self) -> bool {
        matches!(self, Cell::Room(_) | Cell::Hall(_))
    }
}

/// A door across a corridor's mouth or a stair's foot: its centre on the
/// wall line, which way the wall runs, and the opening's width.
#[derive(Clone, Copy, Debug)]
pub struct DoorSpot {
    pub at: P2,
    /// Unit vector along the wall.
    pub along: P2,
    pub width: f32,
    /// Floor under it, site y.
    pub y: f32,
    /// The room it opens into (a corridor's door), if any.
    pub room: Option<u16>,
    /// The portal whose stair's foot it's across, if it is.
    pub portal: Option<u16>,
}

/// The layout on a grid.
#[derive(Clone, Debug)]
pub struct Raster {
    /// Site-space corner of cell (0, 0).
    pub origin: P2,
    pub nx: usize,
    pub nz: usize,
    pub cells: Vec<Cell>,
    /// Ceiling height over the floor, flat cells (stairs: 0).
    pub ceil: Vec<f32>,
    /// The floor under each open cell, site y (rooms on their own levels;
    /// a corridor between levels steps from one to the other).
    pub floors: Vec<f32>,
    /// Water over the floor, flat cells (0: dry), as it starts.
    pub water: Vec<f32>,
    /// The basin each cell's water belongs to, if it rises and falls.
    pub basin: Vec<Option<u16>>,
    /// The gas pocket each cell's air belongs to, if it's bad.
    pub gas: Vec<Option<u16>>,
    pub floor: f32,
    pub stairs: Vec<Stair>,
    pub portals: Vec<Portal>,
    pub doors: Vec<DoorSpot>,
    /// Where every corridor meets a room, door or not (kept clear of props).
    pub mouths: Vec<DoorSpot>,
    /// Each link's corridor legs as rectangles (lo, hi).
    pub legs: Vec<Vec<(P2, P2)>>,
}

impl Raster {
    pub fn build(l: &Layout) -> Self {
        // Bounds: rooms, corridors, kiosks with their stairs (a stair runs
        // at most a few dozen metres).
        let mut lo = [f32::INFINITY; 2];
        let mut hi = [f32::NEG_INFINITY; 2];
        let mut grow = |p: P2| {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        };
        for r in &l.rooms {
            grow(sub(r.at, [r.size[0] * 0.5, r.size[1] * 0.5]));
            grow(add(r.at, [r.size[0] * 0.5, r.size[1] * 0.5]));
        }
        let stairs: Vec<Stair> = l
            .portals
            .iter()
            .map(|p| Stair::new(p, p.floor.unwrap_or(l.floor)))
            .collect();
        for (p, s) in l.portals.iter().zip(&stairs) {
            for q in [
                [-KIOSK[0] - 1.0, -KIOSK[1] - 1.0],
                [KIOSK[0] + 1.0, -KIOSK[1] - 1.0],
                [-KIOSK[0] - 1.0, s.bottom + 5.0],
                [KIOSK[0] + 1.0, s.bottom + 5.0],
            ] {
                grow(p.to_site(q));
            }
        }
        let origin = [
            (lo[0] / CELL).floor() * CELL - 2.0,
            (lo[1] / CELL).floor() * CELL - 2.0,
        ];
        let nx = ((hi[0] - origin[0]) / CELL).ceil() as usize + 4;
        let nz = ((hi[1] - origin[1]) / CELL).ceil() as usize + 4;
        let mut r = Raster {
            origin,
            nx,
            nz,
            cells: vec![Cell::Rock; nx * nz],
            ceil: vec![0.0; nx * nz],
            floors: vec![l.floor; nx * nz],
            water: vec![0.0; nx * nz],
            basin: vec![None; nx * nz],
            gas: vec![None; nx * nz],
            floor: l.floor,
            stairs,
            portals: l.portals.clone(),
            doors: Vec::new(),
            mouths: Vec::new(),
            legs: Vec::new(),
        };
        let room_ix: HashMap<&str, usize> = l
            .rooms
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.as_str(), i))
            .collect();
        // Corridors first; rooms over them.
        for (k, link) in l.links.iter().enumerate() {
            let (Some(&a), Some(&b)) = (
                room_ix.get(link.from.as_str()),
                room_ix.get(link.to.as_str()),
            ) else {
                r.legs.push(Vec::new());
                continue;
            };
            let mut pts = vec![l.rooms[a].at];
            pts.extend(link.via.iter().copied());
            pts.push(l.rooms[b].at);
            let h = link.width * 0.5;
            let legs: Vec<(P2, P2)> = pts
                .windows(2)
                .map(|w| {
                    (
                        [w[0][0].min(w[1][0]) - h, w[0][1].min(w[1][1]) - h],
                        [w[0][0].max(w[1][0]) + h, w[0][1].max(w[1][1]) + h],
                    )
                })
                .collect();
            for (lo, hi) in &legs {
                r.fill(
                    *lo,
                    *hi,
                    Cell::Hall(k as u16),
                    l.corridor_height,
                    l.floor_of(&l.rooms[a]),
                );
            }
            r.legs.push(legs);
            let n = pts.len();
            let from_door = matches!(link.doors, Doors::From | Doors::Both);
            let to_door = matches!(link.doors, Doors::To | Doors::Both);
            // Each end's leg that leaves its room (a via point can be
            // inside the room).
            let inside = |room: &Room, p: P2| {
                (p[0] - room.at[0]).abs() < room.size[0] * 0.5
                    && (p[1] - room.at[1]).abs() < room.size[1] * 0.5
            };
            let leaving = |room: &Room, path: Vec<P2>| -> (P2, P2) {
                path.windows(2)
                    .find(|w| !inside(room, w[1]))
                    .map_or((path[1], path[0]), |w| (w[1], w[0]))
            };
            let rev: Vec<P2> = pts.iter().rev().copied().collect();
            let (fo, fi) = leaving(&l.rooms[a], pts.clone());
            let (to, ti) = leaving(&l.rooms[b], rev);
            let _ = n;
            for (room, outer, inner, door) in [
                (&l.rooms[a], fo, fi, from_door),
                (&l.rooms[b], to, ti, to_door),
            ] {
                if let Some(mut d) = r.mouth(room, outer, inner, link.width) {
                    d.y = l.floor_of(room);
                    d.room = Some(if std::ptr::eq(room, &l.rooms[a]) {
                        a
                    } else {
                        b
                    } as u16);
                    r.mouths.push(d);
                    if door {
                        r.doors.push(d);
                    }
                }
            }
        }
        for (k, room) in l.rooms.iter().enumerate() {
            let h = [room.size[0] * 0.5, room.size[1] * 0.5];
            r.fill(
                sub(room.at, h),
                add(room.at, h),
                Cell::Room(k as u16),
                room.height,
                l.floor_of(room),
            );
        }
        // Corridors between levels: stepped down their length (outside
        // the rooms) from one room's floor to the other's.
        for (k, link) in l.links.iter().enumerate() {
            let (Some(a), Some(b)) = (
                l.rooms.iter().find(|x| x.id == link.from),
                l.rooms.iter().find(|x| x.id == link.to),
            ) else {
                continue;
            };
            let (fa, fb) = (l.floor_of(a), l.floor_of(b));
            if (fa - fb).abs() < 1e-3 {
                continue;
            }
            let mut pts = vec![a.at];
            pts.extend(link.via.iter().copied());
            pts.push(b.at);
            // Arc length along the corridor of each of its cells.
            let along = |p: P2| -> f32 {
                let mut best = (f32::INFINITY, 0.0);
                let mut s0 = 0.0;
                for w in pts.windows(2) {
                    let d = sub(w[1], w[0]);
                    let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-6);
                    let o = sub(p, w[0]);
                    let t = ((o[0] * d[0] + o[1] * d[1]) / len).clamp(0.0, len);
                    let q = add(w[0], [d[0] / len * t, d[1] / len * t]);
                    let e = sub(p, q);
                    let dist = e[0] * e[0] + e[1] * e[1];
                    if dist < best.0 {
                        best = (dist, s0 + t);
                    }
                    s0 += len;
                }
                best.1
            };
            let mine: Vec<(usize, f32)> = (0..r.cells.len())
                .filter(|c| r.cells[*c] == Cell::Hall(k as u16))
                .map(|c| (c, along(r.centre(c % r.nx, c / r.nx))))
                .collect();
            let Some(s_lo) = mine.iter().map(|m| m.1).reduce(f32::min) else {
                continue;
            };
            let n = ((fb - fa).abs() / STEP_RISE).ceil() as usize;
            let mut tread: HashMap<usize, usize> = mine
                .iter()
                .map(|&(c, s)| {
                    (
                        c,
                        (((s - s_lo) / CELL).floor() as usize / STEP_CELLS).min(n),
                    )
                })
                .collect();
            // Where it turns, cells either side of the corner's diagonal
            // measure from different legs, a tread or two apart: lower
            // each to one tread over its lowest neighbour, until none is
            // more.
            loop {
                let mut changed = false;
                for &(c, _) in &mine {
                    let (i, j) = (c % r.nx, c / r.nx);
                    let low = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)]
                        .iter()
                        .filter_map(|(di, dj)| {
                            let (a, b) = (i as i64 + di, j as i64 + dj);
                            (a >= 0 && b >= 0 && (a as usize) < r.nx && (b as usize) < r.nz)
                                .then(|| tread.get(&(b as usize * r.nx + a as usize)))
                                .flatten()
                                .copied()
                        })
                        .min();
                    if let Some(low) = low {
                        if tread[&c] > low + 1 {
                            tread.insert(c, low + 1);
                            changed = true;
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
            for (c, k) in tread {
                r.floors[c] = fa + (fb - fa) * k as f32 / n as f32;
            }
        }
        for f in &l.floods {
            let link = f.link.as_ref().and_then(|(a, b)| {
                l.links
                    .iter()
                    .position(|k| (&k.from, &k.to) == (a, b) || (&k.from, &k.to) == (b, a))
            });
            let room = f
                .room
                .as_ref()
                .and_then(|id| l.rooms.iter().position(|r| &r.id == id));
            for (c, cell) in r.cells.iter().enumerate() {
                let wet = match *cell {
                    Cell::Room(k) => room == Some(k as usize),
                    Cell::Hall(k) => link == Some(k as usize),
                    _ => false,
                };
                if wet {
                    r.water[c] = f.depth;
                }
            }
        }
        for (b, basin) in l.basins.iter().enumerate() {
            let rooms: Vec<usize> = basin
                .rooms
                .iter()
                .filter_map(|id| l.rooms.iter().position(|r| &r.id == id))
                .collect();
            let links: Vec<usize> = basin
                .links
                .iter()
                .filter_map(|(a, z)| {
                    l.links
                        .iter()
                        .position(|k| (&k.from, &k.to) == (a, z) || (&k.from, &k.to) == (z, a))
                })
                .collect();
            for c in 0..r.cells.len() {
                let mine = match r.cells[c] {
                    Cell::Room(k) => rooms.contains(&(k as usize)),
                    Cell::Hall(k) => links.contains(&(k as usize)),
                    _ => false,
                };
                if mine {
                    r.basin[c] = Some(b as u16);
                    r.water[c] = (basin.level - r.floors[c]).max(0.0);
                }
            }
        }
        for (g, pocket) in l.gas.iter().enumerate() {
            let room = pocket
                .room
                .as_ref()
                .and_then(|id| l.rooms.iter().position(|r| &r.id == id));
            let link = pocket.link.as_ref().and_then(|(a, z)| {
                l.links
                    .iter()
                    .position(|k| (&k.from, &k.to) == (a, z) || (&k.from, &k.to) == (z, a))
            });
            for c in 0..r.cells.len() {
                let mine = match r.cells[c] {
                    Cell::Room(k) => room == Some(k as usize),
                    Cell::Hall(k) => link == Some(k as usize),
                    _ => false,
                };
                if mine {
                    r.gas[c] = Some(g as u16);
                }
            }
        }
        // Stairs: from the kiosk down to the first flat cell ahead.
        for (k, p) in l.portals.iter().enumerate() {
            let bottom = r.stairs[k].bottom;
            let mut z = bottom;
            let mut found = false;
            while z < bottom + 4.0 {
                if r.at(p.to_site([0.0, z + 0.01])).flat() {
                    found = true;
                    break;
                }
                z += 0.05;
            }
            let foot = if found { z } else { bottom };
            r.stairs[k].foot = foot;
            for j in 0..nz {
                for i in 0..nx {
                    let q = p.to_portal(r.centre(i, j));
                    let c = j * nx + i;
                    if q[0].abs() < STAIR_HALF
                        && q[1] > -KIOSK[1] + WALL * 0.5
                        && q[1] < foot
                        && !r.cells[c].flat()
                    {
                        r.cells[c] = Cell::Stair(k as u16);
                    }
                }
            }
            if p.door && found {
                r.doors.push(DoorSpot {
                    at: p.to_site([0.0, foot]),
                    along: turn([1.0, 0.0], p.rot),
                    width: 2.0 * STAIR_HALF,
                    y: p.floor.unwrap_or(l.floor),
                    room: None,
                    portal: Some(k as u16),
                });
            }
        }
        r
    }

    /// Where a corridor leg from `outer` to `inner` (a room's centre)
    /// crosses the room's edge.
    fn mouth(&self, room: &Room, outer: P2, inner: P2, width: f32) -> Option<DoorSpot> {
        let h = [room.size[0] * 0.5, room.size[1] * 0.5];
        let d = sub(inner, outer);
        if d[0].abs() > d[1].abs() {
            // Along x: the wall it crosses runs along z.
            let x = if d[0] > 0.0 {
                room.at[0] - h[0]
            } else {
                room.at[0] + h[0]
            };
            Some(DoorSpot {
                at: [x, outer[1]],
                along: [0.0, 1.0],
                width,
                y: self.floor,
                room: None,
                portal: None,
            })
        } else if d[1].abs() > 0.0 {
            let z = if d[1] > 0.0 {
                room.at[1] - h[1]
            } else {
                room.at[1] + h[1]
            };
            Some(DoorSpot {
                at: [outer[0], z],
                along: [1.0, 0.0],
                width,
                y: self.floor,
                room: None,
                portal: None,
            })
        } else {
            None
        }
    }

    fn fill(&mut self, lo: P2, hi: P2, cell: Cell, height: f32, floor: f32) {
        for j in 0..self.nz {
            for i in 0..self.nx {
                let c = self.centre(i, j);
                if c[0] > lo[0] && c[0] < hi[0] && c[1] > lo[1] && c[1] < hi[1] {
                    let k = j * self.nx + i;
                    self.cells[k] = cell;
                    self.ceil[k] = height;
                    self.floors[k] = floor;
                }
            }
        }
    }

    pub fn centre(&self, i: usize, j: usize) -> P2 {
        [
            self.origin[0] + (i as f32 + 0.5) * CELL,
            self.origin[1] + (j as f32 + 0.5) * CELL,
        ]
    }

    pub fn cell_of(&self, p: P2) -> Option<(usize, usize)> {
        let i = ((p[0] - self.origin[0]) / CELL).floor();
        let j = ((p[1] - self.origin[1]) / CELL).floor();
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.nx && (j as usize) < self.nz)
            .then_some((i as usize, j as usize))
    }

    pub fn at(&self, p: P2) -> Cell {
        self.cell_of(p)
            .map_or(Cell::Rock, |(i, j)| self.cells[j * self.nx + i])
    }

    pub fn get(&self, i: usize, j: usize) -> Cell {
        self.cells[j * self.nx + i]
    }

    /// Water over the floor at a site-space point (0: dry).
    pub fn water_at(&self, p: P2) -> f32 {
        self.cell_of(p)
            .map_or(0.0, |(i, j)| self.water[j * self.nx + i])
    }

    /// The floor under a site-space point, if it's an open cell.
    /// The basin a site point's water belongs to, if any.
    pub fn gas_at(&self, p: P2) -> Option<u16> {
        self.cell_of(p).and_then(|(i, j)| self.gas[j * self.nx + i])
    }

    pub fn basin_at(&self, p: P2) -> Option<u16> {
        self.cell_of(p)
            .and_then(|(i, j)| self.basin[j * self.nx + i])
    }

    pub fn floor_at(&self, p: P2) -> Option<f32> {
        match self.at(p) {
            Cell::Rock => None,
            Cell::Room(_) | Cell::Hall(_) => {
                let (i, j) = self.cell_of(p)?;
                Some(self.floors[j * self.nx + i])
            }
            Cell::Stair(k) => {
                let k = k as usize;
                Some(self.stairs[k].floor_at(self.portals[k].to_portal(p)[1]))
            }
        }
    }

    /// The top of the roof over an open cell, site y (rock stands on it).
    pub fn roof_at(&self, p: P2) -> Option<f32> {
        let (i, j) = self.cell_of(p)?;
        match self.get(i, j) {
            Cell::Rock => None,
            Cell::Room(_) | Cell::Hall(_) => {
                let c = j * self.nx + i;
                Some(self.floors[c] + self.ceil[c] + SLAB)
            }
            Cell::Stair(k) => {
                let k = k as usize;
                let z = self.portals[k].to_portal(p)[1];
                (z > KIOSK[1] + WALL * 0.5).then(|| self.stairs[k].ceiling_at(z) + SLAB)
            }
        }
    }

    /// Whether a site-space point is under a kiosk (on the surface).
    pub fn in_kiosk(&self, p: P2) -> bool {
        self.portals.iter().any(|portal| {
            let q = portal.to_portal(p);
            q[0].abs() <= KIOSK[0] + WALL * 0.5 + APRON
                && q[1].abs() <= KIOSK[1] + WALL * 0.5 + APRON
        })
    }

    /// Open cells reachable from the entrance's landing (4-connected).
    pub fn flood(&self) -> Vec<bool> {
        self.flood_past(&[])
    }

    /// The same, with these doors shut (locked: the cells across their
    /// opening count as rock).
    pub fn flood_past(&self, shut: &[DoorSpot]) -> Vec<bool> {
        let blocked = |i: usize, j: usize| {
            let p = self.centre(i, j);
            shut.iter().any(|d| {
                let off = sub(p, d.at);
                let along = off[0] * d.along[0] + off[1] * d.along[1];
                let across = off[0] * d.along[1] - off[1] * d.along[0];
                along.abs() < d.width * 0.5 && across.abs() < CELL
            })
        };
        let mut seen = vec![false; self.cells.len()];
        let Some(start) = self
            .portals
            .first()
            .and_then(|p| self.cell_of(p.to_site([0.0, -2.5])))
        else {
            return seen;
        };
        let mut stack = vec![start];
        seen[start.1 * self.nx + start.0] = self.get(start.0, start.1).open();
        while let Some((i, j)) = stack.pop() {
            for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let (a, b) = (i as i64 + di, j as i64 + dj);
                if a < 0 || b < 0 || a >= self.nx as i64 || b >= self.nz as i64 {
                    continue;
                }
                let k = b as usize * self.nx + a as usize;
                if !seen[k] && self.cells[k].open() && !blocked(a as usize, b as usize) {
                    seen[k] = true;
                    stack.push((a as usize, b as usize));
                }
            }
        }
        seen
    }
}

/// What's wrong with a layout (nothing, if it's fit to build).
pub fn check(l: &Layout) -> Vec<String> {
    let mut bad = Vec::new();
    let ids: Vec<&str> = l.rooms.iter().map(|r| r.id.as_str()).collect();
    for (i, id) in ids.iter().enumerate() {
        if ids[..i].contains(id) {
            bad.push(format!("room `{id}` twice"));
        }
    }
    for link in &l.links {
        for end in [&link.from, &link.to] {
            if !ids.contains(&end.as_str()) {
                bad.push(format!("link to no room `{end}`"));
            }
        }
        let room = |id: &str| l.rooms.iter().find(|r| r.id == id).map(|r| r.at);
        if let (Some(a), Some(b)) = (room(&link.from), room(&link.to)) {
            let mut pts = vec![a];
            pts.extend(link.via.iter().copied());
            pts.push(b);
            for w in pts.windows(2) {
                if (w[0][0] - w[1][0]).abs() > 1e-3 && (w[0][1] - w[1][1]).abs() > 1e-3 {
                    bad.push(format!(
                        "{} -> {}: leg {:?} -> {:?} runs neither along x nor z",
                        link.from, link.to, w[0], w[1]
                    ));
                }
            }
        }
        if link.width < 1.4 {
            bad.push(format!("{} -> {}: under 1.4 m wide", link.from, link.to));
        }
    }
    match l.portals.first() {
        None => bad.push("no entrance".into()),
        Some(p) if p.at != [0.0, 0.0] || p.rot != 0.0 || p.y != 0.0 => {
            bad.push("the entrance must be at the origin, unturned, at y 0".into())
        }
        _ => {}
    }
    for (k, p) in l.portals.iter().enumerate() {
        if (p.rot / 90.0).fract().abs() > 1e-3 {
            bad.push(format!("portal {k}: turned {}, not a right angle", p.rot));
        }
    }
    let r = Raster::build(l);
    for (k, s) in r.stairs.iter().enumerate() {
        if s.foot <= s.bottom - 1e-3 || !r.at(r.portals[k].to_site([0.0, s.foot + 0.01])).flat() {
            bad.push(format!(
                "portal {k}: its stair meets no room within 4 m of its foot"
            ));
        }
    }
    // Planar: a stair never passes over or under another open cell (the
    // bots' grid is one level).
    for (k, p) in l.portals.iter().enumerate() {
        let s = &r.stairs[k];
        for j in 0..r.nz {
            for i in 0..r.nx {
                let q = p.to_portal(r.centre(i, j));
                let near = q[0].abs() < KIOSK[0] + WALL && q[1] > -KIOSK[1] - WALL && q[1] < s.foot;
                if near && r.get(i, j).flat() {
                    bad.push(format!("portal {k}: its kiosk or stair is over a room"));
                    break;
                }
            }
        }
    }
    for h in &l.anomalies {
        if h.kind().is_none() {
            bad.push(format!("an anomaly of no kind `{}`", h.kind));
        }
        match l.rooms.iter().find(|r| r.id == h.room) {
            None => bad.push(format!("an anomaly in no room `{}`", h.room)),
            Some(room) => {
                // Inside its room, clear of the doorways: a hazard you go
                // round, not one you must walk through.
                let at = add(room.at, h.at);
                let inside = (at[0] - room.at[0]).abs() + h.radius <= room.size[0] * 0.5 + 0.3
                    && (at[1] - room.at[1]).abs() + h.radius <= room.size[1] * 0.5 + 0.3;
                let clear = r.mouths.iter().all(|m| {
                    let d = sub(at, m.at);
                    (d[0] * d[0] + d[1] * d[1]).sqrt() > h.radius + m.width * 0.5 + 0.6
                });
                if !inside || !clear {
                    bad.push(format!(
                        "{}: its anomaly fills a doorway or leaves the room",
                        h.room
                    ));
                }
            }
        }
    }
    for (stage, p) in l.power.iter().enumerate() {
        match l.rooms.iter().find(|r| r.id == p.room) {
            None => bad.push(format!("power in no room `{}`", p.room)),
            Some(room) => {
                let at = add(room.at, p.at);
                if !r.at(at).open() {
                    bad.push(format!("{}: its power switch is in the rock", room.id));
                }
                // Facing into the room (its turned -Z): you stand there.
                let front = add(at, turn([0.0, -0.9], p.rot));
                if !r.at(front).flat() {
                    bad.push(format!("{}: its power switch faces the wall", room.id));
                }
                // Reached with this stage's doors and every later one's
                // still shut.
                let shut: Vec<DoorSpot> = r
                    .doors
                    .iter()
                    .filter(|d| l.power[stage..].iter().any(|q| q.holds(l, d)))
                    .copied()
                    .collect();
                let seen = r.flood_past(&shut);
                if !r.cell_of(front).is_some_and(|(i, j)| seen[j * r.nx + i]) {
                    bad.push(format!(
                        "{}: power stage {stage} is behind its own or a later stage's door",
                        room.id
                    ));
                }
            }
        }
        for id in &p.doors {
            if let Some(k) = id.strip_prefix("portal") {
                if k.parse::<usize>()
                    .ok()
                    .is_none_or(|k| k == 0 || k >= l.portals.len())
                {
                    bad.push(format!("a powered door on no exit `{id}`"));
                }
                continue;
            }
            let Some(k) = l.rooms.iter().position(|r| &r.id == id) else {
                bad.push(format!("a powered door into no room `{id}`"));
                continue;
            };
            if !r.doors.iter().any(|d| d.room == Some(k as u16)) {
                bad.push(format!("no door into `{id}` for the power to hold"));
            }
        }
    }
    for room in l.rooms.iter().filter(|r| r.lamp == Lamp::Powered) {
        if room.stage >= l.power.len() {
            bad.push(format!(
                "{}: its lamp's on power stage {} of none",
                room.id, room.stage
            ));
        }
    }
    for id in l.rounds.iter().flat_map(|r| &r.rooms) {
        if !l.rooms.iter().any(|r| &r.id == id) {
            bad.push(format!("a round through no room `{id}`"));
        }
    }
    for basin in &l.basins {
        for id in &basin.rooms {
            if !l.rooms.iter().any(|r| &r.id == id) {
                bad.push(format!("basin `{}` over no room `{id}`", basin.id));
            }
        }
        for (a, z) in &basin.links {
            if !l
                .links
                .iter()
                .any(|k| (&k.from, &k.to) == (a, z) || (&k.from, &k.to) == (z, a))
            {
                bad.push(format!("basin `{}` over no corridor {a}-{z}", basin.id));
            }
        }
    }
    for (g, pocket) in l.gas.iter().enumerate() {
        if !r.gas.contains(&Some(g as u16)) {
            bad.push(format!(
                "gas pocket {g} over no room or corridor: {pocket:?}"
            ));
        }
    }
    for (k, rail) in l.rails.iter().enumerate() {
        for w in rail.windows(2) {
            if w[0][0] != w[1][0] && w[0][1] != w[1][1] {
                bad.push(format!(
                    "rail {k}: a leg {:?} -> {:?} neither along x nor z",
                    w[0], w[1]
                ));
            }
        }
        let len = length(rail);
        let mut s = 0.0;
        while s <= len {
            if !r.at(along(rail, s)).flat() {
                bad.push(format!(
                    "rail {k} runs into the rock at {:?}",
                    along(rail, s)
                ));
                break;
            }
            s += CELL;
        }
    }
    for c in &l.carts {
        match l.rails.get(c.rail) {
            None => bad.push(format!("a cart on no rail {}", c.rail)),
            Some(rail) if c.at < 0.0 || c.at > length(rail) => {
                bad.push(format!("a cart off the end of rail {}", c.rail))
            }
            _ => {}
        }
    }
    for it in &l.items {
        match l.rooms.iter().find(|r| r.id == it.room) {
            Some(room) if r.at(add(room.at, it.at)).open() => {}
            _ => bad.push(format!(
                "item `{}` in the rock or no room `{}`",
                it.id, it.room
            )),
        }
    }
    for sh in &l.shoring {
        if link_points(l, &sh.link.0, &sh.link.1).is_none() || !(0.0..=1.0).contains(&sh.along) {
            bad.push(format!("shoring in no corridor {:?}", sh.link));
        }
    }
    for d in &l.drains {
        let Some(basin) = l.basins.iter().find(|b| b.id == d.basin) else {
            bad.push(format!("a drain for no basin `{}`", d.basin));
            continue;
        };
        if d.to > basin.level || d.rate <= 0.0 {
            bad.push(format!("a drain on `{}` that can't lower it", d.basin));
        }
        match l.rooms.iter().find(|r| r.id == d.room) {
            None => bad.push(format!("a drain in no room `{}`", d.room)),
            Some(room) => {
                let at = add(room.at, d.at);
                let front = add(at, turn([0.0, -0.9], d.rot));
                if !r.at(at).open() || !r.at(front).flat() {
                    bad.push(format!(
                        "{}: its drain is in the rock or faces the wall",
                        room.id
                    ));
                }
            }
        }
    }
    for f in &l.floods {
        let found = match (&f.room, &f.link) {
            (Some(id), None) => l.rooms.iter().any(|r| &r.id == id),
            (None, Some((a, b))) => l
                .links
                .iter()
                .any(|k| (&k.from, &k.to) == (a, b) || (&k.from, &k.to) == (b, a)),
            _ => false,
        };
        if !found {
            bad.push(format!("a flood in no one room or corridor: {f:?}"));
        }
        if !(0.2..=1.6).contains(&f.depth) {
            bad.push(format!("a flood {} m deep", f.depth));
        }
    }
    // Room to breathe over the water (a head afloat stands 0.35 m out).
    for (c, w) in r.water.iter().enumerate() {
        if *w > 0.0 && r.ceil[c] - w < 0.8 {
            bad.push(format!("water {w} m deep under a {} m ceiling", r.ceil[c]));
            break;
        }
    }
    for id in &l.patrol {
        if !l.rooms.iter().any(|r| &r.id == id) {
            bad.push(format!("a patrol through no room `{id}`"));
        }
    }
    if l.crawlers > 0 && l.patrol.len() < 2 {
        bad.push("crawlers with no round to walk".into());
    }
    // Nowhere a step higher than a stair's riser between neighbouring
    // floor cells (a corridor between levels too short for its drop).
    'steps: for j in 0..r.nz {
        for i in 0..r.nx {
            let c = j * r.nx + i;
            if !r.cells[c].flat() {
                continue;
            }
            for (a, b) in [(i + 1, j), (i, j + 1)] {
                if a >= r.nx || b >= r.nz || !r.get(a, b).flat() {
                    continue;
                }
                let d = (r.floors[c] - r.floors[b * r.nx + a]).abs();
                if d > STEP_RISE + 0.01 {
                    bad.push(format!("a step of {d:.2} m at {:?}", r.centre(i, j)));
                    break 'steps;
                }
            }
        }
    }
    let seen = r.flood();
    for (k, room) in l.rooms.iter().enumerate() {
        let reached = (0..r.cells.len()).any(|c| seen[c] && r.cells[c] == Cell::Room(k as u16));
        if !reached {
            bad.push(format!(
                "room `{}` can't be reached from the entrance",
                room.id
            ));
        }
    }
    for (k, p) in l.portals.iter().enumerate().skip(1) {
        let reached = r
            .cell_of(p.to_site([0.0, -2.5]))
            .is_some_and(|(i, j)| seen[j * r.nx + i]);
        if !reached {
            bad.push(format!("portal {k} can't be reached from the entrance"));
        }
    }
    for room in &l.rooms {
        for prop in &room.props {
            let at = add(room.at, prop.at);
            if !r.at(at).open() {
                bad.push(format!(
                    "{}: a {:?} in the rock at {at:?}",
                    room.id, prop.kind
                ));
            }
            // Clear of every corridor's mouth and every stair's foot.
            let feet = r.stairs.iter().zip(&r.portals).map(|(s, p)| DoorSpot {
                at: p.to_site([0.0, s.foot]),
                along: turn([1.0, 0.0], p.rot),
                width: 2.0 * STAIR_HALF,
                y: l.floor,
                room: None,
                portal: None,
            });
            for m in r.mouths.iter().copied().chain(feet) {
                let off = sub(at, m.at);
                let along = off[0] * m.along[0] + off[1] * m.along[1];
                let across = off[0] * m.along[1] - off[1] * m.along[0];
                if along.abs() < m.width * 0.5 + 0.5 && across.abs() < 1.2 {
                    bad.push(format!(
                        "{}: a {:?} at {at:?} in the way of a doorway at {:?}",
                        room.id, prop.kind, m.at
                    ));
                }
            }
        }
    }
    bad
}

/// A site placed on a map: its id and pose, and its portals in the world.
#[derive(Clone, Debug)]
pub struct Placed {
    pub id: String,
    pub at: P2,
    pub rot: f32,
}

impl Placed {
    pub fn to_world(&self, p: P2) -> P2 {
        add(self.at, turn(p, self.rot))
    }

    pub fn to_site(&self, w: P2) -> P2 {
        turn(sub(w, self.at), -self.rot)
    }

    /// Kiosks on the surface past the entrance (which is the placement):
    /// world centre and yaw.
    pub fn exits(&self) -> Vec<(P2, f32, &'static Portal)> {
        layout(&self.id)
            .map(|l| {
                l.portals
                    .iter()
                    .skip(1)
                    .map(|p| (self.to_world(p.at), self.rot + p.rot, p))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The underground sites a map places.
pub fn placed(map: &MapDef) -> Vec<Placed> {
    map.placements
        .iter()
        .filter(|p| is_site(&p.prefab))
        .map(|p| Placed {
            id: p.prefab.clone(),
            at: p.at,
            rot: p.rot,
        })
        .collect()
}

/// Half extents of a kiosk's shell, walls and all: what the robots walk
/// round (its apron they walk over).
pub const SHELL_HALF: P2 = [KIOSK[0] + WALL * 0.5, KIOSK[1] + WALL * 0.5];

/// Every kiosk of the map's sites, the entrances' too: (centre, rot).
pub fn kiosks(map: &MapDef) -> Vec<(P2, f32)> {
    placed(map)
        .iter()
        .flat_map(|s| {
            std::iter::once((s.at, s.rot)).chain(s.exits().into_iter().map(|e| (e.0, e.1)))
        })
        .collect()
}

/// Whether a world point's heightmap cell is opened up for a stairwell:
/// the cell's centre is within [`HOLE_PAD`] of a kiosk's stairwell.
pub fn in_stairwell(sites: &[Placed], p: P2) -> bool {
    sites.iter().any(|s| {
        let Some(l) = layout(&s.id) else {
            return false;
        };
        let local = s.to_site(p);
        l.portals.iter().any(|portal| {
            let q = portal.to_portal(local);
            q[0].abs() < STAIR_HALF + HOLE_PAD
                && q[1] > STAIR_TOP - HOLE_PAD
                && q[1] < KIOSK[1] - WALL * 0.5 + HOLE_PAD
        })
    })
}

/// A plan of a site as SVG: rooms, corridors and stairs, doors, props
/// (loot in gold), lamps, and the kiosks on the surface.
pub fn plan_svg(id: &str) -> Option<String> {
    let l = layout(id)?;
    let r = raster(id)?;
    let s = 12.0; // px per metre
    let (w, h) = (r.nx as f32 * CELL * s, r.nz as f32 * CELL * s);
    let px = |p: P2| ((p[0] - r.origin[0]) * s, (p[1] - r.origin[1]) * s);
    let mut out = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{w:.0}' height='{:.0}' \
         font-family='sans-serif' font-size='11'>\n<rect width='100%' height='100%' fill='#2b2a28'/>\n",
        h + 40.0
    );
    for j in 0..r.nz {
        for i in 0..r.nx {
            let colour = match r.get(i, j) {
                Cell::Rock => continue,
                _ if r.water[j * r.nx + i] >= DEEP => "#2e6f8e",
                _ if r.water[j * r.nx + i] > 0.0 => "#6fa3b8",
                _ if r.gas[j * r.nx + i].is_some() => "#a8b44c",
                Cell::Room(_) => "#b9b3a6",
                Cell::Hall(_) => "#8f8a80",
                Cell::Stair(_) => "#6f8fa6",
            };
            let (x, y) = px(r.centre(i, j));
            out += &format!(
                "<rect x='{:.1}' y='{:.1}' width='{:.1}' height='{:.1}' fill='{colour}'/>\n",
                x - CELL * s * 0.5,
                y - CELL * s * 0.5,
                CELL * s,
                CELL * s
            );
        }
    }
    for (k, p) in r.portals.iter().enumerate() {
        let c = [
            p.to_site([-KIOSK[0], -KIOSK[1]]),
            p.to_site([KIOSK[0], -KIOSK[1]]),
            p.to_site([KIOSK[0], KIOSK[1]]),
            p.to_site([-KIOSK[0], KIOSK[1]]),
        ];
        let pts: Vec<String> = c
            .iter()
            .map(|q| {
                let (x, y) = px(*q);
                format!("{x:.1},{y:.1}")
            })
            .collect();
        out += &format!(
            "<polygon points='{}' fill='none' stroke='#e8e2d0' stroke-width='2' stroke-dasharray='4 3'/>\n",
            pts.join(" ")
        );
        let (x, y) = px(p.to_site([0.0, -KIOSK[1] - 1.2]));
        let label = if k == 0 { "IN" } else { "OUT" };
        out += &format!(
            "<text x='{x:.1}' y='{y:.1}' fill='#e8e2d0' text-anchor='middle'>{label} ({:+.1} m)</text>\n",
            p.y
        );
    }
    for d in &r.doors {
        let a = sub(
            d.at,
            [d.along[0] * d.width * 0.5, d.along[1] * d.width * 0.5],
        );
        let b = add(
            d.at,
            [d.along[0] * d.width * 0.5, d.along[1] * d.width * 0.5],
        );
        let ((x1, y1), (x2, y2)) = (px(a), px(b));
        out += &format!(
            "<line x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}' stroke='#c0392b' stroke-width='4'/>\n"
        );
    }
    for (stage, p) in l.power.iter().enumerate() {
        if let Some(room) = l.rooms.iter().find(|r| r.id == p.room) {
            let (x, y) = px(add(room.at, p.at));
            out += &format!(
                "<rect x='{:.1}' y='{:.1}' width='10' height='10' fill='#f1c40f' stroke='#1d1c1a'/>\n\
                 <text x='{:.1}' y='{:.1}' fill='#f1c40f'>{:?} {}</text>\n",
                x - 5.0,
                y - 5.0,
                x + 8.0,
                y + 4.0,
                p.kind,
                stage + 1
            );
        }
        // The doors on the power, drawn over in yellow.
        for d in r.doors.iter().filter(|d| p.holds(l, d)) {
            let a = sub(
                d.at,
                [d.along[0] * d.width * 0.5, d.along[1] * d.width * 0.5],
            );
            let b = add(
                d.at,
                [d.along[0] * d.width * 0.5, d.along[1] * d.width * 0.5],
            );
            let ((x1, y1), (x2, y2)) = (px(a), px(b));
            out += &format!(
                "<line x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}' stroke='#f1c40f' stroke-width='4'/>\n"
            );
        }
    }
    // Rails (brown), carts on them (C), shoring (S).
    for rail in &l.rails {
        for w in rail.windows(2) {
            let ((x1, y1), (x2, y2)) = (px(w[0]), px(w[1]));
            out += &format!(
                "<line x1='{x1:.1}' y1='{y1:.1}' x2='{x2:.1}' y2='{y2:.1}' stroke='#7b4a2a' stroke-width='3' stroke-dasharray='6 3'/>\n"
            );
        }
    }
    for c in &l.carts {
        if let Some(rail) = l.rails.get(c.rail) {
            let (x, y) = px(along(rail, c.at));
            out += &format!(
                "<rect x='{:.1}' y='{:.1}' width='12' height='16' fill='#7b4a2a' stroke='#1d1c1a'/>\n\
                 <text x='{:.1}' y='{:.1}' fill='#e0c9a6'>C</text>\n",
                x - 6.0,
                y - 8.0,
                x - 3.5,
                y + 4.0
            );
        }
    }
    for sh in &l.shoring {
        if let Some(pts) = link_points(l, &sh.link.0, &sh.link.1) {
            let (x, y) = px(along(&pts, length(&pts) * sh.along));
            out += &format!(
                "<rect x='{:.1}' y='{:.1}' width='14' height='6' fill='#c8a26a' stroke='#1d1c1a'/>\n\
                 <text x='{:.1}' y='{:.1}' fill='#c8a26a'>S</text>\n",
                x - 7.0,
                y - 3.0,
                x + 9.0,
                y + 4.0
            );
        }
    }
    // Drains: a valve wheel (V) or the hand pump (P), and the basin each
    // lowers.
    for d in &l.drains {
        if let Some(room) = l.rooms.iter().find(|r| r.id == d.room) {
            let (x, y) = px(add(room.at, d.at));
            let mark = if d.kind == DrainKind::Valve { "V" } else { "P" };
            out += &format!(
                "<circle cx='{x:.1}' cy='{y:.1}' r='6' fill='#5dade2' stroke='#1d1c1a'/>\n\
                 <text x='{:.1}' y='{:.1}' fill='#5dade2'>{mark} {} to {:.1}</text>\n",
                x + 8.0,
                y + 4.0,
                d.basin,
                d.to
            );
        }
    }
    for room in &l.rooms {
        for p in &room.props {
            let (x, y) = px(add(room.at, p.at));
            let fill = if p.loot.is_some() {
                "#d4a017"
            } else {
                "#5a5650"
            };
            out += &format!("<circle cx='{x:.1}' cy='{y:.1}' r='4' fill='{fill}'/>\n");
        }
        let (x, y) = px(room.at);
        let lamp = match room.lamp {
            Lamp::None => "",
            Lamp::Steady => " *",
            Lamp::Flicker => " ~",
            Lamp::Dead => " x",
            Lamp::Powered => " +",
        };
        out += &format!(
            "<text x='{x:.1}' y='{:.1}' fill='#1d1c1a' text-anchor='middle'>{}{lamp}</text>\n",
            y - 6.0,
            room.id
        );
    }
    out += &format!(
        "<text x='8' y='{:.0}' fill='#e8e2d0'>{} ({id}): floor {:.1} m; grid {:.1} m; \
         gold = loot, red = door (yellow: on the power), blue = stair, teal = water (dark: too deep to stand), olive = firedamp; lamp * on, ~ flickering, x dead, + on the power</text>\n</svg>\n",
        h + 24.0,
        l.name,
        l.floor,
        CELL
    );
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::Heightmap;

    fn maps() -> Vec<MapDef> {
        [
            crate::map::MARROW_CREEK,
            crate::map::KETTLE_PASS,
            crate::map::CINDER_WORKS,
        ]
        .iter()
        .map(|t| MapDef::from_ron(t).unwrap())
        .collect()
    }

    /// Every layout parses and is fit to build: rooms reached from the
    /// entrance, stairs that land somewhere, nothing in the rock, every
    /// loot table one that exists.
    #[test]
    fn every_layout_is_sound() {
        let cat = crate::items::ItemCatalog::from_ron(crate::items::BUILTIN_CATALOG).unwrap();
        let tables = crate::loot::LootTables::from_ron(crate::loot::BUILTIN_LOOT, &cat).unwrap();
        for (id, _) in SITES {
            let l = layout(id).unwrap();
            for room in &l.rooms {
                for p in &room.props {
                    if let Some(t) = &p.loot {
                        assert!(tables.has(t), "{id} {}: no loot table `{t}`", room.id);
                    }
                }
            }
            let bad = check(l);
            assert!(bad.is_empty(), "{id}:\n{}", bad.join("\n"));
            let r = raster(id).unwrap();
            let rooms = l.rooms.len();
            assert!((6..=14).contains(&rooms), "{id}: {rooms} rooms");
            assert!(r.cells.iter().filter(|c| c.open()).count() > 100);
        }
    }

    /// The check walks the power in its stages: a switch behind its own
    /// stage's door (or a later one's) can never be thrown.
    #[test]
    fn power_stages_are_reached_in_order() {
        let mut l = layout("mine_kettle").unwrap().clone();
        assert!(check(&l).is_empty());
        let mut later = l.power[0].clone();
        later.room = "magazine".into();
        later.doors = vec!["magazine".into()];
        l.power.push(later);
        let bad = check(&l);
        assert!(
            bad.iter().any(|b| b.contains("power stage 1 is behind")),
            "{bad:?}"
        );
        // The same switch, its door on the stage before: fine.
        l.power[1].doors.clear();
        let bad = check(&l);
        assert!(!bad.iter().any(|b| b.contains("power stage")), "{bad:?}");
    }

    /// A site's corridors make loops: more ways between rooms than a tree
    /// would need, so there's always another way round.
    #[test]
    fn every_site_has_a_loop() {
        for (id, _) in SITES {
            let l = layout(id).unwrap();
            // Rooms joined directly (sharing an edge) count as linked.
            let r = raster(id).unwrap();
            let mut edges = std::collections::HashSet::new();
            for link in &l.links {
                let a = l.rooms.iter().position(|x| x.id == link.from).unwrap();
                let b = l.rooms.iter().position(|x| x.id == link.to).unwrap();
                edges.insert((a.min(b), a.max(b)));
            }
            for j in 0..r.nz - 1 {
                for i in 0..r.nx - 1 {
                    for (a, b) in [
                        (r.get(i, j), r.get(i + 1, j)),
                        (r.get(i, j), r.get(i, j + 1)),
                    ] {
                        if let (Cell::Room(a), Cell::Room(b)) = (a, b) {
                            if a != b {
                                edges.insert((a.min(b) as usize, a.max(b) as usize));
                            }
                        }
                    }
                }
            }
            assert!(
                edges.len() >= l.rooms.len(),
                "{id}: {} joins between {} rooms: no loop",
                edges.len(),
                l.rooms.len()
            );
            assert!(
                l.portals.len() >= 2 || id == "none",
                "{id}: one way in and out"
            );
        }
    }

    /// Every site on the valleys lies under the ground: rock over every
    /// ceiling past the kiosks, and the exits' ground near where the
    /// layout says.
    #[test]
    fn every_site_is_under_rock() {
        let mut found = 0;
        let mut bad = Vec::new();
        for map in maps() {
            let hm = Heightmap::build(&map);
            for site in placed(&map) {
                found += 1;
                let r = raster(&site.id).unwrap();
                let footing = hm.footing(&site.id, site.at, site.rot);
                let mut worst = (f32::INFINITY, [0.0; 2]);
                for j in 0..r.nz {
                    for i in 0..r.nx {
                        let c = r.centre(i, j);
                        if r.in_kiosk(c) {
                            continue;
                        }
                        let Some(roof) = r.roof_at(c) else { continue };
                        let w = site.to_world(c);
                        let cover = hm.height(w) - (footing + roof);
                        if cover < worst.0 {
                            worst = (cover, w);
                        }
                    }
                }
                if worst.0 < COVER {
                    bad.push(format!(
                        "{} {}: {:.2} m of ground over the roof at {:.1?}",
                        map.name, site.id, worst.0, worst.1
                    ));
                }
                for (at, rot, p) in site.exits() {
                    let want = footing + p.y;
                    let got = hm.footing(&site.id, at, rot);
                    if (got - want).abs() > 0.3 {
                        bad.push(format!(
                            "{} {}: the exit at {at:.1?} stands at {got:.2}, the layout wants {want:.2}",
                            map.name, site.id
                        ));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        assert!(found >= 3, "{found} sites placed");
    }

    /// The terrain is opened only inside the kiosks (their walls and
    /// aprons cover the edge), and opened right over every stairwell.
    #[test]
    fn holes_only_under_kiosks() {
        for map in maps() {
            let hm = Heightmap::build(&map);
            let sites = placed(&map);
            for (k, hole) in hm.holes.iter().enumerate() {
                if !hole {
                    continue;
                }
                let (i, j) = (k % hm.n, k / hm.n);
                let p = hm.pos(i, j);
                let centre = [p[0] + hm.cell * 0.5, p[1] + hm.cell * 0.5];
                let inside = sites.iter().any(|s| {
                    let r = raster(&s.id).unwrap();
                    let l = s.to_site(centre);
                    r.portals.iter().any(|portal| {
                        let q = portal.to_portal(l);
                        // The far corner of a cell whose centre is in.
                        let corner = hm.cell * std::f32::consts::FRAC_1_SQRT_2;
                        q[0].abs() + corner < KIOSK[0] + WALL * 0.5 + APRON
                            && q[1].abs() + corner < KIOSK[1] + WALL * 0.5 + APRON
                    })
                });
                assert!(inside, "{}: a hole at {centre:?} outside a kiosk", map.name);
            }
            for s in &sites {
                let r = raster(&s.id).unwrap();
                for portal in &r.portals {
                    // Down the middle of the stairwell, inside the kiosk.
                    let mut z = STAIR_TOP;
                    while z < KIOSK[1] - WALL {
                        let w = s.to_world(portal.to_site([0.0, z]));
                        let (i, j) = (
                            ((w[0] - hm.origin) / hm.cell).floor() as usize,
                            ((w[1] - hm.origin) / hm.cell).floor() as usize,
                        );
                        assert!(
                            hm.holes[hm.idx(i, j)],
                            "{}: ground across the stairwell at {w:?}",
                            map.name
                        );
                        z += 0.25;
                    }
                }
            }
        }
    }

    /// The plans draw (and `PLAN_DIR=docs/underground` writes them).
    #[test]
    fn plans_draw() {
        for (id, _) in SITES {
            let svg = plan_svg(id).unwrap();
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>\n"));
            if let Ok(dir) = std::env::var("PLAN_DIR") {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(format!("{dir}/{id}.svg"), svg).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod scout {
    use super::*;
    use crate::terrain::Heightmap;

    /// Where a site could go on a map: `SCOUT=map:site:x:z:radius cargo
    /// test -p extraction_core scout -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scout() {
        let arg = std::env::var("SCOUT").unwrap();
        let v: Vec<&str> = arg.split(':').collect();
        let text = match v[0] {
            "marrow_creek" => crate::map::MARROW_CREEK,
            "kettle_pass" => crate::map::KETTLE_PASS,
            _ => crate::map::CINDER_WORKS,
        };
        let id = v[1];
        let (tx, tz, rad): (f32, f32, f32) = (
            v[2].parse().unwrap(),
            v[3].parse().unwrap(),
            v[4].parse().unwrap(),
        );
        let mut map = MapDef::from_ron(text).unwrap();
        let hm0 = Heightmap::build(&map);
        let r = raster(id).unwrap();
        let mut found = Vec::new();
        let mut x = tx - rad;
        while x <= tx + rad {
            let mut z = tz - rad;
            while z <= tz + rad {
                for rot in [0.0f32, 90.0, 180.0, 270.0] {
                    let site = Placed {
                        id: id.into(),
                        at: [x, z],
                        rot,
                    };
                    let footing = hm0.footing(id, [x, z], rot);
                    let mut worst = f32::INFINITY;
                    let mut wet = false;
                    let mut under = 0;
                    for j in (0..r.nz).step_by(2) {
                        for i in (0..r.nx).step_by(2) {
                            let c = r.centre(i, j);
                            let Some(roof) = r.roof_at(c) else { continue };
                            let w = site.to_world(c);
                            if hm0.water_level(w).is_some() {
                                wet = true;
                            }
                            if r.in_kiosk(c) {
                                continue;
                            }
                            worst = worst.min(hm0.height(w) - footing - roof);
                            if map.placements.iter().any(|p| {
                                let f = crate::map::prefab_footprint(&p.prefab);
                                let l = turn([w[0] - p.at[0], w[1] - p.at[1]], -p.rot);
                                l[0].abs() < f[0] * 0.5 + 1.0 && l[1].abs() < f[2] * 0.5 + 1.0
                            }) {
                                under += 1;
                            }
                        }
                    }
                    // Kiosks clear of placements and roads.
                    let mut clash = false;
                    let mut kiosks = vec![([x, z], rot, 0.0)];
                    for (at, krot, p) in site.exits() {
                        kiosks.push((at, krot, hm0.footing(id, at, krot) - footing - p.y));
                    }
                    for (at, krot, _) in &kiosks {
                        for q in [
                            [-2.5, -4.7],
                            [2.5, -4.7],
                            [2.5, 4.7],
                            [-2.5, 4.7],
                            [0.0, 0.0],
                            [0.0, -6.0],
                        ] {
                            let w = add(*at, turn(q, *krot));
                            if hm0.road_at(w).weight > 40
                                || hm0.water_level(w).is_some()
                                || hm0.slope_deg(w) > 14.0
                            {
                                clash = true;
                            }
                            if map.placements.iter().any(|p| {
                                let f = crate::map::prefab_footprint(&p.prefab);
                                let l = turn([w[0] - p.at[0], w[1] - p.at[1]], -p.rot);
                                l[0].abs() < f[0] * 0.5 + 2.0 && l[1].abs() < f[2] * 0.5 + 2.0
                            }) {
                                clash = true;
                            }
                        }
                    }
                    let exit_dy = kiosks.get(1).map_or(0.0, |k| k.2);
                    if !clash && !wet && worst > 1.0 {
                        found.push((worst, exit_dy, under, x, z, rot));
                    }
                }
                z += 4.0;
            }
            x += 4.0;
        }
        found.sort_by(|a, b| {
            let d = |f: &(f32, f32, i32, f32, f32, f32)| (f.3 - tx).hypot(f.4 - tz);
            d(a).total_cmp(&d(b))
        });
        for f in found.iter().take(25) {
            println!(
                "cover {:.1} exit_dy {:+.2} under-buildings {} at ({}, {}) rot {}",
                f.0, f.1, f.2, f.3, f.4, f.5
            );
        }
        map.placements.clear();
    }
}

#[cfg(test)]
mod where_ {
    use super::*;

    /// Prints each placed site's footing and rooms in world space:
    /// `cargo test -p extraction_core where_sites -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn where_sites() {
        for text in [
            crate::map::MARROW_CREEK,
            crate::map::KETTLE_PASS,
            crate::map::CINDER_WORKS,
        ] {
            let map = MapDef::from_ron(text).unwrap();
            let hm = crate::terrain::Heightmap::build(&map);
            for s in placed(&map) {
                let footing = hm.footing(&s.id, s.at, s.rot);
                let l = layout(&s.id).unwrap();
                println!(
                    "{} {} footing {footing:.2} floor {:.2}",
                    map.name,
                    s.id,
                    footing + l.floor
                );
                for r in &l.rooms {
                    let w = s.to_world(r.at);
                    println!("  {:14} ({:.1}, {:.1})", r.id, w[0], w[1]);
                }
                for (k, p) in l.portals.iter().enumerate() {
                    let w = s.to_world(p.at);
                    println!(
                        "  portal {k} ({:.1}, {:.1}) ground {:.2} rot {}",
                        w[0],
                        w[1],
                        footing + p.y,
                        s.rot + p.rot
                    );
                }
            }
        }
    }
}
