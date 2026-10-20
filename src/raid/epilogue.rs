//! The ending, played (`docs/LORE.md`, "The ending"). On the raid after
//! the recall, in Marrow, Lev comes with you: he steps down from the cab
//! and walks behind you, all the way to the Gate and down into the Depot.
//! At the tag office the terminal finds a release procedure now (the
//! review is closed) and the kept crates open; he walks to bay K-14 and
//! finds his mother, and Rusty's collar in the small crate. No machines,
//! no hurry; you leave when you like, and the raid ends at the truck as any
//! does. It happens once a career (`Epilogue::done`, saved).

use bevy::prelude::*;

use super::{Phase, Raid};
use crate::launch::LaunchOptions;
use crate::rig::LocalPlayer;
use crate::world::Zone;
use crate::world::scenes::SceneScreen;

pub struct EpiloguePlugin;

impl Plugin for EpiloguePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Epilogue>()
            .init_resource::<DepotNav>()
            .add_message::<EpilogueBeat>()
            .add_systems(
                Update,
                (begin, plan_depot, walk, release, at_the_bay, home).chain(),
            );
    }
}

/// Where the ending is: only ever played in Marrow, after the recall.
pub const MAP: &str = "marrow_creek";

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Not this raid.
    #[default]
    Off,
    /// He walks behind you.
    Following,
    /// The release went through: he's on his way to K-14.
    ToTheBay,
    /// He's with her.
    Found,
}

/// The ending's state; `done` is kept with the career.
#[derive(Resource, Default, Debug, Clone)]
pub struct Epilogue {
    pub done: bool,
    pub stage: Stage,
    /// The kept crates are open.
    pub crates_open: bool,
    /// His lines at her crate said so far.
    pub said: usize,
}

impl Epilogue {
    /// On the raid it's played (the score falls silent).
    pub fn active(&self) -> bool {
        self.stage != Stage::Off
    }
}

/// Moments of the ending, for sound and anyone else who'd mark them:
/// what, and where (the crates for the opening; him otherwise).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct EpilogueBeat {
    pub kind: BeatKind,
    pub at: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeatKind {
    /// He steps down from the cab.
    Begins,
    /// The release goes through; the crates open.
    CratesOpen,
    /// He's at her crate.
    Found,
}

/// Lev, on foot.
#[derive(Component, Debug, Default)]
pub struct LevWalker {
    path: Vec<Vec3>,
    repath: f32,
    /// Seconds since he last moved (for his lines' timing).
    pub idle: f32,
}

/// How close he keeps behind you, and how fast he walks.
const BEHIND: f32 = 2.2;
const PACE: f32 = 1.35;
/// Near enough to the terminal, you and he, for the release.
const AT_TERMINAL: f32 = 3.0;
const LEV_NEAR: f32 = 7.0;
/// He's with her within this of where he stands.
const AT_BAY: f32 = 0.5;

/// His lines, in order, a few seconds apart where there are several.
pub const SETTING_OFF: &str = "Driver: \"I'm coming in with you. Go slow. My knees.\"";
pub const GOING_DOWN: &str = "Driver: \"So this is where they took it all.\"";
pub const STACKS_SHUT: &str = "Driver: \"The stacks are shut. There'll be a breaker somewhere.\"";
pub const RELEASED: &str = "Driver: \"Next of kin. That's me. Marek, Lev.\"";
pub const FOUND: &[&str] = &[
    "Driver: \"Mum.\"",
    "Driver: \"Rusty's in the small one. I'll take his collar home.\"",
    "Driver: \"Go on up. I'll sit with her a while.\"",
];

/// The logbook's entry for that raid.
pub const LOGBOOK: &str = "WENT IN WITH LEV\nThe Depot, bay K-14.\nHe found her.";

/// What the intake terminal says once the release goes through.
pub const RELEASE_PAGES: &[&str] = &[
    "RELEASE PROCEDURE\n\nReview: closed (recall).\nKept material is released\nto next of kin on request.\n\nRequest   MAREK, L.\nBay K-14  OPENED",
    "KEPT MATERIAL\n\nAll bays: OPENED\nSweep: ended.\n\nNothing is destroyed.",
];

/// Where he stands at her crate (world x, z, floor): beside the long
/// crate in bay K-14, from the scenes' own placing of its tag.
pub fn bay_point() -> Option<Vec3> {
    let s = crate::world::scenes::all()
        .iter()
        .find(|s| s.name == "Marek crate")?;
    Some(Vec3::new(s.at[0] + 0.3, s.floor?, s.at[1] + 0.85))
}

