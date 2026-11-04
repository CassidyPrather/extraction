//! The PM's whole cycle by hand, scripted: drawn from the holster, racked,
//! raised to the eye, fired to slide lock, the magazine dropped and a fresh
//! one seated, the slide released, a shot, a stovepipe and its clear, a
//! last shot. Hands are driven as a headset drives them ([`RigInput`]), so
//! every beat is the real game's.
//!
//! Headless, [`Cycle::report`] gives each beat's numbers
//! (`weapons::cycle::tests`); rendered, `--gallery cycle:pm[,EVERY]` films
//! it, a frame every EVERY ticks at 90 Hz (default 3: 30 fps), to
//! `<screenshot stem>_NNN.png`:
//!
//! ```text
//! scripts/shot.sh target/reel/pm.png 0,0,30,0,-15 99999 --map training_yard \
//!     --no-robots --weather clearing --hour 11 --gallery cycle:pm
//! ```
//!
//! The camera given is the eye (the player stands under it); pitched down
//! 15 degrees, the hands at the chest and the sights both show.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use super::{CasingLanded, Ejection, FireEvent, Gun, GunEvent, Mag, feel::Handling};
use crate::interact::{Grabbable, GripRole, Hand, Socket, SocketKind, SocketOwner};
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::rig::{LocalPlayer, RigHand, RigInput, Side};
use extraction_core::firearm::{FireMode, Malfunction};
use extraction_core::inventory::SlotKind;

/// The rate the cycle is scripted at (the headset's, and the headless
/// game's).
pub const HZ: f32 = 90.0;

/// Where a hand goes, fixed when its move starts.
#[derive(Clone, Copy, Debug)]
enum To {
    /// In the playspace (metres; -Z ahead, Y up from the floor).
    Local(Vec3),
    /// The gun's grab point of this role, plus a world offset.
    Grab(GripRole, Vec3),
    /// The slide's grab, drawn back by this share of its travel.
    Slide(f32),
    /// The seated magazine's middle, plus this along the gun's up.
    Mag(f32),
    /// The spare magazine's middle.
    Spare,
    /// The well's mouth, less this along the gun's up (below it).
    Well(f32),
}

