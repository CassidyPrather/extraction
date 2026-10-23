//! The machines that had no sound, and the moments you must react to that
//! didn't either (`docs/AUDIO.md`, "Everything you must react to"): the
//! mender, the minelayer and its mines, the plough, the spotter, the
//! sorter, the hound's crouch and bite, a resident waking, a wreck coming
//! down, a hand thrown off a live cell, and you hurt or down. Each was a
//! message "for sound" that nothing played; `the_dangers_are_heard`
//! fails on the next one.
//!
//! Like the rest of Audio this only listens.

use bevy::prelude::*;

use crate::audio::{AudioRng, CRACKLE_SET, SoundBank, play_at_speed};
use crate::robots::devices::{DeviceNoticed, Reaction};
use crate::robots::hazards::{Effect, Hazard, HazardStruck};
use crate::robots::hound::HoundCrouch;
use crate::robots::hound_mind::HoundBite;
use crate::robots::indoor::Waking;
use crate::robots::mender::{MenderAct, MenderEvent};
use crate::robots::minelayer::{MineAct2, MineEvent};
use crate::robots::plough::{PloughAct, PloughEvent};
use crate::robots::recall::Filed;
use crate::robots::salvage::Arced;
use crate::robots::sorter::{SorterThrow, SorterWindup};
use crate::robots::spotter::{SpotterAct, SpotterEvent};
use crate::robots::squad::{Role, SquadCall};
use crate::robots::walker::Suppressing;
use crate::robots::wreck::WreckImpact;

pub struct MachineSoundsPlugin;

impl Plugin for MachineSoundsPlugin {
    fn build(&self, app: &mut App) {
        // (Their own plugins register these too; this keeps the readers
        // valid in apps without them.)
        app.add_message::<MenderEvent>()
            .add_message::<MineEvent>()
            .add_message::<PloughEvent>()
            .add_message::<SpotterEvent>()
            .add_message::<SorterWindup>()
            .add_message::<SorterThrow>()
            .add_message::<HoundCrouch>()
            .add_message::<HoundBite>()
            .add_message::<Waking>()
            .add_message::<WreckImpact>()
            .add_message::<Arced>()
            .add_message::<DeviceNoticed>()
            .add_message::<SquadCall>()
            .add_message::<HazardStruck>()
            .add_message::<crate::health::Hurt>()
            .add_message::<crate::health::Downed>()
            .add_systems(
                Update,
                (
                    mender_sounds,
                    mine_sounds,
                    plough_sounds,
                    spotter_sounds,
                    sorter_sounds,
                    hound_sounds,
                    body_sounds,
                    you_sounds,
                    notice_sounds,
                    squad_sounds,
                    filed_sounds,
                    suppress_sounds,
                    hazard_sounds,
                ),
            );
    }
}

/// A layer: (set, cue, playback speed).
type Layer = (&'static str, &'static str, f64);

fn layers(commands: &mut Commands, bank: &SoundBank, rng: &mut AudioRng, at: Vec3, ls: &[Layer]) {
    for &(set, cue, speed) in ls {
        play_at_speed(
            commands,
            bank,
            &mut rng.0,
            set,
            Some(at),
            cue,
            0.0,
            speed,
            0.06,
        );
    }
}

/// The mender: setting off to a patient, a servo's rising whine; welding,
/// the arc's crackle; a part whole again, a clank.
fn mender_sounds(
    mut commands: Commands,
    mut events: MessageReader<MenderEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for e in events.read() {
        let ls: &[Layer] = match e.what {
            MenderAct::Called => &[("machines/servo", "mender.called", 1.2)],
            MenderAct::Strike => &[(CRACKLE_SET, "mender.weld", 0.7)],
            MenderAct::Mended => &[("machines/clank", "mender.mended", 1.3)],
        };
        layers(&mut commands, &bank, &mut rng, e.at, ls);
    }
}

