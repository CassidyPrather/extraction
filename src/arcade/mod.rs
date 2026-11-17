//! The arcade: waves of the Zone's machines on a small fenced ground, with
//! the real guns, items, hands and minds, and none of the strategy layer
//! (no career, truck runs, contracts, economy or Surge). A sandbox for
//! item mechanics, tactics and UI, and a place to collect playtest data.
//! See `docs/ARCADE.md`.
//!
//! `extraction --arcade [--data-dir DIR]`, or [`enter`] from a menu.
//!
//! A run: you stand at the bench (`arena`), take what you want off its
//! tables and press CALL WAVE. Nothing comes until you do. The wave's
//! machines, and the gate they will come through, are on the board, that
//! gate's lamp lights and a siren sounds from it, and [`WARNING`] seconds
//! later they come through it (`waves`). Clear the wave and call the
//! next. Go down and the run is over: the board shows it, RESTART lays
//! everything out again where you stand. Each run is written down
//! (`record`, its numbers in `stats`), and the gun board beside the bench
//! holds each gun used to its best and asks how it felt. East of the bench
//! is a range (`range`): plates at 10, 25 and 50 m.
//!
//! Nothing here runs outside an arcade session: the plugin adds no system
//! unless [`active`].

use bevy::prelude::*;

use crate::launch::{LaunchOptions, SceneChoice};

pub mod arena;
pub mod range;
pub mod record;
pub mod scenario;
pub mod stats;
pub mod visuals;
pub mod waves;

/// The arcade's ground (`assets/maps/arcade.ron`).
pub const MAP: &str = "arcade";

/// Seconds from calling a wave to its machines coming through the gate:
/// the board, the lamp and the siren say what and where first.
pub const WARNING: f32 = 10.0;

/// The one entry point: makes `opts` an arcade session. A menu calls this
/// (and relaunches) as `--arcade` does.
pub fn enter(o: &mut LaunchOptions) {
    o.scene = SceneChoice::Arcade;
    o.map = MAP.into();
    o.map_explicit = true;
    // The director sends nothing: the ladder is the only source of
    // machines, and gunfire brings no one else.
    o.robots = false;
    // Runs compare: daylight, dry, unless asked otherwise.
    o.hour.get_or_insert_with(|| "day".into());
    o.weather.get_or_insert_with(|| "clearing".into());
}

/// Whether this session is an arcade one.
pub fn active(opts: &LaunchOptions) -> bool {
    opts.scene == SceneChoice::Arcade
}

/// Where the run is.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum RunPhase {
    /// At the bench, nothing called yet (or after RESTART).
    #[default]
    Ready,
    /// Called: the machines come through their gate when `left` runs out.
    Warning { wave: u32, left: f32 },
    /// The wave's machines are in.
    Fighting { wave: u32 },
    /// Wave `wave` cleared; the next waits for a call.
    Cleared { wave: u32 },
    /// Down: the run's over and its summary is on the board.
    Over { wave: u32 },
}