#[derive(Clone, Copy, Debug)]
enum Act {
    /// A beat begins (named in the report).
    Beat(&'static str),
    Wait(u32),
    /// A hand moves there over so many ticks, turned so (playspace).
    Move(Side, To, Quat, u32),
    Grip(Side, f32),
    Trigger(Side, f32),
    /// The gun hand's A (the release) held or let go.
    Release(bool),
    /// The gun hand's B (the slide stop) pressed and let go.
    SlideStop,
    /// The next shot's case hangs in the port.
    Stovepipe,
}

use Act::*;

/// The gun hand's turn: level, the PM's bore straight ahead.
const LEVEL: Quat = Quat::IDENTITY;

fn script() -> Vec<Act> {
    let eye = Vec3::new(0.0, 1.65, 0.0);
    // At the eye: the rear notch 0.0805 over the grip, the gun out 45 cm.
    let aim = eye + Vec3::new(0.0, -0.0805, -0.45);
    let ready = Vec3::new(0.12, 1.25, -0.32);
    let cup = To::Grab(GripRole::Support, Vec3::ZERO);
    let shot = [
        Trigger(Side::Right, 1.0),
        Wait(8),
        Trigger(Side::Right, 0.0),
        Wait(28),
    ];
    let mut s = vec![
        Wait(30),
        Beat("draw"),
        Move(
            Side::Right,
            To::Grab(GripRole::Primary, Vec3::ZERO),
            LEVEL,
            30,
        ),
        Grip(Side::Right, 1.0),
        Wait(4),
        Move(Side::Right, To::Local(ready), LEVEL, 32),
        Beat("rack"),
        Move(Side::Left, To::Slide(0.0), Quat::IDENTITY, 24),
        Trigger(Side::Left, 1.0),
        Wait(3),
        Move(Side::Left, To::Slide(1.15), Quat::IDENTITY, 11),
        Trigger(Side::Left, 0.0),
        Wait(2),
        Beat("aim"),
        Move(Side::Right, To::Local(aim), LEVEL, 36),
        Move(Side::Left, cup, Quat::IDENTITY, 14),
        Grip(Side::Left, 1.0),
        Wait(20),
        Beat("fire"),
    ];
    // Three rounds in the magazine: the third locks the slide back.
    for _ in 0..3 {
        s.extend(shot);
    }
    s.extend([
        Beat("slide lock"),
        Wait(30),
        Beat("mag out"),
        Grip(Side::Left, 0.0),
        Move(Side::Left, To::Mag(0.0), Quat::IDENTITY, 14),
        Release(true),
        Wait(6),
        Move(Side::Left, To::Mag(-0.02), Quat::IDENTITY, 4),
        Release(false),
        Wait(30),
        Beat("mag in"),
        Move(Side::Left, To::Spare, Quat::IDENTITY, 36),
        Grip(Side::Left, 1.0),
        Wait(4),
        Move(Side::Left, To::Well(0.12), Quat::IDENTITY, 30),
        Move(Side::Left, To::Well(0.0), Quat::IDENTITY, 6),
        Grip(Side::Left, 0.0),
        Wait(8),
        Beat("slide release"),
        SlideStop,
        Wait(20),
        Move(Side::Left, cup, Quat::IDENTITY, 14),
        Grip(Side::Left, 1.0),
        Wait(12),
        Beat("fire again"),
        Stovepipe,
    ]);
    s.extend(shot);
    s.extend([
        Beat("stovepipe"),
        Trigger(Side::Right, 1.0),
        Wait(8),
        Trigger(Side::Right, 0.0),
        Wait(20),
        Beat("clear"),
        Grip(Side::Left, 0.0),
        Move(Side::Left, To::Slide(0.0), Quat::IDENTITY, 18),
        Trigger(Side::Left, 1.0),
        Wait(3),
        Move(Side::Left, To::Slide(1.15), Quat::IDENTITY, 11),
        Trigger(Side::Left, 0.0),
        Wait(4),
        Move(Side::Left, cup, Quat::IDENTITY, 14),
        Grip(Side::Left, 1.0),
        Wait(10),
        Beat("last shot"),
    ]);
    s.extend(shot);
    s.extend([Beat("end"), Wait(45)]);
    s
}

/// One frame's worth of what happened, for the report.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    pub t: f32,
    pub beat: &'static str,
    /// The gun's bore against where the gun hand points (degrees, up +).
    pub rise: f32,
    /// How far the gun sits back of the hand along the bore (m).
    pub push: f32,
    pub slide: f32,
    pub locked: bool,
    pub flash: bool,
    pub events: Vec<FireEvent>,
    /// Cases thrown this tick (where, how fast).
    pub thrown: Vec<(Vec3, Vec3)>,
    /// Cases landing this tick (where, first touch).
    pub landed: Vec<(Vec3, bool)>,
}

/// The scripted cycle and what it recorded.
#[derive(Resource, Default)]
pub struct Cycle {
    acts: Vec<Act>,
    at: usize,
    since: u32,
    frame: u32,
    beat: &'static str,
    /// A move's start and end (world), fixed as it starts.
    path: Option<(Vec3, Vec3)>,
    pub gun: Option<Entity>,
    pub spare: Option<Entity>,
    stovepipe: bool,
    pub ticks: Vec<Tick>,
    pub done: bool,
}

impl Cycle {
    pub fn new() -> Self {
        Self {
            acts: script(),
            ..Default::default()
        }
    }
}

pub struct CyclePlugin;

impl Plugin for CyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                drive.run_if(resource_exists::<Cycle>),
                record.run_if(resource_exists::<Cycle>),
                film,
            )
                .chain(),
        );
    }
}

fn player(world: &mut World) -> Option<Entity> {
    let mut q = world.query_filtered::<Entity, (With<LocalPlayer>, With<RigInput>)>();
    q.iter(world).next()
}

fn hand_of(world: &mut World, side: Side) -> Option<Hand> {
    let mut q = world.query::<(&RigHand, &Hand)>();
    q.iter(world)
        .find(|(r, _)| r.side == side)
        .map(|(_, h)| h.clone())
}

