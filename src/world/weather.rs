//! Weather: each Surge cycle brings its own, a pure function of the seed
//! and the cycle. It's gameplay as much as looks — visibility caps how far
//! robots (and you) see, and rain masks noise — so it lives here, and the
//! sky, fog, lights and rain read it (`sky::apply_weather`, `rain`).
//!
//! The hour is the second axis: each cycle's raid starts at dawn, in the
//! day or at dusk, from a stream of its own (so the weather a seed gives
//! is the same as before there were hours). The light it brings is
//! gameplay too: robots see less far as it fails (`Conditions::sight`).
//! Night comes late in the ladder: once Progression opens it (its third
//! tier), about one cycle in five is a night; before that, `--hour night`
//! shows it.

use bevy::prelude::*;
use extraction_core::noise::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherKind {
    Drizzle,
    PeaSouper,
    Downpour,
    Clearing,
}

/// Indoors, a room's walls let in this share of the sky's light (the
/// client dims the ambient to it; robots' senses can read it too).
pub const INTERIOR_LIGHT: f32 = 0.3;

/// When in the day the raid is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hour {
    Dawn,
    Day,
    Dusk,
    Night,
}

impl Hour {
    /// The hour after this one (a night slept in a safehouse).
    pub fn next(self) -> Self {
        match self {
            Hour::Dawn => Hour::Day,
            Hour::Day => Hour::Dusk,
            Hour::Dusk => Hour::Night,
            Hour::Night => Hour::Dawn,
        }
    }

    /// The sky's light, against the day's (0..=1).
    pub fn light(self) -> f32 {
        match self {
            Hour::Dawn => 0.45,
            Hour::Day => 1.0,
            Hour::Dusk => 0.36,
            Hour::Night => 0.04,
        }
    }

    /// The sun's share of its daytime strength: low and through more air.
    pub fn sun(self) -> f32 {
        match self {
            Hour::Dawn => 0.45,
            Hour::Day => 1.0,
            Hour::Dusk => 0.4,
            Hour::Night => 0.0,
        }
    }

    /// `dawn`, `day`, `dusk` or `night`, or a clock hour (`23`, `5:30`)
    /// to the hour it falls in: dawn from 5, day from 7, dusk from 18,
    /// night from 21. A clock hour once fell through to the raid's own
    /// hour, so `--hour 23` shot daylight.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "dawn" => return Some(Hour::Dawn),
            "day" => return Some(Hour::Day),
            "dusk" => return Some(Hour::Dusk),
            "night" => return Some(Hour::Night),
            _ => {}
        }
        let (h, m) = s.split_once(':').unwrap_or((s, "0"));
        let (h, m) = (h.parse::<f32>().ok()?, m.parse::<f32>().ok()?);
        let t = h + m / 60.0;
        if !(0.0..=24.0).contains(&t) || !(0.0..60.0).contains(&m) {
            return None;
        }
        Some(match t {
            t if (5.0..7.0).contains(&t) => Hour::Dawn,
            t if (7.0..18.0).contains(&t) => Hour::Day,
            t if (18.0..21.0).contains(&t) => Hour::Dusk,
            _ => Hour::Night,
        })
    }
}

/// What `--weather` takes (`rain` is drizzle).
pub const WEATHERS: &[&str] = &["clearing", "drizzle", "rain", "fog", "downpour", "storm"];

/// The moon's month is this many Surge cycles: its phase is the cycle's.
pub const MOON_CYCLES: u32 = 8;

/// The moon's phase in a cycle, 0..1: 0 new, 0.25 first quarter, 0.5 full,
/// 0.75 last quarter.
pub fn moon_phase(cycle: u32) -> f32 {
    (cycle % MOON_CYCLES) as f32 / MOON_CYCLES as f32
}

/// How much of the moon's face is lit, 0 (new) to 1 (full).
pub fn moon_lit(phase: f32) -> f32 {
    (1.0 - (phase * std::f32::consts::TAU).cos()) * 0.5
}

/// Where the moon is in a night raid, along the sun's own path (east at
/// dawn, over the north at noon, west at dusk), so its bearing is a
/// compass the player can learn: full, it stands where the noon sun does;
/// waxing, it's down toward the west; waning, up from the east; within a
/// few days of new, it isn't up. Returns (bearing from +X toward +Z in
/// degrees, elevation in degrees), or None when it's down.
pub fn moon_position(phase: f32) -> Option<(f32, f32)> {
    let s = 1.0 - (phase - 0.25) / 0.5;
    if !(0.0..=1.0).contains(&s) {
        return None;
    }
    Some((
        sun_path_bearing(s),
        8.0 + 42.0 * (std::f32::consts::PI * s).sin(),
    ))
}