/// The minelayer and its mines. The click underfoot is the one that must
/// be heard (`mine.click`, a Signal): a hard switch's snap, before the
/// blast.
fn mine_sounds(
    mut commands: Commands,
    mut events: MessageReader<MineEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for e in events.read() {
        let ls: &[Layer] = match e.what {
            MineAct2::Laid => &[("machines/clank", "mine.laid", 0.8)],
            MineAct2::Click => &[("physical/switch", "mine.click", 0.7)],
            MineAct2::Blast => &[("explosions/grenade_sharp", "mine.blast", 1.0)],
            MineAct2::Disarmed => &[("physical/switch", "mine.safe", 1.4)],
            MineAct2::CookOff => &[
                ("explosions/grenade_sharp", "mine.blast", 0.8),
                (CRACKLE_SET, "mine.cookoff", 1.0),
            ],
            MineAct2::Fled => &[("machines/servo", "mine.flee", 1.4)],
        };
        layers(&mut commands, &bank, &mut rng, e.at, ls);
    }
}

/// The plough: its horn at something in its lane (the heavy's horn,
/// higher); the bank swallowing something; the auger jamming and clearing.
fn plough_sounds(
    mut commands: Commands,
    mut events: MessageReader<PloughEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for e in events.read() {
        let ls: &[Layer] = match e.what {
            PloughAct::Horn => &[("voice/heavy/horn", "plough.horn", 1.3)],
            PloughAct::Buried => &[("impacts/dirt", "plough.bury", 0.6)],
            PloughAct::Jammed => &[
                ("machines/groan", "plough.jam", 0.7),
                ("machines/clank", "mender.mended", 0.8),
            ],
            PloughAct::Cleared => &[("machines/clank", "mender.mended", 1.0)],
            PloughAct::Caught => &[("machines/groan", "plough.jam", 1.0)],
            PloughAct::Dug => &[("impacts/dirt", "plough.bury", 0.9)],
        };
        layers(&mut commands, &bank, &mut rng, e.at, ls);
    }
}

/// The spotter: building a fix on you, a lock tone (a Signal); its
/// klaxon calling the others; cut loose, holed, down.
fn spotter_sounds(
    mut commands: Commands,
    mut events: MessageReader<SpotterEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for e in events.read() {
        let ls: &[Layer] = match e.what {
            SpotterAct::Fixing => &[
                ("machines/drone_lock", "spotter.fix", 0.8),
                ("physical/switch", "lock.tick", 1.8),
            ],
            SpotterAct::Call => &[("machines/siren", "spotter.call", 1.0)],
            SpotterAct::Loose => &[("physical/whoosh", "spotter.loose", 1.0)],
            SpotterAct::Holed => &[
                ("physical/whoosh", "spotter.loose", 0.7),
                ("machines/groan", "plough.jam", 1.2),
            ],
            SpotterAct::Down => &[("impacts/metal", "spotter.down", 0.6)],
            // Drifted off out of the valley: gone without a sound.
            SpotterAct::Gone => &[],
        };
        layers(&mut commands, &bank, &mut rng, e.at, ls);
    }
}

/// The sorter: winding up to throw (the tell: its grab drawing back, a
/// servo straining up, with a lock tone), and the throw's whoosh.
fn sorter_sounds(
    mut commands: Commands,
    mut windups: MessageReader<SorterWindup>,
    mut throws: MessageReader<SorterThrow>,
    places: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for w in windups.read() {
        let at = places.get(w.sorter).map_or(w.at, |g| g.translation());
        layers(
            &mut commands,
            &bank,
            &mut rng,
            at,
            &[
                ("machines/servo", "sorter.windup", 0.8),
                ("machines/drone_lock", "spotter.fix", 1.2),
            ],
        );
    }
    for t in throws.read() {
        layers(
            &mut commands,
            &bank,
            &mut rng,
            t.from,
            &[("physical/whoosh", "sorter.throw", 1.0)],
        );
    }
}

/// The hound: coiling to spring (the tell, 0.6 s before the lunge: its
/// servos winding low), and its bite's hard snap on you.
fn hound_sounds(
    mut commands: Commands,
    mut crouches: MessageReader<HoundCrouch>,
    mut bites: MessageReader<HoundBite>,
    places: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for c in crouches.read() {
        let at = places.get(c.hound).map_or(c.at, |g| g.translation());
        layers(
            &mut commands,
            &bank,
            &mut rng,
            at,
            &[("machines/servo", "hound.crouch", 0.65)],
        );
    }
    for b in bites.read() {
        layers(
            &mut commands,
            &bank,
            &mut rng,
            b.at,
            &[
                ("impacts/metal", "hound.bite", 1.4),
                ("physical/body_hit", "hit.body", 1.0),
            ],
        );
    }
}