/// The PM in the holster, a magazine of three in it and the chamber empty;
/// a spare of eight in a pouch.
fn set_up(world: &mut World, c: &mut Cycle, who: Entity) {
    let catalog = world.resource::<Catalog>().clone();
    let root = *world.get::<Transform>(who).unwrap();
    let near = root.transform_point(Vec3::new(0.3, 1.0, -0.3));
    let spawn = |world: &mut World, id: &str| {
        let kind = catalog.kind(id).expect("an item");
        let e = spawn_item(
            &mut world.commands(),
            &catalog,
            SpawnItem {
                kind,
                transform: Transform::from_translation(near),
                condition: 1.0,
            },
        );
        world.flush();
        e
    };
    let gun = spawn(world, "pm_pistol");
    let mag = spawn(world, "pm_mag");
    let spare = spawn(world, "pm_mag");
    world.get_mut::<Item>(mag).unwrap().rounds = 3;
    world.get_mut::<Item>(spare).unwrap().rounds = 8;
    c.gun = Some(gun);
    c.spare = Some(spare);
    c.since = 0;
    // Its well and the player's holster and a pouch, once spawned.
    let _ = mag;
}

/// Seats `item` in `socket` as a hand putting it there would.
fn seat(world: &mut World, item: Entity, socket: Entity) {
    let mut s = world.get::<Socket>(socket).unwrap().clone();
    {
        let mut commands = world.commands();
        crate::interact::seat_in_socket(&mut commands, item, socket, &mut s);
    }
    world.flush();
    *world.get_mut::<Socket>(socket).unwrap() = s;
    world.write_message(crate::interact::InteractEvent::Socketed { item, socket });
}

fn slot(world: &mut World, who: Entity, kind: SlotKind) -> Option<Entity> {
    let mut q = world.query::<(Entity, &Socket, &SocketOwner)>();
    q.iter(world)
        .find(|(_, s, o)| o.0 == who && s.kind == SocketKind::Body(kind) && s.occupant.is_none())
        .map(|(e, ..)| e)
}

/// Where `to` is in the world now.
fn place(world: &mut World, c: &Cycle, to: To, root: &Transform) -> Option<Vec3> {
    let gun = c.gun?;
    let gt = *world.get::<GlobalTransform>(gun)?;
    let g = world.get::<Gun>(gun)?;
    let up = gt.up().as_vec3();
    Some(match to {
        To::Local(p) => root.transform_point(p),
        To::Grab(role, off) => {
            let grab = world.get::<Grabbable>(gun)?;
            let p = grab.points.iter().find(|p| p.role == role)?;
            gt.mul_transform(p.local).translation() + off
        }
        To::Slide(k) => {
            let h = g.layout.handle.as_ref()?;
            gt.transform_point(h.grab) + gt.back().as_vec3() * h.travel * k
        }
        To::Well(below) => {
            let well = g.magwell?;
            world.get::<GlobalTransform>(well)?.translation() - up * below
        }
        To::Mag(off) => {
            let well = g.magwell?;
            let s = world.get::<Socket>(well)?;
            let m = s.occupant?;
            world.get::<GlobalTransform>(m)?.translation() + up * off
        }
        To::Spare => world.get::<GlobalTransform>(c.spare?)?.translation(),
    })
}

fn input(world: &mut World, who: Entity, f: impl FnOnce(&mut crate::rig::HandInput)) {
    let mut i = world.get_mut::<RigInput>(who).unwrap();
    f(i.hand_mut(Side::Right));
}

fn set_hand(world: &mut World, who: Entity, side: Side, at: Vec3, rot: Quat) {
    let root = *world.get::<Transform>(who).unwrap();
    let local = root.compute_affine().inverse().transform_point3(at);
    let mut i = world.get_mut::<RigInput>(who).unwrap();
    let h = i.hand_mut(side);
    h.pose = Transform::from_translation(local).with_rotation(rot);
    h.tracked = true;
}

fn drive(world: &mut World) {
    world.insert_resource(crate::rig::RigDriver::Xr);
    let Some(who) = player(world) else {
        return;
    };
    let mut c = world.remove_resource::<Cycle>().unwrap();
    c.frame += 1;
    run(world, &mut c, who);
    world.insert_resource(c);
}