/// The sun's path, 0 (dawn, the east) to 1 (dusk, the west), over the
/// north (-Z): its bearing in degrees from +X toward +Z.
pub fn sun_path_bearing(s: f32) -> f32 {
    11.0 - 174.0 * s
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Conditions {
    pub kind: WeatherKind,
    pub hour: Hour,
    /// The moon's phase (`moon_phase`) and how much of it lights the night:
    /// its lit face if it's up, else 0 (and 0 by day).
    pub moon_phase: f32,
    pub moon: f32,
    /// The sky's light against a day's, 0..=1 (`Hour::light`).
    pub light: f32,
    /// Metres at which things fade out.
    pub visibility: f32,
    /// 0..=1.
    pub rain: f32,
    /// Ground-fog density at its base height (see `render::fog`).
    pub ground_fog: f32,
    /// Direct sun, lux (the weather's, at this hour).
    pub sun: f32,
    /// A downpour with thunder in it (`world::storm`): lightning, harder
    /// wind, darker, the ground soaking fast. Only ever on a `Downpour`,
    /// so everything that reads the kind takes it for one.
    pub storm: bool,
    pub cycle: u32,
}

impl Conditions {
    pub fn for_cycle(seed: u64, cycle: u32) -> Self {
        Self::for_cycle_with(seed, cycle, false)
    }

    /// The cycle's conditions, with night raids open or not.
    pub fn for_cycle_with(seed: u64, cycle: u32, nights: bool) -> Self {
        let mut rng = Rng::new(seed ^ 0x3EA7_4E12 ^ (cycle as u64).wrapping_mul(0x9E37_79B9));
        // The first cycle is always the reference look.
        let kind = if cycle == 0 {
            WeatherKind::Drizzle
        } else {
            [
                WeatherKind::Drizzle,
                WeatherKind::Drizzle,
                WeatherKind::PeaSouper,
                WeatherKind::Downpour,
                WeatherKind::Clearing,
            ][rng.below(5)]
        };
        let c = Self::compose(kind, Self::hour_for_cycle(seed, cycle, nights), cycle);
        // Half the downpours carry thunder, from a stream of their own (so
        // the other cycles' weather and hours stay as they were).
        let mut t = Rng::new(seed ^ 0x57_0A11 ^ (cycle as u64).wrapping_mul(0x1656_67B1));
        if kind == WeatherKind::Downpour && t.below(2) == 0 {
            c.stormy()
        } else {
            c
        }
    }

    /// This downpour as a thunderstorm: darker under the anvil, harder
    /// rain, shorter sight. (Anything else is left as it is.)
    pub fn stormy(mut self) -> Self {
        if self.kind != WeatherKind::Downpour {
            return self;
        }
        self.storm = true;
        self.visibility = 190.0;
        self.rain = 1.0;
        self.light *= 0.88;
        self.sun *= 0.6;
        self
    }

    /// The cycle's hour, from a stream of its own: the first cycle is the
    /// day, then half are days and the rest dawn or dusk; with nights
    /// open, about one in five is a night instead (from a third stream, so
    /// the other cycles keep their hours).
    pub fn hour_for_cycle(seed: u64, cycle: u32, nights: bool) -> Hour {
        if cycle == 0 {
            return Hour::Day;
        }
        if nights {
            let mut n = Rng::new(seed ^ 0x0A16_47E5 ^ (cycle as u64).wrapping_mul(0x27D4_EB2F));
            if n.below(5) == 0 {
                return Hour::Night;
            }
        }
        let mut rng = Rng::new(seed ^ 0x40C1_0C4D ^ (cycle as u64).wrapping_mul(0xC2B2_AE3D));
        [Hour::Dawn, Hour::Day, Hour::Day, Hour::Dusk][rng.below(4)]
    }

    /// A weather at an hour.
    pub fn compose(kind: WeatherKind, hour: Hour, cycle: u32) -> Self {
        Self::compose_with_moon(kind, hour, cycle, moon_phase(cycle))
    }

    /// `compose` under a given moon phase (the cycle's, unless a dev
    /// switch asks for another).
    pub fn compose_with_moon(kind: WeatherKind, hour: Hour, cycle: u32, phase: f32) -> Self {
        let (visibility, rain, ground_fog, sun) = match kind {
            WeatherKind::Drizzle => (320.0, 0.35, 0.011, 1600.0),
            WeatherKind::PeaSouper => (150.0, 0.0, 0.024, 1100.0),
            WeatherKind::Downpour => (210.0, 1.0, 0.009, 900.0),
            WeatherKind::Clearing => (520.0, 0.0, 0.006, 2600.0),
        };
        // Mist settles at the ends of the day.
        let mist = match hour {
            Hour::Dawn => 1.5,
            Hour::Dusk => 1.2,
            Hour::Night => 1.3,
            Hour::Day => 1.0,
        };
        let moon_phase = phase;
        let up = moon_position(moon_phase).is_some();
        let moon = if hour == Hour::Night && up {
            moon_lit(moon_phase)
        } else {
            0.0
        };
        // At night the sky's light and the "sun" are the moon's: under
        // cloud almost nothing (an overcast night really is black); in a
        // clearing, starlight and more under a fuller moon, and the moon
        // itself a cool light that casts shadows.
        let (light, sun) = if hour == Hour::Night {
            match kind {
                WeatherKind::Clearing => (0.02 + 0.06 * moon, 220.0 * moon),
                WeatherKind::Drizzle => (0.012 + 0.006 * moon, 15.0 * moon),
                WeatherKind::PeaSouper | WeatherKind::Downpour => (0.01, 0.0),
            }
        } else {
            (hour.light(), sun * hour.sun())
        };
        Self {
            kind,
            hour,
            moon_phase,
            moon,
            light,
            visibility,
            rain,
            ground_fog: ground_fog * mist,
            sun,
            storm: false,
            cycle,
        }
    }

    /// The sky's light at a place, against a day's outdoors (0..=1):
    /// `light`, and a room's share of it indoors.
    pub fn light_at(&self, indoors: bool) -> f32 {
        self.light * if indoors { INTERIOR_LIGHT } else { 1.0 }
    }

    /// How far a robot's eyes reach, given its own sight range: the
    /// weather caps it, and failing light shortens it (to 60% at dusk,
    /// 40% at night: they don't see in the dark much better than you).
    pub fn sight(&self, own: f32) -> f32 {
        own.min(self.visibility * 0.6) * self.light.sqrt().max(0.4)
    }

    /// How hard the wind blows, 0..=1: still in thick fog, hard in a
    /// downpour.
    pub fn wind(&self) -> f32 {
        if self.storm {
            return 1.0;
        }
        match self.kind {
            WeatherKind::Drizzle => 0.4,
            WeatherKind::PeaSouper => 0.12,
            WeatherKind::Downpour => 0.9,
            WeatherKind::Clearing => 0.55,
        }
    }

    /// Hearing scale: rain covers footsteps (not gunshots, much).
    pub fn hearing(&self) -> f32 {
        1.0 - 0.35 * self.rain
    }
}

/// How wet the world's surfaces are, 0 (dry) to 1 (soaked): rain wets
/// them over minutes and they dry over twenty, faster in
/// sun; mist keeps them damp. The raid starts with the ground as the
/// weather has left it; a new cycle's weather then works on it. Drives
/// the surfaces' look (`render::fog`'s wet patch, the terrain, puddles).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Wetness {
    pub level: f32,
    settled: bool,
}

