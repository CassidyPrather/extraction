//! Captions (`Comfort::captions`): what's said, with who's saying it and
//! from where, for the wrist. The driver over the radio, the driver from
//! the cab at base, the machines' chatter a radio picks up (from where
//! they're headed). Narration (the game's own words) has no speaker.

use bevy::prelude::*;

use super::Raid;
use super::chatter::RobotChatter;
use super::truck::Truck;

/// Who's speaking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speaker {
    /// Lev, over the radio on you.
    LevRadio,
    /// Lev, from the truck's cab (at base).
    LevCab,
    /// Machines' chatter, picked up on your radio.
    Machines,
    /// Not speech: the game's own words.
    None,
}

/// The latest thing said.
#[derive(Clone, Debug, PartialEq)]
pub struct Caption {
    pub speaker: Speaker,
    /// The words, without the speaker's prefix and quotes.
    pub text: String,
    /// Where it comes from (none: on you, the radio).
    pub from: Option<Vec3>,
}

#[derive(Resource, Default, Debug)]
pub struct Captions {
    pub last: Option<Caption>,
}

pub struct CaptionsPlugin;

impl Plugin for CaptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Captions>()
            .add_systems(Update, caption_said);
    }
}

/// Splits a said line into its speaker and words: `Truck: "..."` is Lev on
/// the radio, `Driver: "..."` Lev from the cab.
pub fn parse(line: &str) -> (Speaker, String) {
    let unquote = |s: &str| {
        let s = s.trim();
        let s = s.strip_prefix('"').unwrap_or(s);
        s.strip_suffix('"').unwrap_or(s).to_string()
    };
    if let Some(rest) = line.strip_prefix("Truck:") {
        (Speaker::LevRadio, unquote(rest))
    } else if let Some(rest) = line.strip_prefix("Driver:") {
        (Speaker::LevCab, unquote(rest))
    } else {
        (Speaker::None, line.to_string())
    }
}

/// The clock bearing of `at` from a head (12 ahead, 3 to the right).
pub fn clock(head: &GlobalTransform, at: Vec3) -> u8 {
    let fwd = (head.rotation() * Vec3::NEG_Z)
        .xz()
        .normalize_or(Vec2::NEG_Y);
    let to = (at - head.translation()).xz().normalize_or(fwd);
    // Clockwise from ahead, seen from above.
    let a = fwd.angle_to(to);
    let h = (a / std::f32::consts::TAU * 12.0).round().rem_euclid(12.0) as u8;
    if h == 0 { 12 } else { h }
}

impl Caption {
    /// The wrist's speaker line: "LEV, RADIO", "LEV AT 11", "MACHINES AT
    /// 2" (where on the clock face, 12 ahead); none for narration. Short
    /// enough for a line of the wrist.
    pub fn header(&self, head: Option<&GlobalTransform>) -> Option<String> {
        let at = |what: &str| match (self.from, head) {
            (Some(p), Some(h)) => format!("{what} AT {}", clock(h, p)),
            _ => format!("{what}, RADIO"),
        };
        match self.speaker {
            Speaker::LevRadio => Some("LEV, RADIO".into()),
            Speaker::LevCab => Some(at("LEV")),
            Speaker::Machines => Some(at("MACHINES")),
            Speaker::None => None,
        }
    }
}

/// Each new line said becomes the caption, with its speaker and source.
fn caption_said(
    raid: Res<Raid>,
    mut captions: ResMut<Captions>,
    mut chatter: MessageReader<RobotChatter>,
    trucks: Query<&GlobalTransform, With<Truck>>,
    mut last: Local<Option<String>>,
    mut heard: Local<Option<(Vec3, f32)>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    for c in chatter.read() {
        *heard = Some((c.at, now));
    }
    let Some((line, _)) = raid.log.last() else {
        return;
    };
    if last.as_deref() == Some(line.as_str()) {
        return;
    }
    *last = Some(line.clone());
    let (speaker, text) = parse(line);
    // A line that came with chatter is the machines', from where they are.
    let chatter = heard.filter(|(_, t)| now - t < 1.0).map(|(at, _)| at);
    captions.last = Some(match (speaker, chatter) {
        (Speaker::None, Some(at)) => Caption {
            speaker: Speaker::Machines,
            text,
            from: Some(at),
        },
        (Speaker::LevCab, _) => Caption {
            speaker,
            text,
            from: trucks.iter().next().map(|t| t.translation()),
        },
        _ => Caption {
            speaker,
            text,
            from: None,
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_split_into_speaker_and_words() {
        assert_eq!(
            parse("Truck: \"Get in the back.\""),
            (Speaker::LevRadio, "Get in the back.".to_string())
        );
        assert_eq!(
            parse("Driver: \"You came back.\""),
            (Speaker::LevCab, "You came back.".to_string())
        );
        assert_eq!(
            parse("You bind the wound."),
            (Speaker::None, "You bind the wound.".to_string())
        );
    }

    /// Ahead is 12, right 3, behind 6, left 9.
    #[test]
    fn the_clock_reads_round_the_head() {
        let head = GlobalTransform::from(Transform::from_xyz(0.0, 1.6, 0.0));
        let at = |x: f32, z: f32| clock(&head, Vec3::new(x, 1.0, z));
        assert_eq!(at(0.0, -10.0), 12);
        assert_eq!(at(10.0, 0.0), 3);
        assert_eq!(at(0.0, 10.0), 6);
        assert_eq!(at(-10.0, 0.0), 9);
        assert_eq!(at(5.0, -8.66), 1);
    }

    /// In the game: Lev on the radio is captioned as such; machines heard
    /// on the radio, with where they are.
    #[test]
    fn what_is_said_is_captioned_with_its_speaker() {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                map: "marrow_creek".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<Raid>()
            .say("Truck: \"Heard you. Two minutes.\"");
        app.update();
        let c = app.world().resource::<Captions>().last.clone().unwrap();
        assert_eq!(c.speaker, Speaker::LevRadio);
        assert_eq!(c.text, "Heard you. Two minutes.");
        assert_eq!(c.header(None).as_deref(), Some("LEV, RADIO"));
        // Chatter, and its line.
        let radio = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(RobotChatter {
            radio,
            at: Vec3::new(100.0, 0.0, 0.0),
            close: false,
        });
        app.world_mut()
            .resource_mut::<Raid>()
            .say("The radio crackles: machines, east.");
        app.update();
        let c = app.world().resource::<Captions>().last.clone().unwrap();
        assert_eq!(c.speaker, Speaker::Machines);
        let head = GlobalTransform::from(Transform::from_xyz(0.0, 1.6, 0.0));
        assert_eq!(c.header(Some(&head)).as_deref(), Some("MACHINES AT 3"));
        // The longest fits a line of the wrist.
        let far = Caption {
            from: Some(Vec3::new(0.0, 0.0, -10.0)),
            ..c
        };
        assert!(far.header(Some(&head)).unwrap().chars().count() <= 18);
    }
}