fn run(world: &mut World, c: &mut Cycle, who: Entity) {
    let root = *world.get::<Transform>(who).unwrap();
    if c.frame == 1 {
        let mut i = world.get_mut::<RigInput>(who).unwrap();
        i.head = Transform::from_xyz(0.0, 1.65, 0.0);
        for (side, x) in [(Side::Left, -0.18), (Side::Right, 0.18)] {
            let h = i.hand_mut(side);
            h.pose = Transform::from_xyz(x, 0.95, -0.15);
            h.tracked = true;
        }
        set_up(world, c, who);
        return;
    }
    if c.frame == 4 {
        // Into the holster and the pouch, the magazine into the gun.
        let (gun, spare) = (c.gun.unwrap(), c.spare.unwrap());
        let well = world.get::<Gun>(gun).and_then(|g| g.magwell);
        let mag = {
            let mut q = world.query_filtered::<(Entity, &Item), With<Mag>>();
            q.iter(world)
                .find(|(e, i)| *e != spare && i.rounds == 3)
                .map(|(e, _)| e)
        };
        if let (Some(well), Some(mag)) = (well, mag) {
            seat(world, mag, well);
        }
        if let Some(h) = slot(world, who, SlotKind::Holster) {
            seat(world, gun, h);
        }
        if let Some(p) = slot(world, who, SlotKind::Pouch) {
            seat(world, spare, p);
        }
        let mut g = world.get_mut::<Gun>(gun).unwrap();
        g.firearm.chambers.iter_mut().for_each(|r| *r = None);
        g.firearm.condition = 1.0;
        while g.firearm.fire_mode() != FireMode::Semi {
            g.firearm.cycle_mode();
        }
        return;
    }
    if c.frame < 4 {
        return;
    }
    let Some(&act) = c.acts.get(c.at) else {
        c.done = true;
        return;
    };
    let mut next = true;
    match act {
        Beat(b) => c.beat = b,
        Wait(n) => next = c.since + 1 >= n,
        Move(side, to, rot, n) => {
            if c.since == 0 {
                let from = {
                    let i = world.get::<RigInput>(who).unwrap();
                    root.transform_point(i.hand(side).pose.translation)
                };
                c.path = place(world, c, to, &root).map(|b| (from, b));
            }
            if let Some((a, b)) = c.path {
                let k = ((c.since + 1) as f32 / n as f32).min(1.0);
                // Eased in and out, as a hand moves.
                let e = k * k * (3.0 - 2.0 * k);
                set_hand(world, who, side, a.lerp(b, e), rot);
            }
            next = c.since + 1 >= n;
        }
        Grip(side, v) => world.get_mut::<RigInput>(who).unwrap().hand_mut(side).grip = v,
        Trigger(side, v) => {
            world
                .get_mut::<RigInput>(who)
                .unwrap()
                .hand_mut(side)
                .trigger = v
        }
        Release(down) => input(world, who, |h| h.primary = down),
        SlideStop => {
            input(world, who, |h| h.secondary = c.since < 3);
            next = c.since + 1 >= 6;
        }
        Stovepipe => c.stovepipe = true,
    }
    if c.stovepipe
        && let Some(mut g) = c.gun.and_then(|g| world.get_mut::<Gun>(g))
        && g.firearm.malfunction.is_none()
        && c.beat == "stovepipe"
    {
        g.firearm.malfunction = Some(Malfunction::FailureToEject);
        g.firearm.bolt_travel = 0.2;
        c.stovepipe = false;
    }
    if next {
        c.at += 1;
        c.since = 0;
    } else {
        c.since += 1;
    }
    let _ = hand_of;
}