/// Machines' bodies: a resident waking (servos stirring, a clank as it
/// rises: a Signal, it's coming); a wreck coming down hard, as heavy as
/// it's big; a hand thrown off a live cell, the arc's crack.
fn body_sounds(
    mut commands: Commands,
    mut wakes: MessageReader<Waking>,
    mut wrecks: MessageReader<WreckImpact>,
    mut arcs: MessageReader<Arced>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for w in wakes.read() {
        layers(
            &mut commands,
            &bank,
            &mut rng,
            w.at,
            &[
                ("machines/servo", "robot.wake", 0.6),
                ("machines/clank", "mine.laid", 1.1),
            ],
        );
    }
    for w in wrecks.read() {
        let size = w.size.clamp(0.5, 4.0);
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "impacts/metal",
            Some(w.at),
            "wreck.thud",
            -6.0 + 3.0 * size.log2(),
            (0.75 / size.sqrt()) as f64,
            0.06,
        );
    }
    for a in arcs.read() {
        layers(
            &mut commands,
            &bank,
            &mut rng,
            a.at,
            &[
                (CRACKLE_SET, "cell.arc", 1.2),
                ("physical/body_hit", "hit.body", 1.1),
            ],
        );
    }
}

/// A machine noticing a device you set down (`robots::devices`): a
/// bright two-note chirp at it, a question (`device.notice`), or, if it
/// means to hunt the thing down or shoot it from afar, three quicker,
/// harder, higher (`device.hunt`). Its voice's turn follows on its own
/// (`search::MindShift`): this is "it's clocked my device", that is
/// "it's heard me".
fn notice_sounds(
    mut commands: Commands,
    mut noticed: MessageReader<DeviceNoticed>,
    places: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for n in noticed.read() {
        let at = places
            .get(n.robot)
            .map_or(n.at, |g| g.translation() + Vec3::Y * 0.5);
        let (set, cue) = match n.reaction {
            Reaction::Investigate => ("device/notice", "device.notice"),
            Reaction::Hunt | Reaction::ShootFromAfar => ("device/notice_hard", "device.hunt"),
        };
        layers(&mut commands, &bank, &mut rng, at, &[(set, cue, 1.0)]);
    }
}

/// A machine taking its part in a plan on you (`robots::squad::SquadCall`):
/// its pattern at it (`squad.*`), so a player within the chirp's reach
/// hears the plan, and where each part is, without a radio (the radio's
/// line is Robot minds', heard as chatter). A tick under it to place it
/// by (`lock.tick`).
fn squad_sounds(
    mut commands: Commands,
    mut calls: MessageReader<SquadCall>,
    places: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for c in calls.read() {
        let at = places
            .get(c.robot)
            .map_or(c.at, |g| g.translation() + Vec3::Y * 0.5);
        layers(
            &mut commands,
            &bank,
            &mut rng,
            at,
            &[squad_layer(c.role), ("physical/switch", "lock.tick", 1.8)],
        );
    }
}

/// The pattern a part's call plays.
pub fn squad_layer(role: Role) -> Layer {
    match role {
        Role::Pin => ("signal/squad_pin", "squad.pin", 1.0),
        Role::Flank => ("signal/squad_flank", "squad.flank", 1.0),
        Role::CutOff => ("signal/squad_cutoff", "squad.cutoff", 1.0),
        Role::Spot => ("signal/squad_spot", "squad.spot", 1.0),
        Role::Call => ("signal/squad_call", "squad.call", 1.0),
    }
}

/// A relay reporting you (`robots::recall::Filed`, added or its count
/// up): three falling notes at it, "noted", quicker and nearer full the
/// more often it has.
fn filed_sounds(
    mut commands: Commands,
    relays: Query<(&GlobalTransform, Ref<Filed>)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (gt, f) in &relays {
        if !f.is_changed() {
            continue;
        }
        let (db, speed) = filed_level(f.count);
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "signal/filed",
            Some(gt.translation() + Vec3::Y * 0.5),
            "relay.filed",
            db,
            speed,
            0.03,
        );
    }
}

/// A relay's `count`th report: (dB under its cue, playback speed).
pub fn filed_level(count: u32) -> (f32, f64) {
    let k = (count.saturating_sub(1)).min(4) as f32 / 4.0;
    (-6.0 * (1.0 - k), 1.0 + 0.15 * k as f64)
}