/// The arcade's state.
#[derive(Resource, Debug, Default)]
pub struct Arcade {
    pub phase: RunPhase,
    /// The wave a fresh run starts at (the board's START AT card).
    pub start_at: u32,
    pub score: u32,
    /// Seconds since this run's first call.
    pub run_t: f32,
    /// Runs this session (the record's file names count them).
    pub runs: u32,
    /// The bench's frame and where you stand at it, once set up.
    pub bench: Option<Transform>,
    pub stand: Option<Transform>,
    /// Frames left to keep standing the player at the bench (the truck's
    /// own placement runs in the first frames too).
    pub hold: u32,
    /// A reset under way: clear the ground this frame, lay out the next.
    pub reset: Reset,
    /// The last run's ending, short (`stats::RunStats::board`), for the
    /// board once it's over.
    pub summary: Option<String>,
    /// The best wave cleared and the best score this session.
    pub best: (u32, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Reset {
    #[default]
    None,
    /// Everything carried is dropped (`raid::truck::DropEverything`)...
    Dropped,
    /// ...then swept up with the rest and the tables laid again.
    Sweep,
}

impl Arcade {
    /// The wave on now or last, 0 before the first call.
    pub fn wave(&self) -> u32 {
        match self.phase {
            RunPhase::Ready => 0,
            RunPhase::Warning { wave, .. }
            | RunPhase::Fighting { wave }
            | RunPhase::Cleared { wave }
            | RunPhase::Over { wave } => wave,
        }
    }

    /// The wave the next call brings.
    pub fn next_wave(&self) -> u32 {
        match self.phase {
            RunPhase::Ready | RunPhase::Over { .. } => self.start_at.max(1),
            RunPhase::Cleared { wave } => wave + 1,
            RunPhase::Warning { wave, .. } | RunPhase::Fighting { wave } => wave + 1,
        }
    }

    /// Whether a call is answered now (not while a wave is coming or in).
    pub fn can_call(&self) -> bool {
        matches!(
            self.phase,
            RunPhase::Ready | RunPhase::Cleared { .. } | RunPhase::Over { .. }
        ) && self.reset == Reset::None
    }
}

/// The systems' order within a frame.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ArcadeSystems {
    Setup,
    Play,
    Record,
}

pub struct ArcadePlugin;

impl Plugin for ArcadePlugin {
    fn build(&self, app: &mut App) {
        let Some(opts) = app.world().get_resource::<LaunchOptions>().cloned() else {
            return;
        };
        if !active(&opts) {
            return;
        }
        app.insert_resource(Arcade {
            start_at: 1,
            ..default()
        })
        .add_message::<waves::WaveCalled>()
        .configure_sets(
            Update,
            (
                ArcadeSystems::Setup,
                ArcadeSystems::Play,
                ArcadeSystems::Record,
            )
                .chain()
                .after(crate::raid::downed),
        )
        .add_plugins((
            arena::ArenaPlugin,
            range::RangePlugin,
            waves::WavesPlugin,
            record::RecordPlugin,
            scenario::ScenarioPlugin,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_and_the_entry_point_agree() {
        let o = LaunchOptions::parse(["--arcade".to_string()]);
        assert!(active(&o));
        assert_eq!(o.map, MAP);
        assert!(!o.robots, "the director stays home");
        assert_eq!(o.hour.as_deref(), Some("day"));
        assert_eq!(o.weather.as_deref(), Some("clearing"));
        assert!(
            crate::persist::career_path(&o).is_none(),
            "an arcade session never reads or writes a career"
        );
        assert!(!o.rolls_career_seed());
        let mut m = LaunchOptions::default();
        enter(&mut m);
        assert_eq!(m.scene, o.scene);
        assert_eq!(m.map, o.map);
        // Asked-for weather stands.
        let w = LaunchOptions::parse("--weather fog --arcade".split(' ').map(String::from));
        assert_eq!(w.weather.as_deref(), Some("fog"));
        // The smoke scenario is an arcade session by itself.
        let s = LaunchOptions::parse(
            "--headless --scenario arcade_smoke"
                .split(' ')
                .map(String::from),
        );
        assert!(active(&s));
        // And nothing else is.
        assert!(!active(&LaunchOptions::parse(Vec::<String>::new())));
        assert!(!active(&LaunchOptions::parse(
            "--map training_yard".split(' ').map(String::from)
        )));
    }

    #[test]
    fn calls_follow_the_run() {
        let mut a = Arcade {
            start_at: 1,
            ..default()
        };
        assert!(a.can_call());
        assert_eq!(a.next_wave(), 1);
        a.phase = RunPhase::Warning { wave: 1, left: 3.0 };
        assert!(!a.can_call(), "one wave at a time");
        a.phase = RunPhase::Cleared { wave: 3 };
        assert_eq!(a.next_wave(), 4);
        a.phase = RunPhase::Over { wave: 5 };
        a.start_at = 4;
        assert_eq!(a.next_wave(), 4, "a new run starts where the board says");
        a.reset = Reset::Sweep;
        assert!(!a.can_call(), "not while the ground is being laid out");
    }
}
