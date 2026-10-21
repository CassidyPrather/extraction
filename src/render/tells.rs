//! Tells: what the game says with colour, and what it says the same thing
//! with besides (a sound, a rhythm, a shape), so a player who can't tell
//! the colours apart still can. Each group is a set of tells a player must
//! tell from each other; `visuals` take their colours from here, so the
//! test below checks what's drawn.
//!
//! The test simulates the three dichromacies (Machado, Oliveira and
//! Fernandes 2009, full severity, in linear RGB) and asks of every pair in
//! a group: do the colours still differ (CIELAB ΔE ≥ [`SEPARATE`]), or do
//! the pair differ in a channel that isn't colour?

/// A way a tell is told besides its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// A sound of its own (an `assets/audio` path, no extension or number).
    Sound(&'static str),
    /// A rhythm: steady, a slow pulse, a fast one.
    Rhythm(&'static str),
    /// A shape or motion: particles, haze, words.
    Shape(&'static str),
}

/// A colour that means something, linear RGB.
#[derive(Clone, Copy, Debug)]
pub struct Tell {
    pub name: &'static str,
    pub colour: [f32; 3],
    pub channels: &'static [Channel],
}

/// The anomalies' light colours (sRGB, as their lights take them).
pub const LIFT: [f32; 3] = [0.9, 0.85, 0.7];
pub const BURNER: [f32; 3] = [1.0, 0.32, 0.08];
pub const SNARE: [f32; 3] = [0.45, 0.65, 1.0];
pub const WELL: [f32; 3] = [0.6, 0.3, 1.0];
pub const MIRROR: [f32; 3] = [0.6, 0.85, 1.0];

/// Which anomaly is which: by its light, and by its own sound and motion.
pub const ANOMALIES: &[Tell] = &[
    Tell {
        name: "lift",
        colour: srgb(LIFT),
        channels: &[
            Channel::Sound("anomaly/lift_hum"),
            Channel::Shape("leaves drifting up"),
        ],
    },
    Tell {
        name: "burner",
        colour: srgb(BURNER),
        channels: &[
            Channel::Sound("anomaly/burner_roar"),
            Channel::Shape("embers, heat haze"),
        ],
    },
    Tell {
        name: "snare",
        colour: srgb(SNARE),
        channels: &[Channel::Sound("anomaly/snare_buzz"), Channel::Shape("arcs")],
    },
    Tell {
        name: "well",
        colour: srgb(WELL),
        channels: &[
            Channel::Sound("anomaly/well_drone"),
            Channel::Shape("grit, a lens of haze"),
        ],
    },
    Tell {
        name: "mirror",
        colour: srgb(MIRROR),
        channels: &[
            Channel::Sound("anomaly/mirror_shimmer"),
            Channel::Shape("glints"),
        ],
    },
];

/// A robot's eye: what it knows of you. Red brightening toward orange-white
/// as it suspects and sees; and a rhythm, which is what a colour-blind
/// player (or anyone at a distance, in fog) reads.
pub fn eye_colour(suspicion: f32, exposure: f32) -> [f32; 3] {
    let level = 0.45 + 0.9 * suspicion + 1.4 * exposure;
    [
        9.0 * level,
        (1.2 + 2.5 * exposure) * level,
        (0.6 + 0.8 * exposure) * level,
    ]
}

/// The eye's pulse (a factor on its glow): steady at work, a slow pulse
/// when it suspects something, a fast one while it can see you.
pub fn eye_pulse(suspicion: f32, sees: bool, t: f32) -> f32 {
    let hz = if sees {
        EYE_SEES_HZ
    } else if suspicion > 0.25 {
        EYE_SUSPECTS_HZ
    } else {
        return 1.0;
    };
    1.0 - 0.45 * (0.5 + 0.5 * (std::f32::consts::TAU * hz * t).sin())
}

pub const EYE_SUSPECTS_HZ: f32 = 0.8;
pub const EYE_SEES_HZ: f32 = 3.0;

/// The eye's three states, for the test (at work, suspicious, seeing you).
pub fn robot_eyes() -> [Tell; 3] {
    [
        Tell {
            name: "eye at work",
            colour: eye_colour(0.0, 0.0),
            channels: &[Channel::Rhythm("steady")],
        },
        Tell {
            name: "eye suspicious",
            colour: eye_colour(0.6, 0.0),
            channels: &[
                Channel::Rhythm("slow pulse"),
                Channel::Sound("machines/servo"),
            ],
        },
        Tell {
            name: "eye sees you",
            colour: eye_colour(0.8, 1.0),
            channels: &[
                Channel::Rhythm("fast pulse"),
                Channel::Sound("machines/servo"),
            ],
        },
    ]
}

/// Warnings that stand alone (no other tell to be mistaken for), each
/// with something besides its colour.
pub const WARNINGS: &[Tell] = &[
    Tell {
        name: "drone lock flare",
        colour: [1.0, 0.12, 0.08],
        channels: &[
            Channel::Sound("machines/drone_lock"),
            Channel::Rhythm("pulsed"),
        ],
    },
    Tell {
        name: "detector lamp",
        colour: [1.0, 0.1, 0.0],
        channels: &[
            Channel::Sound("machines/detector_click"),
            Channel::Rhythm("with the clicks"),
        ],
    },
    Tell {
        name: "wrist display",
        colour: [1.0, 0.68, 0.22],
        channels: &[Channel::Shape("words: SURGE, HP, BLEED")],
    },
];

/// ΔE (CIELAB) two colours must differ by to count as told apart.
pub const SEPARATE: f32 = 12.0;

const fn srgb(c: [f32; 3]) -> [f32; 3] {
    // `Color::srgb` as a light uses it: the linear values. (const: a
    // rational approximation of the sRGB curve is good enough to compare.)
    [lin(c[0]), lin(c[1]), lin(c[2])]
}

const fn lin(v: f32) -> f32 {
    // v^2.2 by two multiplies and a square root's worth: within a couple
    // of percent of the curve over 0..1.
    let v2 = v * v;
    v2 * (0.8 * v + 0.2)
}

/// A dichromacy, as a matrix on linear RGB.
#[derive(Clone, Copy, Debug)]
pub enum Vision {
    Normal,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

impl Vision {
    pub const ALL: [Vision; 4] = [
        Vision::Normal,
        Vision::Protanopia,
        Vision::Deuteranopia,
        Vision::Tritanopia,
    ];

    pub fn see(self, c: [f32; 3]) -> [f32; 3] {
        let m: [[f32; 3]; 3] = match self {
            Vision::Normal => return c,
            Vision::Protanopia => [
                [0.152286, 1.052583, -0.204868],
                [0.114503, 0.786281, 0.099216],
                [-0.003882, -0.048116, 1.051998],
            ],
            Vision::Deuteranopia => [
                [0.367322, 0.860646, -0.227968],
                [0.280085, 0.672501, 0.047413],
                [-0.011820, 0.042940, 0.968881],
            ],
            Vision::Tritanopia => [
                [1.255528, -0.076749, -0.178779],
                [-0.078411, 0.930809, 0.147602],
                [0.004733, 0.691367, 0.303900],
            ],
        };
        let r = |row: [f32; 3]| (row[0] * c[0] + row[1] * c[1] + row[2] * c[2]).max(0.0);
        [r(m[0]), r(m[1]), r(m[2])]
    }
}

/// CIELAB of a linear RGB colour (brighter than white is tone-mapped down
/// first, as the screen would).
pub fn lab(c: [f32; 3]) -> [f32; 3] {
    let t = |v: f32| v / (1.0 + v) * 2.0;
    let c = if c.iter().any(|v| *v > 1.0) {
        [t(c[0]), t(c[1]), t(c[2])]
    } else {
        c
    };
    let x = 0.4124 * c[0] + 0.3576 * c[1] + 0.1805 * c[2];
    let y = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let z = 0.0193 * c[0] + 0.1192 * c[1] + 0.9505 * c[2];
    let f = |v: f32| {
        if v > 0.008856 {
            v.cbrt()
        } else {
            7.787 * v + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.9505), f(y), f(z / 1.089));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

pub fn delta_e(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (a, b) = (lab(a), lab(b));
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Pairs in a group that can't be told apart by someone with `vision`:
/// colours too close, and nothing else between them.
pub fn confusions(group: &[Tell], vision: Vision) -> Vec<(&'static str, &'static str, f32)> {
    let mut out = Vec::new();
    for (i, a) in group.iter().enumerate() {
        for b in &group[i + 1..] {
            let d = delta_e(vision.see(a.colour), vision.see(b.colour));
            let other =
                !a.channels.is_empty() && !b.channels.is_empty() && a.channels != b.channels;
            if d < SEPARATE && !other {
                out.push((a.name, b.name, d));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Under each kind of colour blindness, every tell in a group is still
    /// told from every other: by its colour, or failing that by its sound,
    /// rhythm or shape. And no tell is colour alone.
    #[test]
    fn every_tell_separates_under_colour_blindness() {
        let eyes = robot_eyes();
        let groups: [(&str, &[Tell]); 3] = [
            ("anomalies", ANOMALIES),
            ("robot eyes", &eyes),
            ("warnings", WARNINGS),
        ];
        let mut bad = Vec::new();
        for (name, group) in groups {
            for t in group {
                if t.channels.is_empty() {
                    bad.push(format!("{name}: {} is colour alone", t.name));
                }
            }
            for v in Vision::ALL {
                for (a, b, d) in confusions(group, v) {
                    bad.push(format!("{name}, {v:?}: {a} and {b} (ΔE {d:.1})"));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// The simulation does what it's for: red and green, apart to normal
    /// eyes, run together for the red-green dichromacies, and blue and
    /// green for tritanopia; and the sounds the tells cite exist.
    #[test]
    fn the_simulation_and_the_sounds_are_real() {
        let (red, green) = ([0.8, 0.1, 0.05], [0.12, 0.45, 0.05]);
        assert!(delta_e(red, green) > 40.0);
        for v in [Vision::Protanopia, Vision::Deuteranopia] {
            let d = delta_e(v.see(red), v.see(green));
            assert!(d < delta_e(red, green) * 0.5, "{v:?}: {d}");
        }
        let (blue, teal) = ([0.05, 0.2, 0.8], [0.05, 0.35, 0.35]);
        let t = Vision::Tritanopia;
        assert!(delta_e(t.see(blue), t.see(teal)) < delta_e(blue, teal));
        let eyes = robot_eyes();
        for t in ANOMALIES.iter().chain(&eyes).chain(WARNINGS) {
            for c in t.channels {
                if let Channel::Sound(s) = c {
                    let dir = crate::asset_root_path().join("audio");
                    let found = std::fs::read_dir(dir.join(s).parent().unwrap())
                        .into_iter()
                        .flatten()
                        .flatten()
                        .any(|f| {
                            f.file_name().to_string_lossy().starts_with(
                                std::path::Path::new(s)
                                    .file_name()
                                    .unwrap()
                                    .to_str()
                                    .unwrap(),
                            )
                        });
                    assert!(found, "{}: no sound {s}", t.name);
                }
            }
        }
    }
}