/// On the raid after the recall, Lev steps down and follows.
#[allow(clippy::too_many_arguments)]
fn begin(
    mut commands: Commands,
    mut epi: ResMut<Epilogue>,
    mut raid: ResMut<Raid>,
    opts: Res<LaunchOptions>,
    players: Query<&GlobalTransform, With<LocalPlayer>>,
    walkers: Query<Entity, With<LevWalker>>,
    mut beats: MessageWriter<EpilogueBeat>,
    mut was_out: Local<bool>,
) {
    let out = raid.phase != Phase::AtBase;
    let set_off = out && !*was_out;
    *was_out = out;
    if !set_off || epi.done || !raid.recalled || opts.map != MAP || !walkers.is_empty() {
        return;
    }
    let Some(me) = players.iter().next() else {
        return;
    };
    let at = me.translation() - me.forward().as_vec3() * BEHIND;
    commands.spawn((
        Name::new("Lev"),
        LevWalker::default(),
        Transform::from_translation(at),
        Visibility::default(),
    ));
    epi.stage = Stage::Following;
    raid.say(SETTING_OFF);
    beats.write(EpilogueBeat {
        kind: BeatKind::Begins,
        at,
    });
    info!("epilogue: Lev comes with you");
}

/// Where Lev is going now, and the next step on the way there.
#[allow(clippy::too_many_arguments)]
fn walk(
    epi: Res<Epilogue>,
    zone: Option<Res<Zone>>,
    nav: Option<Res<crate::robots::walker::Nav>>,
    players: Query<&GlobalTransform, (With<LocalPlayer>, Without<LevWalker>)>,
    mut lev: Query<(&mut Transform, &mut LevWalker)>,
    time: Res<Time>,
    mut raid: ResMut<Raid>,
    mut said_down: Local<bool>,
    depot: Res<DepotNav>,
    doors: Query<(Entity, &crate::world::buildings::Door, &GlobalTransform)>,
    player_e: Query<Entity, With<LocalPlayer>>,
    mut press: MessageWriter<crate::interact::InteractEvent>,
) {
    let player = player_e.iter().next();
    let Some(zone) = zone else {
        return;
    };
    if lev.is_empty() {
        *said_down = false;
        return;
    }
    let Ok((mut t, mut w)) = lev.single_mut() else {
        *said_down = false;
        return;
    };
    let dt = time.delta_secs();
    let me = players.iter().next().map(|g| g.translation());
    let (goal, stop) = match epi.stage {
        Stage::Following => match me {
            Some(p) => (p, BEHIND),
            None => return,
        },
        Stage::ToTheBay | Stage::Found => match bay_point() {
            Some(p) => (p, 0.05),
            None => return,
        },
        Stage::Off => return,
    };
    let here = t.translation;
    let below = crate::world::underground::site_under(&zone, here).is_some();
    if below && !*said_down && epi.stage == Stage::Following {
        *said_down = true;
        raid.say(GOING_DOWN);
    }
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    if flat(goal).distance(flat(here)) <= stop && (goal.y - here.y).abs() < 2.0 {
        w.idle += dt;
        w.path.clear();
        return;
    }
    // A fresh path now and then: the Depot's own grid when the goal is
    // down there, the Zone's otherwise.
    w.repath -= dt;
    if w.path.is_empty() || w.repath <= 0.0 {
        w.repath = 1.0;
        w.path = route(&zone, nav.as_deref(), depot.0.as_ref(), here, goal)
            .unwrap_or_else(|| vec![goal]);
    }
    let Some(&next) = w.path.first() else {
        return;
    };
    let d = flat(next) - flat(here);
    let step = PACE * dt;
    let mut to = if d.length() <= step {
        w.path.remove(0);
        next
    } else {
        let s = d.normalize() * step;
        Vec3::new(here.x + s.x, here.y, here.z + s.y)
    };
    // On a floor down there, on the ground up here.
    to.y = crate::world::underground::floor_under(&zone, to.with_y(here.y))
        .or_else(|| crate::world::underground::floor_under(&zone, to.with_y(next.y)))
        .unwrap_or_else(|| zone.ground(to.x, to.z));
    if d.length() > 1e-3 {
        t.rotation = Quat::from_rotation_y(d.x.atan2(d.y) + std::f32::consts::PI);
    }
    t.translation = to;
    w.idle = 0.0;
    // He opens a shut door in his way, as you would (one that's locked
    // stays shut: he waits).
    let ahead = to + t.rotation * Vec3::NEG_Z * 0.6;
    if let Some(player) = player {
        for (e, door, g) in &doors {
            if !door.open && g.translation().distance(ahead) < 1.3 {
                press.write(crate::interact::InteractEvent::Pressed { item: e, player });
            }
        }
    }
}