/// Minutes rain takes to bring the ground most of the way to soaked.
pub const SOAK_MINUTES: f32 = 4.0;
/// Minutes the ground takes to dry most of the way, in no sun.
pub const DRY_MINUTES: f32 = 25.0;

impl Wetness {
    /// Where the weather leaves the ground, given time.
    pub fn target(c: &Conditions) -> f32 {
        if c.rain > 0.0 {
            (0.35 + 0.65 * c.rain).min(1.0)
        } else if c.kind == WeatherKind::PeaSouper {
            // Mist: damp, never running wet.
            0.3
        } else {
            0.0
        }
    }

    /// `dt` seconds of the weather.
    pub fn step(&mut self, c: &Conditions, dt: f32) {
        let target = Self::target(c);
        let minutes = if target > self.level {
            // Heavier rain soaks faster; a storm's cloudburst fills the
            // hollows in a minute or two.
            SOAK_MINUTES / (0.4 + c.rain).min(1.4) / if c.storm { 3.0 } else { 1.0 }
        } else {
            // Sun and wind dry it.
            DRY_MINUTES / (1.0 + 0.5 * c.sun / 1600.0 + 0.3 * c.wind())
        };
        let k = 1.0 - (-dt / (minutes * 60.0 / 3.0)).exp();
        self.level += (target - self.level) * k;
    }
}