/// What the frame showed and did.
#[allow(clippy::too_many_arguments)]
fn record(
    mut cycle: ResMut<Cycle>,
    time: Res<Time>,
    guns: Query<(&Gun, &GlobalTransform, Option<&Handling>)>,
    hands: Query<(&RigHand, &GlobalTransform)>,
    mut events: MessageReader<GunEvent>,
    mut thrown: MessageReader<Ejection>,
    mut landed: MessageReader<CasingLanded>,
    bits: Query<&super::impacts::Bit>,
) {
    let Some(gun) = cycle.gun else {
        return;
    };
    let mut tick = Tick {
        t: time.elapsed_secs(),
        beat: cycle.beat,
        ..Default::default()
    };
    if let Ok((g, gt, _h)) = guns.get(gun) {
        tick.slide = g.firearm.bolt_travel;
        tick.locked = g.firearm.locked_back;
        if let Some((_, hand)) = hands.iter().find(|(r, _)| r.side == Side::Right) {
            let bore = gt.forward().as_vec3();
            let aim = hand.forward().as_vec3();
            let up = hand.up().as_vec3();
            tick.rise = bore.dot(up).atan2(bore.dot(aim)).to_degrees();
            tick.push = (gt.translation() - hand.translation()).dot(-aim);
        }
    }
    tick.flash = bits.iter().any(|b| b.life < 0.05 && b.age < b.life);
    tick.events = events
        .read()
        .filter(|e| e.gun == gun)
        .map(|e| e.event.clone())
        .collect();
    tick.thrown = thrown.read().map(|e| (e.at, e.velocity)).collect();
    tick.landed = landed.read().map(|l| (l.at, l.first)).collect();
    if cycle.frame > 4 && !cycle.done {
        cycle.ticks.push(tick);
    }
}

/// `--gallery cycle:pm[,EVERY]`: the cycle filmed from the eye.
fn film(
    mut commands: Commands,
    opts: Res<crate::launch::LaunchOptions>,
    cycle: Option<Res<Cycle>>,
    loaded: Option<Res<crate::render::timeline::Loaded>>,
    mut frame: Local<u32>,
    mut shot: Local<u32>,
) {
    let (Some(spec), Some(out)) = (&opts.gallery, &opts.screenshot) else {
        return;
    };
    let Some(rest) = spec.strip_prefix("cycle:") else {
        return;
    };
    let mut args = rest.split(',');
    let _what = args.next();
    let every: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(3).max(1);
    *frame += 1;
    if *frame == 1 {
        commands.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f32(1.0 / HZ),
        ));
        commands.insert_resource(Time::<Fixed>::from_hz(HZ as f64));
        return;
    }
    // Once everything's in, and the free camera's placed (`render`): then
    // the rig is ours.
    if cycle.is_none() {
        if loaded.is_some() && *frame > 8 {
            commands.insert_resource(Cycle::new());
        }
        return;
    }
    let Some(cycle) = cycle else {
        return;
    };
    if cycle.frame < 4 || (cycle.frame % every != 0 && !cycle.done) {
        return;
    }
    let stem = out
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "cycle".into());
    let dir = out.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{stem}_{:03}.png", *shot));
    *shot += 1;
    let last = cycle.done;
    let mut save = save_to_disk(path);
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            save(captured);
            if last {
                exit.write(AppExit::Success);
            }
        },
    );
}

impl Cycle {
    /// Each beat's numbers, a line each, then each shot's.
    pub fn report(&self) -> String {
        let mut out = String::new();
        let mut beats: Vec<&'static str> = Vec::new();
        for t in self.ticks.iter().filter(|t| !t.beat.is_empty()) {
            if beats.last() != Some(&t.beat) {
                beats.push(t.beat);
            }
        }
        for b in beats {
            let ts: Vec<&Tick> = self.ticks.iter().filter(|t| t.beat == b).collect();
            let (t0, t1) = (ts[0].t, ts[ts.len() - 1].t);
            let evs: Vec<String> = ts
                .iter()
                .flat_map(|t| {
                    t.events
                        .iter()
                        .map(move |e| format!("{:.2} {e:?}", t.t - t0))
                })
                .collect();
            out += &format!("{b}: {:.2} s; {}\n", t1 - t0, evs.join(", "));
        }
        for s in self.shots() {
            out += &format!("{s:?}\n");
        }
        out
    }