/// The Depot's walkable plan (its kiosk's ground included).
#[derive(Resource, Default)]
pub struct DepotNav(pub Option<crate::robots::indoor::LocalNav>);

/// Plans the Depot once there's someone to walk it.
fn plan_depot(
    mut depot: ResMut<DepotNav>,
    lev: Query<(), With<LevWalker>>,
    buildings: Query<(Entity, &crate::world::buildings::Building, &Transform)>,
    doors: Query<(Entity, &crate::world::buildings::DoorLeaf, &ChildOf)>,
    mut done: Local<bool>,
) {
    if *done || lev.is_empty() {
        return;
    }
    *done = true;
    depot.0 = crate::sim::indoor::plan_buildings(&buildings, &doors)
        .into_iter()
        .find(|p| p.name == crate::world::underground::DEPOT)
        .map(|plan| crate::robots::indoor::LocalNav {
            grid: std::sync::Arc::new(plan.grid),
            xf: plan.xf,
            floors: plan.levels.map(std::sync::Arc::new),
        });
}

/// A walk from `from` to `to`: through the Depot on its plan when the goal
/// is down there, on the Zone's grid when it's up here.
fn route(
    zone: &Zone,
    nav: Option<&crate::robots::walker::Nav>,
    depot: Option<&crate::robots::indoor::LocalNav>,
    from: Vec3,
    to: Vec3,
) -> Option<Vec<Vec3>> {
    let below = |p: Vec3| crate::world::underground::site_under(zone, p).is_some();
    if let Some(depot) = depot
        && (below(to) || below(from))
        && let Some(p) = depot.path(from, to)
    {
        return Some(p.into_iter().skip(1).collect());
    }
    let nav = nav?;
    nav.0.path([from.x, from.z], [to.x, to.z]).map(|p| {
        p.into_iter()
            .skip(1)
            .map(|q| Vec3::new(q[0], 0.0, q[1]))
            .collect()
    })
}

/// You and he at the tag office terminal: the release goes through, the
/// crates open, and he sets off for K-14 (once the stacks are lit).
#[allow(clippy::too_many_arguments)]
fn release(
    mut epi: ResMut<Epilogue>,
    mut raid: ResMut<Raid>,
    players: Query<&GlobalTransform, (With<LocalPlayer>, Without<LevWalker>)>,
    lev: Query<&Transform, With<LevWalker>>,
    mut screens: Query<(
        &GlobalTransform,
        &SceneScreen,
        &mut crate::world::notes::Terminal,
        &mut crate::raid::bench::ScreenText,
    )>,
    power: Query<(
        &crate::world::buildings::Building,
        &crate::world::underground::SitePower,
    )>,
    mut beats: MessageWriter<EpilogueBeat>,
    mut told_shut: Local<bool>,
) {
    if epi.stage != Stage::Following {
        *told_shut = false;
        return;
    }
    let (Some(me), Ok(lev)) = (players.iter().next(), lev.single()) else {
        return;
    };
    let Some((at, _, mut term, mut text)) = screens
        .iter_mut()
        .find(|(_, s, _, _)| s.scene == "Tag office desk")
    else {
        return;
    };
    let at = at.translation();
    if me.translation().distance(at) > AT_TERMINAL || lev.translation.distance(at) > LEV_NEAR {
        return;
    }
    if !epi.crates_open {
        epi.crates_open = true;
        term.pages = RELEASE_PAGES.iter().map(|p| p.to_string()).collect();
        term.page = 0;
        text.text = term.screen();
        raid.say(RELEASED);
        beats.write(EpilogueBeat {
            kind: BeatKind::CratesOpen,
            at: bay_point().unwrap_or(at),
        });
        info!("epilogue: released");
    }
    // The stacks open on the tag office's own board (the Depot's second
    // stage): he waits for it.
    let lit = power
        .iter()
        .find(|(b, _)| b.prefab == crate::world::underground::DEPOT)
        .is_none_or(|(_, p)| p.on(1));
    if lit {
        epi.stage = Stage::ToTheBay;
    } else if !*told_shut {
        *told_shut = true;
        raid.say(STACKS_SHUT);
    }
}