/// The walker pinning you going over to suppressing fire while another
/// moves up (`robots::walker::Suppressing` added): its feed racked heavy
/// and slow (`walker.suppress`, unlike its aim's clank and servo) and a
/// mode's snap (`walker.suppress_mode`): "it's keeping my head down, so
/// someone's coming". Racked down when it stops (`walker.suppress_off`).
fn suppress_sounds(
    mut commands: Commands,
    started: Query<&GlobalTransform, Added<Suppressing>>,
    mut stopped: RemovedComponents<Suppressing>,
    places: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for gt in &started {
        layers(
            &mut commands,
            &bank,
            &mut rng,
            gt.translation() + Vec3::Y * 1.5,
            &[
                ("guns/charging_handle", "walker.suppress", 0.7),
                ("physical/switch", "walker.suppress_mode", 0.9),
            ],
        );
    }
    for e in stopped.read() {
        if let Ok(gt) = places.get(e) {
            layers(
                &mut commands,
                &bank,
                &mut rng,
                gt.translation() + Vec3::Y * 1.5,
                &[("guns/charging_handle", "walker.suppress_off", 0.55)],
            );
        }
    }
}

/// A machine the Zone does something to (`robots::hazards::HazardStruck`),
/// at it, by what it does: a drone thrown by a lift, its rotors screaming
/// as it tumbles; a walker thrown, a servo's lurch and its foot scraping;
/// dragged in a well, its legs grinding; crushed at a well's heart, a
/// metal groan; dazzled, its sensor stuttering and its lock clicking out;
/// hurt by what makes its own sound (a burner, a snare, gas, a mine), a
/// short distress note over it; stunned, its servos winding down.
/// Deafened, nothing: the hush is heard already.
fn hazard_sounds(
    mut commands: Commands,
    mut struck: MessageReader<HazardStruck>,
    robots: Query<(&GlobalTransform, &crate::robots::Robot)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::robots::RobotKind;
    for h in struck.read() {
        let (at, flies) = robots.get(h.robot).map_or((h.at, false), |(g, r)| {
            (
                g.translation(),
                matches!(r.kind, RobotKind::Drone | RobotKind::Dirigible),
            )
        });
        layers(
            &mut commands,
            &bank,
            &mut rng,
            at + Vec3::Y * 0.5,
            hazard_layers(h.hazard, h.effect, flies),
        );
    }
}

/// What a machine struck by `hazard` to `effect` sounds like (`flies`: a
/// drone or the airship).
pub fn hazard_layers(hazard: Hazard, effect: Effect, flies: bool) -> &'static [Layer] {
    match (effect, hazard) {
        (Effect::Thrown, _) if flies => &[
            ("machines/drone_rotor", "hazard.scream", 1.6),
            ("physical/whoosh", "hazard.tumble", 0.8),
        ],
        (Effect::Thrown, _) => &[
            ("machines/servo", "hazard.lurch", 0.75),
            ("footsteps/gravel", "hazard.scrape", 0.6),
        ],
        (Effect::Slowed, _) => &[("machines/groan", "hazard.drag", 1.3)],
        (Effect::Hurt, Hazard::Well) => &[
            ("machines/groan", "hazard.crush", 0.6),
            ("impacts/metal", "hazard.crush_hit", 0.7),
        ],
        (Effect::Dazzled, _) => &[
            (CRACKLE_SET, "hazard.stutter", 1.8),
            ("machines/detector_click", "hazard.lock_lost", 0.7),
        ],
        (Effect::Stunned, _) => &[("machines/servo", "hazard.powerdown", 0.5)],
        (Effect::Hurt, _) => &[("machines/servo", "hazard.distress", 1.8)],
        (Effect::Deafened | Effect::Unaffected, _) => &[],
    }
}

/// The least between two hurts heard (s): a frost's or a fire's steady
/// harm would otherwise stutter.
const HURT_EVERY: f32 = 0.6;