/// The ground as the weather keeps it.
fn soak(conditions: Res<Conditions>, mut wet: ResMut<Wetness>, time: Res<Time>) {
    if !wet.settled {
        // A raid starts with the ground as this weather has left it.
        wet.level = Wetness::target(&conditions);
        wet.settled = true;
        return;
    }
    wet.step(&conditions, time.delta_secs());
}

/// Lightning struck (`world::storm`): where, how far from the eye (m),
/// and how bright (0..1). Thunder follows by `distance / 343` seconds
/// (`storm::thunder_delay`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Lightning {
    pub at: Vec3,
    pub distance: f32,
    pub strength: f32,
}

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Conditions::for_cycle(1, 0))
            .init_resource::<Wetness>()
            .add_message::<Lightning>()
            .add_systems(Update, (follow_cycle, soak).chain());
    }
}

/// New Surge cycle (or a loaded save), new weather.
/// Night raids are open once the career's third tier is done
/// (`Progress::nights`).

fn follow_cycle(
    raid: Res<crate::raid::Raid>,
    opts: Res<crate::launch::LaunchOptions>,
    progression: Option<Res<crate::progression::Progression>>,
    shift: Option<Res<crate::progression::safehouse::HourShift>>,
    mut conditions: ResMut<Conditions>,
    mut latched: Local<Option<(u32, bool)>>,
) {
    // Whether nights are open is read when the cycle starts and kept: a
    // tier finished at base mustn't turn the raid's day to night.
    let open = progression.is_some_and(|p| p.0.nights());
    let nights = match *latched {
        Some((cycle, n)) if cycle == raid.cycle => n,
        _ => {
            *latched = Some((raid.cycle, open));
            open
        }
    };
    let mut want = Conditions::for_cycle_with(opts.seed, raid.cycle, nights);
    // Dev overrides: the asked-for weather and hour, the rest as rolled.
    let kind = match opts.weather.as_deref() {
        Some("fog") => WeatherKind::PeaSouper,
        Some("downpour" | "storm") => WeatherKind::Downpour,
        Some("clearing") => WeatherKind::Clearing,
        Some(_) => WeatherKind::Drizzle,
        None => want.kind,
    };
    let hour = opts
        .hour
        .as_deref()
        .and_then(Hour::parse)
        .unwrap_or(want.hour);
    // Nights slept in a safehouse move the hour on (`progression::safehouse`).
    let hour = (0..shift.map_or(0, |s| s.0)).fold(hour, |h, _| h.next());
    let phase = opts.moon.unwrap_or(want.moon_phase);
    let storm = match opts.weather.as_deref() {
        Some("storm") => true,
        Some(_) => false,
        None => want.storm,
    };
    if (kind, hour, phase, storm) != (want.kind, want.hour, want.moon_phase, want.storm) {
        want = Conditions::compose_with_moon(kind, hour, raid.cycle, phase);
        if storm {
            want = want.stormy();
        }
    }
    if *conditions != want {
        *conditions = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--hour 23` once fell through to the raid's own hour (daylight).
    #[test]
    fn clock_hours_parse_to_their_hour() {
        for (s, h) in [
            ("23", Hour::Night),
            ("0", Hour::Night),
            ("4:59", Hour::Night),
            ("5", Hour::Dawn),
            ("6:30", Hour::Dawn),
            ("7", Hour::Day),
            ("12:15", Hour::Day),
            ("17:59", Hour::Day),
            ("18", Hour::Dusk),
            ("20:30", Hour::Dusk),
            ("21", Hour::Night),
            ("24", Hour::Night),
            ("night", Hour::Night),
            ("dusk", Hour::Dusk),
        ] {
            assert_eq!(Hour::parse(s), Some(h), "{s}");
        }
        for bad in ["25", "-1", "12:60", "noon", "", "23h"] {
            assert_eq!(Hour::parse(bad), None, "{bad}");
        }
        assert!(Hour::Night.light() < 0.1 * Hour::Day.light());
    }

    #[test]
    fn storms_come_with_some_downpours_and_soak_the_ground_fast() {
        let all: Vec<Conditions> = (0..400).map(|c| Conditions::for_cycle(9, c)).collect();
        let pours = all
            .iter()
            .filter(|c| c.kind == WeatherKind::Downpour)
            .count();
        let storms = all.iter().filter(|c| c.storm).count();
        assert!(
            all.iter()
                .all(|c| !c.storm || c.kind == WeatherKind::Downpour)
        );
        assert!(
            storms > pours / 4 && storms < pours * 3 / 4,
            "{storms} of {pours}"
        );
        // The storm takes nothing from the other cycles' rolls.
        for (i, c) in all.iter().enumerate() {
            assert_eq!(c.hour, Conditions::hour_for_cycle(9, i as u32, false));
        }
        let pour = Conditions::compose(WeatherKind::Downpour, Hour::Day, 3);
        let storm = pour.stormy();
        assert!(storm.light < pour.light && storm.visibility < pour.visibility);
        assert_eq!(storm.wind(), 1.0);
        // Not a storm without the rain.
        assert!(
            !Conditions::compose(WeatherKind::Clearing, Hour::Day, 3)
                .stormy()
                .storm
        );
        let soaked = |c: &Conditions| {
            // Twenty seconds in, from dry.
            let mut w = Wetness::default();
            w.step(c, 20.0);
            w.level
        };
        assert!(soaked(&storm) > 1.5 * soaked(&pour));
    }

    #[test]
    fn weather_varies_by_cycle_but_not_by_chance() {
        let a: Vec<WeatherKind> = (0..20).map(|c| Conditions::for_cycle(5, c).kind).collect();
        let b: Vec<WeatherKind> = (0..20).map(|c| Conditions::for_cycle(5, c).kind).collect();
        assert_eq!(a, b);
        assert_eq!(a[0], WeatherKind::Drizzle);
        assert!(a.iter().any(|k| *k != WeatherKind::Drizzle));
    }

    #[test]
    fn the_hour_is_a_pure_function_of_seed_and_cycle() {
        for seed in [1, 5, 99] {
            let a: Vec<Hour> = (0..40)
                .map(|c| Conditions::for_cycle(seed, c).hour)
                .collect();
            let b: Vec<Hour> = (0..40)
                .map(|c| Conditions::for_cycle(seed, c).hour)
                .collect();
            assert_eq!(a, b);
            // The first raid is in daylight; dawns, days and dusks all come.
            assert_eq!(a[0], Hour::Day);
            for h in [Hour::Dawn, Hour::Day, Hour::Dusk] {
                assert!(a.contains(&h), "seed {seed}: no {h:?} in 40 cycles");
            }
            // Night is for later in the ladder: not until it's open...
            assert!(!a.contains(&Hour::Night));
            // ...and then about one in five, the rest keeping their hours.
            let n: Vec<Hour> = (0..200)
                .map(|c| Conditions::for_cycle_with(seed, c, true).hour)
                .collect();
            let nights = n.iter().filter(|h| **h == Hour::Night).count();
            assert!(
                (25..=60).contains(&nights),
                "seed {seed}: {nights} nights in 200"
            );
            assert_eq!(n[0], Hour::Day);
            for c in 0..40 {
                if n[c] != Hour::Night {
                    assert_eq!(n[c], a[c]);
                }
            }
        }
        // Different seeds give different days.
        let one: Vec<Hour> = (0..40).map(|c| Conditions::for_cycle(1, c).hour).collect();
        let two: Vec<Hour> = (0..40).map(|c| Conditions::for_cycle(2, c).hour).collect();
        assert_ne!(one, two);
    }

    #[test]
    fn hours_leave_the_weather_as_it_was() {
        // The weather a seed rolls, from before there were hours.
        let kinds: Vec<WeatherKind> = (0..12).map(|c| Conditions::for_cycle(5, c).kind).collect();
        for c in 0..12u32 {
            let mut rng = Rng::new(5 ^ 0x3EA7_4E12 ^ (c as u64).wrapping_mul(0x9E37_79B9));
            let old = if c == 0 {
                WeatherKind::Drizzle
            } else {
                [
                    WeatherKind::Drizzle,
                    WeatherKind::Drizzle,
                    WeatherKind::PeaSouper,
                    WeatherKind::Downpour,
                    WeatherKind::Clearing,
                ][rng.below(5)]
            };
            assert_eq!(kinds[c as usize], old);
        }
    }

    #[test]
    fn rain_soaks_in_minutes_and_dries_in_twenty() {
        let pour = Conditions::compose(WeatherKind::Downpour, Hour::Day, 1);
        let clear = Conditions::compose(WeatherKind::Clearing, Hour::Day, 1);
        let mist = Conditions::compose(WeatherKind::PeaSouper, Hour::Day, 1);
        let run = |w: &mut Wetness, c: &Conditions, secs: u32| {
            for _ in 0..secs {
                w.step(c, 1.0);
            }
        };
        // Dry ground in a downpour: wet within a minute or two, soaked
        // within five, never past soaked.
        let mut w = Wetness::default();
        run(&mut w, &pour, 60);
        assert!(w.level > 0.3 && w.level < 0.9, "{}", w.level);
        run(&mut w, &pour, 240);
        assert!(w.level > 0.9 && w.level <= 1.0, "{}", w.level);
        // Then the sky clears: still wet after five minutes, mostly dry by
        // twenty, and the clouded sun dries slower than a clear one.
        let soaked = w;
        run(&mut w, &clear, 300);
        assert!(w.level > 0.25, "{}", w.level);
        run(&mut w, &clear, 900);
        assert!(w.level < 0.1, "{}", w.level);
        let mut dim = soaked;
        let dusk = Conditions::compose(WeatherKind::Clearing, Hour::Dusk, 1);
        run(&mut dim, &dusk, 300);
        let mut bright = soaked;
        run(&mut bright, &clear, 300);
        assert!(dim.level > bright.level);
        // Mist keeps it damp, no more.
        let mut m = Wetness::default();
        run(&mut m, &mist, 3600);
        assert!((m.level - 0.3).abs() < 0.02);
    }

    #[test]
    fn nights_are_lit_by_the_moon_and_black_under_cloud() {
        let night = |k, cycle| Conditions::compose(k, Hour::Night, cycle);
        // The moon's month: new at cycle 0, full at 4.
        assert_eq!(moon_lit(moon_phase(0)), 0.0);
        assert!((moon_lit(moon_phase(4)) - 1.0).abs() < 1e-5);
        let full = night(WeatherKind::Clearing, 4);
        let new = night(WeatherKind::Clearing, 0);
        let quarter = night(WeatherKind::Clearing, 2);
        // A clear full-moon night: the moon up, casting light, the sky
        // brighter than a new moon's, which is brighter than any overcast.
        assert!(full.moon > 0.99 && full.sun > 150.0);
        assert!(full.light > quarter.light && quarter.light > new.light);
        for k in [
            WeatherKind::Drizzle,
            WeatherKind::PeaSouper,
            WeatherKind::Downpour,
        ] {
            for c in 0..MOON_CYCLES {
                assert!(night(k, c).light < new.light, "{k:?} cycle {c}");
                assert!(night(k, c).sun < 20.0);
            }
        }
        // Not a moon by day.
        assert_eq!(
            Conditions::compose(WeatherKind::Clearing, Hour::Day, 4).moon,
            0.0
        );
    }

    #[test]
    fn the_moon_keeps_to_the_suns_path() {
        // Full: where the noon sun stands, high over the north (-Z side).
        let (b, e) = moon_position(0.5).unwrap();
        assert!((b - sun_path_bearing(0.5)).abs() < 1e-3 && e > 45.0);
        assert!(b.to_radians().sin() < -0.9, "the north is -Z");
        // Waxing: low in the west (the dusk end); waning: low in the east.
        let (west, low) = moon_position(0.27).unwrap();
        assert!(west.to_radians().cos() < -0.9 && low < 15.0);
        let (east, _) = moon_position(0.73).unwrap();
        assert!(east.to_radians().cos() > 0.9);
        // Near new, it's down.
        assert!(moon_position(0.05).is_none() && moon_position(0.95).is_none());
    }

    #[test]
    fn failing_light_shortens_sight() {
        let day = Conditions::compose(WeatherKind::Clearing, Hour::Day, 1);
        let dusk = Conditions::compose(WeatherKind::Clearing, Hour::Dusk, 1);
        let night = Conditions::compose(WeatherKind::Clearing, Hour::Night, 1);
        assert_eq!(day.sight(90.0), 90.0);
        assert!(dusk.sight(90.0) < day.sight(90.0) * 0.7);
        assert!(night.sight(90.0) < dusk.sight(90.0));
        assert!(dusk.sun < day.sun && night.sun == 0.0);
    }
}