/// He reaches her: his lines, a few seconds apart, and it's done.
#[allow(clippy::too_many_arguments)]
fn at_the_bay(
    mut epi: ResMut<Epilogue>,
    mut raid: ResMut<Raid>,
    lev: Query<&Transform, With<LevWalker>>,
    time: Res<Time>,
    mut beats: MessageWriter<EpilogueBeat>,
    mut said: Local<(usize, f32)>,
) {
    let Ok(t) = lev.single() else {
        *said = (0, 0.0);
        return;
    };
    let Some(bay) = bay_point() else {
        return;
    };
    if epi.stage == Stage::ToTheBay
        && Vec2::new(t.translation.x - bay.x, t.translation.z - bay.z).length() < AT_BAY
    {
        epi.stage = Stage::Found;
        beats.write(EpilogueBeat {
            kind: BeatKind::Found,
            at: t.translation,
        });
        info!("epilogue: Lev finds her");
        *said = (0, 3.0);
    }
    if epi.stage != Stage::Found || said.0 >= FOUND.len() {
        return;
    }
    said.1 -= time.delta_secs();
    if said.1 <= 0.0 {
        raid.say(FOUND[said.0]);
        *said = (said.0 + 1, 6.0);
        epi.said = said.0;
    }
}

/// Home: the raid ends at the truck as any does. He walked back on his own
/// (he's in the cab); the score comes back.
fn home(
    mut commands: Commands,
    mut epi: ResMut<Epilogue>,
    raid: Res<Raid>,
    lev: Query<Entity, With<LevWalker>>,
    mut logbook: Option<ResMut<super::logbook::Logbook>>,
) {
    if raid.phase != Phase::AtBase || epi.stage == Stage::Off {
        return;
    }
    for e in &lev {
        commands.entity(e).despawn();
    }
    // Played, if he found her (home before that, he'll come again).
    if epi.stage == Stage::Found {
        epi.done = true;
        // The logbook says it plainly (the raid's own entry is written by
        // now: this one stands in its place).
        if let Some(l) = logbook.as_mut() {
            l.written = Some(LOGBOOK.into());
        }
    }
    epi.stage = Stage::Off;
    epi.crates_open = false;
    epi.said = 0;
}

/// Client: Lev's body (`lev_body`, his arm `lev_hand`), boxes until the
/// models are in, and a slow walker's sway.
pub struct EpilogueVisualsPlugin;

impl Plugin for EpilogueVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, dress_lev);
    }
}