/// You: hurt (a jolt through the body, and the breath knocked out of you,
/// for a hurt worth feeling; at most every [`HURT_EVERY`]); down (a heavy
/// fall). In your head.
fn you_sounds(
    mut commands: Commands,
    mut hurts: MessageReader<crate::health::Hurt>,
    mut downs: MessageReader<crate::health::Downed>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut quiet_until: Local<f32>,
) {
    let now = time.elapsed_secs();
    for h in hurts.read() {
        if !locals.contains(h.player) || h.amount < 4.0 || now < *quiet_until {
            continue;
        }
        *quiet_until = now + HURT_EVERY;
        let db = (-6.0 + 6.0 * (h.amount / 30.0).min(1.0)) - 0.0;
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/body_hit",
            None,
            "you.hurt",
            db,
            0.85,
            0.05,
        );
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "body/breath",
            None,
            "you.gasp",
            db,
            1.5,
            0.05,
        );
    }
    for d in downs.read() {
        if !locals.contains(d.player) {
            continue;
        }
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/body_hit",
            None,
            "you.down",
            0.0,
            0.6,
            0.03,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Messages that change what's dangerous, or that you must react to,
    /// that no sound needs: each with why. Everything else written as a
    /// `Message` must be read by Audio (or by the robots' voices).
    const UNHEARD: &[(&str, &str)] = &[
        (
            "ArmOrder",
            "the heavy's mind to its arm; the arm's moves are heard (ArmState)",
        ),
        (
            "Breach",
            "a wall breached is heard as it Broke (destruction)",
        ),
        ("CalledOut", "the robots' call is their voice's Calling"),
        ("CarriedOutOfDepot", "progression bookkeeping"),
        ("Cleared", "a space cleared for the nav grids"),
        (
            "ClimbedOver",
            "the climb's grips are heard; the comfort blink is visual",
        ),
        (
            "Combine",
            "flat mode's item combine; its gun events are heard",
        ),
        ("CraneRidden", "a counter for the bot; the crane is heard"),
        ("DeckEvent", "the tape deck's own sounds (radio)"),
        ("DepartRequest", "the truck's departure is heard"),
        ("DepotEntered", "a first-time hint"),
        ("DialClick", "the radio's own sounds (radio)"),
        ("Disturb", "a sign for robots to read"),
        ("DropEverything", "the items' landings are heard"),
        ("DropMagazine", "heard as the gun's MagazineRemoved"),
        (
            "Ejection",
            "heard as the gun's Ejected and the casing landing",
        ),
        ("EnteredWell", "a first-time hint; the well is heard"),
        ("FirstMet", "the bot's report"),
        ("Gestured", "a gesture's start (UI)"),
        ("GoalDone", "the onboarding's step is heard (StepDone)"),
        (
            "HandAction",
            "flat mode's hand commands; what they do is heard",
        ),
        (
            "HandOver",
            "Lev's item through the hatch lands in your hand (heard)",
        ),
        ("Haptic", "felt, not heard"),
        ("HapticShape", "felt, not heard"),
        ("HeavyTear", "the heavy's tear is heard by its arm (Tore)"),
        ("Hit", "heard as the Impact it's classed into"),
        ("Investigating", "a mind's turn: heard in its voice"),
        ("JobDone", "the card's chime (UI)"),
        ("LostYou", "a mind's turn: heard in its voice"),
        ("MeetFirst", "a first-time line"),
        ("NightVisit", "the walkers sent are heard walking"),
        (
            "Noise",
            "what robots hear; the sound itself is played by whoever made it",
        ),
        ("Noticed", "a mind's turn: heard in its voice"),
        ("PianoNote", "the piano's own sounds (radio)"),
        ("Relocate", "a load"),
        ("SaveNow", "a save"),
        ("Searching", "a mind's turn: heard in its voice"),
        ("Setup", "a bot scenario"),
        ("SiteGrid", "a nav grid"),
        ("Slept", "a fade"),
        ("Smash", "what it breaks is heard (Broke)"),
        ("ToggleBackpack", "flat mode's pack; opening it is heard"),
    ];

    /// Every gameplay event has a sound: each `Message` type the game
    /// writes is read by Audio, by the robots' voices, or listed above
    /// with why it needs none.
    #[test]
    fn the_dangers_are_heard() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sources = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    sources.push((p.clone(), std::fs::read_to_string(&p).unwrap()));
                }
            }
        }
        let mut messages = Vec::new();
        for (_, text) in &sources {
            let mut lines = text.lines();
            while let Some(l) = lines.next() {
                if !l.contains("derive(Message") {
                    continue;
                }
                for l in lines.by_ref() {
                    let l = l.trim();
                    if l.starts_with("///") || l.starts_with("#[") {
                        continue;
                    }
                    if let Some(rest) = l
                        .strip_prefix("pub struct ")
                        .or_else(|| l.strip_prefix("pub(crate) struct "))
                        .or_else(|| l.strip_prefix("pub enum "))
                        .or_else(|| l.strip_prefix("pub(crate) enum "))
                    {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        messages.push(name);
                    }
                    break;
                }
            }
        }
        messages.sort();
        messages.dedup();
        assert!(messages.len() > 100, "{} messages found", messages.len());
        let heard_in: Vec<&String> = sources
            .iter()
            .filter(|(p, _)| {
                let n = p.file_name().unwrap().to_string_lossy();
                n.starts_with("audio") || n == "robot_voice.rs" || n == "foley.rs"
            })
            .map(|(_, t)| t)
            .collect();
        let mut silent = Vec::new();
        for m in &messages {
            let read = heard_in.iter().any(|t| {
                t.split("MessageReader<").skip(1).any(|r| {
                    r.split('>')
                        .next()
                        .is_some_and(|ty| ty.rsplit("::").next() == Some(m))
                })
            });
            let excused = UNHEARD.iter().any(|(n, _)| n == m);
            assert!(!(read && excused), "`{m}` is heard: take it off UNHEARD");
            if !read && !excused {
                silent.push(m.clone());
            }
        }
        assert!(silent.is_empty(), "no sound for: {silent:?}");
    }

    /// Every cue this file plays is in the mix.
    #[test]
    fn the_machine_cues_are_in_the_mix() {
        let code = include_str!("audio_machines.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let mut n = 0;
        for lit in code.split('"').skip(1).step_by(2) {
            let prefixes = [
                "mender.", "mine.", "plough.", "spotter.", "sorter.", "hound.", "robot.", "wreck.",
                "cell.", "you.", "device.", "squad.", "relay.", "walker.", "hazard.",
            ];
            if prefixes.iter().any(|p| lit.starts_with(p)) && !lit.contains(' ') {
                assert!(
                    crate::audio_mix::MIX.iter().any(|c| c.name == lit),
                    "`{lit}` not in the mix"
                );
                n += 1;
            }
        }
        assert!(n > 20, "{n}");
    }

    /// Each part of a plan has its own pattern, in the mix with its set;
    /// a relay's "noted" comes up with its count, to its cue's level.
    #[test]
    fn every_part_sounds_its_own() {
        let roles = [Role::Pin, Role::Flank, Role::CutOff, Role::Spot, Role::Call];
        let mut seen = std::collections::HashSet::new();
        for r in roles {
            let (set, cue, _) = squad_layer(r);
            assert!(seen.insert(set), "{r:?} shares its pattern");
            let c = crate::audio_mix::MIX.iter().find(|c| c.name == cue);
            assert_eq!(c.map(|c| c.set), Some(set), "{cue}");
            assert!(
                crate::synth_foley::STAND_INS.iter().any(|s| s.set == set),
                "{set} is not rendered"
            );
        }
        assert_eq!(filed_level(1).0, -6.0);
        assert_eq!(filed_level(5).0, 0.0);
        assert!(filed_level(3).1 > filed_level(1).1);
        assert_eq!(filed_level(50), filed_level(5));
    }

    /// Every way the Zone strikes a machine that's heard is in the mix
    /// with its set, and a thrown drone isn't heard as a thrown walker.
    #[test]
    fn the_zone_striking_a_machine_is_heard() {
        let hazards = [
            Hazard::Lift,
            Hazard::Burner,
            Hazard::Snare,
            Hazard::Well,
            Hazard::Mirror,
            Hazard::Hush,
            Hazard::Gas,
            Hazard::Mine,
            Hazard::Emp,
        ];
        let effects = [
            Effect::Hurt,
            Effect::Slowed,
            Effect::Thrown,
            Effect::Dazzled,
            Effect::Deafened,
            Effect::Stunned,
        ];
        for h in hazards {
            for e in effects {
                for flies in [false, true] {
                    for (set, cue, _) in hazard_layers(h, e, flies) {
                        let c = crate::audio_mix::MIX.iter().find(|c| c.name == *cue);
                        assert_eq!(c.map(|c| c.set), Some(*set), "{cue}");
                    }
                    let heard = !hazard_layers(h, e, flies).is_empty();
                    assert_eq!(heard, e != Effect::Deafened, "{h:?} {e:?}");
                }
            }
        }
        assert_ne!(
            hazard_layers(Hazard::Lift, Effect::Thrown, true),
            hazard_layers(Hazard::Lift, Effect::Thrown, false)
        );
    }
}