    /// What each shot did, from the tick it fired.
    pub fn shots(&self) -> Vec<Shot> {
        let fired: Vec<usize> = (0..self.ticks.len())
            .filter(|&i| {
                self.ticks[i]
                    .events
                    .iter()
                    .any(|e| matches!(e, FireEvent::Fired { .. }))
            })
            .collect();
        fired
            .iter()
            .map(|&i| {
                let t0 = self.ticks[i].t;
                let base = self.ticks[i.saturating_sub(1)].rise;
                let after: Vec<&Tick> = self.ticks[i..]
                    .iter()
                    .take_while(|t| t.t - t0 < 0.4)
                    .collect();
                let (k, peak) = after
                    .iter()
                    .enumerate()
                    .map(|(k, t)| (k, t.rise - base))
                    .fold((0, 0.0f32), |a, b| if b.1 > a.1 { b } else { a });
                let back = after[k..]
                    .iter()
                    .find(|t| t.rise - base < peak * 0.15)
                    .map(|t| t.t - t0);
                let push0 = self.ticks[i.saturating_sub(1)].push;
                let push = after.iter().map(|t| t.push - push0).fold(0.0f32, f32::max);
                let flash = after.iter().filter(|t| t.flash).count();
                let slide = after.iter().filter(|t| t.slide > 0.05).count();
                let thrown = after.iter().flat_map(|t| &t.thrown).next().copied();
                let landed = self.ticks[i..]
                    .iter()
                    .take_while(|t| t.t - t0 < 3.0)
                    .find_map(|t| t.landed.iter().find(|l| l.1).map(|l| (t.t - t0, l.0)));
                Shot {
                    t: t0,
                    rise_deg: peak,
                    peak_s: after[k].t - t0,
                    back_s: back,
                    push_m: push,
                    flash_ticks: flash,
                    slide_ticks: slide,
                    case_speed: thrown.map(|(_, v)| v.length()),
                    case_flight: landed.map(|l| l.0),
                    case_out_m: landed
                        .zip(thrown)
                        .map(|(l, (at, _))| Vec2::new(l.1.x - at.x, l.1.z - at.z).length()),
                }
            })
            .collect()
    }
}

/// One shot, measured.
#[derive(Clone, Debug)]
pub struct Shot {
    pub t: f32,
    /// The muzzle's rise off the hand at its peak (degrees), and when.
    pub rise_deg: f32,
    pub peak_s: f32,
    /// Back within 15% of the peak.
    pub back_s: Option<f32>,
    pub push_m: f32,
    pub flash_ticks: usize,
    /// Ticks the slide was seen off its rest.
    pub slide_ticks: usize,
    pub case_speed: Option<f32>,
    pub case_flight: Option<f32>,
    pub case_out_m: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vr_harness::Vr;

    /// The whole cycle runs through headless and every beat does its
    /// thing: three shots to slide lock, the magazine out and in, the slide
    /// released onto a round, a stovepipe cleared, and a last shot.
    #[test]
    fn the_pm_cycle_runs_by_hand() {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(0.0, 30.0);
        vr.stand(feet, 0.0);
        vr.app.insert_resource(Cycle::new());
        for _ in 0..3000 {
            vr.step(1);
            if vr.app.world().resource::<Cycle>().done {
                break;
            }
        }
        let c = vr.app.world().resource::<Cycle>();
        println!("{}", c.report());
        assert!(c.done, "the script ran out");
        let fired = c
            .ticks
            .iter()
            .flat_map(|t| &t.events)
            .filter(|e| matches!(e, FireEvent::Fired { .. }))
            .count();
        assert_eq!(fired, 5, "{}", c.report());
        // Three shots in the string, the third locking it back: two slams
        // home, not three.
        let homes = c
            .ticks
            .iter()
            .filter(|t| t.beat == "fire" || t.beat == "slide lock")
            .flat_map(|t| &t.events)
            .filter(|e| matches!(e, FireEvent::CarrierHome))
            .count();
        assert_eq!(homes, 2, "{}", c.report());
        // Each case lands somewhere of its own.
        let out: Vec<f32> = c.shots().iter().filter_map(|s| s.case_out_m).collect();
        let span =
            out.iter().fold(0.0f32, |m, x| m.max(*x)) - out.iter().fold(9.0f32, |m, x| m.min(*x));
        assert!(out.len() >= 4 && span > 0.1, "{out:?}");
    }
}