fn dress_lev(
    mut commands: Commands,
    added: Query<Entity, Added<LevWalker>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use crate::render::world_props::{Blockout, SwapIn};
    for e in &added {
        let coat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.17, 0.17, 0.19),
            perceptual_roughness: 0.95,
            ..default()
        });
        let skin = materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.42, 0.34),
            perceptual_roughness: 0.75,
            ..default()
        });
        // His body (Kit models') and his right hand (Hands') at its wrist:
        // the forearm hanging, fingers down, palm to his side.
        let hand = Transform::from_xyz(0.25, 0.77, 0.0).with_rotation(
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
                * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        );
        commands.entity(e).insert(SwapIn(vec![
            ("lev_body", Transform::IDENTITY),
            ("lev_hand", hand),
        ]));
        for (size, at, m) in [
            (Vec3::new(0.46, 0.95, 0.3), Vec3::new(0.0, 0.95, 0.0), &coat),
            (Vec3::new(0.36, 0.5, 0.22), Vec3::new(0.0, 0.25, 0.0), &coat),
            (Vec3::new(0.2, 0.24, 0.22), Vec3::new(0.0, 1.6, 0.0), &skin),
            (
                Vec3::new(0.26, 0.06, 0.3),
                Vec3::new(0.0, 1.74, -0.03),
                &coat,
            ),
        ] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
                MeshMaterial3d(m.clone()),
                Transform::from_translation(at),
                Blockout,
                ChildOf(e),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let opts = crate::LaunchOptions {
            robots: false,
            map: MAP.into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        app
    }

    fn lev(app: &mut App) -> Option<Vec3> {
        app.world_mut()
            .query_filtered::<&Transform, With<LevWalker>>()
            .iter(app.world())
            .next()
            .map(|t| t.translation)
    }

    fn set_off(app: &mut App) {
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        for _ in 0..3 {
            app.update();
        }
    }

    /// Before the recall he stays in the cab; after it he comes, once.
    #[test]
    fn the_epilogue_opens_only_after_the_recall() {
        let mut a = app();
        set_off(&mut a);
        assert!(lev(&mut a).is_none(), "not before the recall");
        assert!(!a.world().resource::<Epilogue>().active());

        let mut b = app();
        b.world_mut().resource_mut::<Raid>().recalled = true;
        set_off(&mut b);
        assert!(lev(&mut b).is_some(), "after it, he comes");
        assert!(b.world().resource::<Epilogue>().active());
        let raid = b.world().resource::<Raid>();
        assert!(raid.log.iter().any(|(l, _)| l == SETTING_OFF));

        let mut c = app();
        c.world_mut().resource_mut::<Raid>().recalled = true;
        c.world_mut().resource_mut::<Epilogue>().done = true;
        set_off(&mut c);
        assert!(lev(&mut c).is_none(), "and only once a career");
    }

    /// From the tag office: the release opens the crates, he walks to
    /// K-14 and finds her; you go home, and the raid ends at the truck.
    #[test]
    fn lev_reaches_k14_the_crates_open_and_the_raid_ends_at_the_truck() {
        let mut app = app();
        app.world_mut().resource_mut::<Raid>().recalled = true;
        set_off(&mut app);
        // The stacks lit, as the player would have them.
        for mut p in app
            .world_mut()
            .query::<&mut crate::world::underground::SitePower>()
            .iter_mut(app.world_mut())
        {
            for s in &mut p.stages {
                s.on = true;
            }
        }
        let terminal = app
            .world_mut()
            .query::<(&GlobalTransform, &SceneScreen)>()
            .iter(app.world())
            .find(|(_, s)| s.scene == "Tag office desk")
            .map(|(g, _)| g.translation())
            .expect("the intake terminal");
        // You at the terminal, he just behind you.
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .single(app.world())
            .unwrap();
        let stand = |app: &mut App, at: Vec3| {
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = at;
        };
        stand(&mut app, terminal - Vec3::Z * 1.0 - Vec3::Y * 0.8);
        let behind = terminal - Vec3::Z * 2.5 - Vec3::Y * 0.8;
        for mut t in app
            .world_mut()
            .query_filtered::<&mut Transform, With<LevWalker>>()
            .iter_mut(app.world_mut())
        {
            t.translation = behind;
        }
        for _ in 0..5 {
            stand(&mut app, terminal - Vec3::Z * 1.0 - Vec3::Y * 0.8);
            app.update();
        }
        let epi = app.world().resource::<Epilogue>().clone();
        assert!(epi.crates_open, "the release went through");
        assert_eq!(epi.stage, Stage::ToTheBay);
        let text = app
            .world_mut()
            .query::<(&SceneScreen, &crate::raid::bench::ScreenText)>()
            .iter(app.world())
            .find(|(s, _)| s.scene == "Tag office desk")
            .map(|(_, t)| t.text.clone())
            .unwrap();
        assert!(text.contains("OPENED"), "{text}");
        // He walks to her.
        let bay = bay_point().unwrap();
        let mut reached = false;
        for _ in 0..60 * 120 {
            stand(&mut app, terminal - Vec3::Z * 1.0 - Vec3::Y * 0.8);
            app.update();
            if app.world().resource::<Epilogue>().stage == Stage::Found {
                reached = true;
                break;
            }
        }
        let at = lev(&mut app).unwrap();
        assert!(reached, "Lev reaches K-14: he's at {at:?}, the bay {bay:?}");
        assert!((at.y - bay.y).abs() < 0.5, "on the bay's floor: {at:?}");
        for _ in 0..60 * 40 {
            app.update();
            if app.world().resource::<Epilogue>().said == FOUND.len() {
                break;
            }
        }
        assert_eq!(
            app.world().resource::<Epilogue>().said,
            FOUND.len(),
            "he says it all"
        );
        // Home: the truck, as any raid ends.
        app.world_mut().resource_mut::<Raid>().phase = Phase::Extracting { left: 0.3 };
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(app.world().resource::<Raid>().phase, Phase::AtBase);
        assert!(lev(&mut app).is_none(), "he's back in the cab");
        assert!(!app.world().resource::<Epilogue>().active());
        assert!(app.world().resource::<Epilogue>().done);
        assert_eq!(
            app.world()
                .resource::<super::super::logbook::Logbook>()
                .written
                .as_deref(),
            Some(LOGBOOK),
            "the logbook says so"
        );
    }
}
